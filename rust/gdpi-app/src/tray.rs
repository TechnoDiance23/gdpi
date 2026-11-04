//! Tepsi simgesi: Göster / Başlat-Durdur / Çıkış. Linux'ta GTK gerekir; yoksa çağıran
//! "Tepsi simgesi kullanılamıyor" yazar ve pencere kapatılınca uygulama kapanır.
use std::sync::OnceLock;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy)]
pub enum Cmd {
    Show,
    Toggle,
    Quit,
}

type Handler = Box<dyn Fn(Cmd) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();

struct Tray {
    icon: TrayIcon,
    toggle: MenuItem,
}

fn icon(on: bool) -> Icon {
    Icon::from_rgba(crate::shield::shield_rgba(on, 64), 64, 64).expect("simge")
}

fn tooltip(on: bool) -> &'static str {
    if on {
        "GDPI - Açık"
    } else {
        "GDPI - Kapalı"
    }
}

impl Tray {
    fn build(on: bool) -> Result<Tray, String> {
        let show = MenuItem::with_id("show", "Göster", true, None);
        let toggle = MenuItem::with_id("toggle", if on { "Durdur" } else { "Başlat" }, true, None);
        let quit = MenuItem::with_id("quit", "Çıkış", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show, &toggle, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip(on))
            .with_icon(icon(on))
            .with_title("GDPI")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Tray { icon, toggle })
    }

    fn set(&self, on: bool) {
        let _ = self.icon.set_icon(Some(icon(on)));
        let _ = self.icon.set_tooltip(Some(tooltip(on)));
        self.toggle.set_text(if on { "Durdur" } else { "Başlat" });
    }
}

fn dispatch(c: Cmd) {
    if let Some(h) = HANDLER.get() {
        h(c);
    }
}

fn install_handlers(handler: Handler) {
    let _ = HANDLER.set(handler);
    MenuEvent::set_event_handler(Some(|e: MenuEvent| match e.id().as_ref() {
        "show" => dispatch(Cmd::Show),
        "toggle" => dispatch(Cmd::Toggle),
        "quit" => dispatch(Cmd::Quit),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            dispatch(Cmd::Show);
        }
    }));
}

// ---------------------------------------------------------------- Windows / diğer: ana iş parçacığı
#[cfg(not(target_os = "linux"))]
thread_local! {
    static TRAY: std::cell::RefCell<Option<Tray>> = const { std::cell::RefCell::new(None) };
}

/// Ana iş parçacığında (olay döngüsü) çağrılmalı.
#[cfg(not(target_os = "linux"))]
pub fn init(on: bool, handler: Handler) -> Result<(), String> {
    let t = Tray::build(on)?;
    TRAY.with(|c| *c.borrow_mut() = Some(t));
    install_handlers(handler);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn set_state(on: bool) {
    TRAY.with(|c| {
        if let Ok(b) = c.try_borrow() {
            if let Some(t) = b.as_ref() {
                t.set(on);
            }
        }
    });
}

// ---------------------------------------------------------------- Linux: kendi GTK iş parçacığı
#[cfg(target_os = "linux")]
static STATE_TX: OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<bool>>> = OnceLock::new();

#[cfg(target_os = "linux")]
pub fn init(on: bool, handler: Handler) -> Result<(), String> {
    use std::sync::mpsc;
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (state_tx, state_rx) = mpsc::channel::<bool>();
    std::thread::Builder::new()
        .name("gdpi-tray".into())
        .spawn(move || {
            if let Err(e) = gtk::init() {
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
            let tray = match Tray::build(on) {
                Ok(t) => t,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                let mut last = None;
                while let Ok(s) = state_rx.try_recv() {
                    last = Some(s);
                }
                if let Some(s) = last {
                    tray.set(s);
                }
                gtk::glib::ControlFlow::Continue
            });
            gtk::main();
        })
        .map_err(|e| e.to_string())?;
    match ready_rx.recv() {
        Ok(Ok(())) => {
            let _ = STATE_TX.set(std::sync::Mutex::new(state_tx));
            install_handlers(handler);
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("GTK başlatılamadı".into()),
    }
}

#[cfg(target_os = "linux")]
pub fn set_state(on: bool) {
    if let Some(tx) = STATE_TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(on);
        }
    }
}
