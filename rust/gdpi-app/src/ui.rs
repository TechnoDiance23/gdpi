//! egui arayüzü (Python `App` + `PowerButton`).
use crate::state::{Core, Page, CTX};
use crate::tray::{self, Cmd};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder,
};
use gdpi_core::{Fake, Mode, APP, PRESETS, VERSION};
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------ renkler (Python C)
const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
const HEADER: Color32 = hex(0x0b2a6b);
const BG: Color32 = hex(0x0f141b);
const SIDE: Color32 = hex(0x0a0e13);
const CARD: Color32 = hex(0x161d27);
const CARD2: Color32 = hex(0x1c2532);
const LINE: Color32 = hex(0x243042);
const TEXT: Color32 = hex(0xe6edf6);
const MUTE: Color32 = hex(0x8a98ab);
const BLUE: Color32 = hex(0x2f80ff);
const BLUE_HOVER: Color32 = hex(0x2563eb);
const GREEN: Color32 = hex(0x22c55e);
const RED: Color32 = hex(0xef4444);
const SEG_BG: Color32 = hex(0x4a4d50);
const LOG_GREEN: Color32 = hex(0x86efac);

fn sk(w: f32, c: Color32) -> Stroke {
    Stroke::new(w, c)
}
fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
fn bfont(size: f32) -> FontId {
    FontId::new(size, bold())
}
fn btxt(s: impl Into<String>, size: f32, c: Color32) -> RichText {
    RichText::new(s).family(bold()).size(size).color(c)
}
fn txt(s: impl Into<String>, size: f32, c: Color32) -> RichText {
    RichText::new(s).size(size).color(c)
}

fn setup_fonts(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    f.font_data.insert("dejavu".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf"))));
    f.font_data.insert("dejavu-b".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf"))));
    f.font_data.insert("dejavu-m".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/DejaVuSansMono.ttf"))));
    let prop = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut b = vec!["dejavu-b".to_string()];
    b.extend(prop.iter().cloned());
    f.families.insert(bold(), b);
    f.families.entry(FontFamily::Proportional).or_default().insert(0, "dejavu".into());
    f.families.entry(FontFamily::Monospace).or_default().insert(0, "dejavu-m".into());
    ctx.set_fonts(f);
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut(|s| {
        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.extreme_bg_color = BG;
        v.override_text_color = None;
        v.selection.bg_fill = BLUE.gamma_multiply(0.6);
        v.text_cursor.stroke = sk(2.0, TEXT);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::same(6);
            w.fg_stroke.color = TEXT;
        }
        v.widgets.inactive.weak_bg_fill = CARD2;
        v.widgets.inactive.bg_fill = CARD2;
        v.widgets.hovered.weak_bg_fill = LINE;
        v.widgets.hovered.bg_fill = LINE;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        v.widgets.active.weak_bg_fill = LINE;
        v.widgets.open.weak_bg_fill = LINE;
        v.widgets.noninteractive.bg_stroke = sk(1.0, LINE);
        s.spacing.scroll = egui::style::ScrollStyle::solid();
    });
}

// ------------------------------------------------------------------ uygulama
struct TestRow {
    host: String,
    result: Arc<Mutex<Option<(bool, String)>>>,
}

pub struct App {
    core: Arc<Mutex<Core>>,
    logo: egui::TextureHandle,
    test_entry: String,
    tests: Vec<TestRow>,
    preset: Option<usize>,
    started: std::time::Instant,
    cmd_rx: std::sync::mpsc::Receiver<Cmd>,
}

pub fn run(tray_start: bool) -> Result<(), String> {
    let icon = egui::IconData { rgba: crate::shield::shield_rgba(true, 64), width: 64, height: 64 };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP)
            .with_app_id("gdpi")
            .with_inner_size([900.0, 580.0])
            .with_min_inner_size([780.0, 520.0])
            .with_icon(Arc::new(icon))
            .with_visible(!tray_start),
        ..Default::default()
    };
    eframe::run_native(APP, opts, Box::new(move |cc| Ok(Box::new(App::new(cc, tray_start)))))
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn win32_show() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};
    let title: Vec<u16> = APP.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: başlık sıfırla biten geçerli bir UTF-16 dizisi; dönen pencere tanıtıcısı sadece bu çağrılarda kullanılır.
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, tray_start: bool) -> App {
        let ctx = cc.egui_ctx.clone();
        let _ = CTX.set(ctx.clone());
        setup_fonts(&ctx);
        setup_style(&ctx);
        let img = egui::ColorImage::from_rgba_unmultiplied([64, 64], &crate::shield::shield_rgba(true, 64));
        let logo = ctx.load_texture("logo", img, egui::TextureOptions::LINEAR);
        let core = Arc::new(Mutex::new(Core::new()));

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let ctx2 = ctx.clone();
        let handler = Box::new(move |cmd: Cmd| {
            let _ = cmd_tx.send(cmd);
            ctx2.request_repaint();
        });
        {
            let mut c = core.lock().unwrap();
            match tray::init(false, handler) {
                Ok(()) => c.tray = true,
                Err(e) => c.log(&format!("Tepsi simgesi kullanılamıyor: {e}")),
            }
            if !crate::engine::is_admin() {
                c.log(&format!("Yönetici izni yok — {} ile açın.", crate::engine::LAUNCHER));
            }
            if tray_start {
                c.toggle();
            }
        }
        App { core, logo, test_entry: String::new(), tests: Vec::new(), preset: None, started: std::time::Instant::now(), cmd_rx }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                Cmd::Show => {
                    #[cfg(windows)]
                    win32_show();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Cmd::Toggle => {
                    let mut c = self.core.lock().unwrap();
                    c.toggle();
                }
                Cmd::Quit => {
                    let mut c = self.core.lock().unwrap();
                    c.shutdown();
                    std::process::exit(0);
                }
            }
        }

        let core = self.core.clone();
        let mut c = core.lock().unwrap();

        // kapatma ve simge durumuna küçültme tepsiye gizler
        let (close, minimized) = ctx.input(|i| (i.viewport().close_requested(), i.viewport().minimized == Some(true)));
        if close {
            if c.tray {
                c.read_ui();
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                c.shutdown();
            }
        } else if minimized && c.tray {
            c.read_ui();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.header(ctx);
        self.statusbar(ctx, &c);
        self.sidebar(ctx, &mut c);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| match c.page {
                Page::Home => self.page_home(ui, &mut c),
                Page::Sites => self.page_sites(ui, &mut c),
                Page::Test => self.page_test(ui, &c),
                Page::Settings => self.page_settings(ui, &mut c),
                Page::Log => self.page_log(ui, &mut c),
            });

        let next = if c.running() { 40 } else { 500 };
        ctx.request_repaint_after(std::time::Duration::from_millis(next));
    }
}

// ------------------------------------------------------------------ çizim yardımcıları
fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(sk(1.0, LINE))
        .corner_radius(CornerRadius::same(12))
}

fn paint_card(ui: &Ui, r: Rect) {
    ui.painter().rect(r, CornerRadius::same(12), CARD, sk(1.0, LINE), StrokeKind::Inside);
}

fn button(ui: &mut Ui, label: &str, size: [f32; 2], fill: Color32, hover: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(size[0], size[1]), Sense::click());
    let col = if resp.hovered() { hover } else { fill };
    ui.painter().rect_filled(rect, CornerRadius::same(6), col);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font(13.0), Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// CTkSegmentedButton benzeri; tıklanan seçeneğin sırasını döndürür.
fn segmented(ui: &mut Ui, opts: &[&str], sel: Option<usize>) -> Option<usize> {
    let f = font(13.0);
    let widths: Vec<f32> = opts
        .iter()
        .map(|o| ui.fonts(|fs| fs.layout_no_wrap(o.to_string(), f.clone(), TEXT).size().x) + 12.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(6), SEG_BG);
    let mut x = rect.left() + 3.0;
    let mut clicked = None;
    for (i, (o, w)) in opts.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(*w, rect.height() - 6.0));
        let resp = ui.interact(r, ui.id().with(("seg", o, i)), Sense::click());
        let on = sel == Some(i);
        if on || resp.hovered() {
            let col = if on { if resp.hovered() { BLUE_HOVER } else { BLUE } } else { hex(0x5e6266) };
            ui.painter().rect_filled(r, CornerRadius::same(4), col);
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *o, f.clone(), Color32::WHITE);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
        x += w;
    }
    clicked
}

/// CTkSwitch benzeri anahtar; değiştiyse true.
fn switch(ui: &mut Ui, on: &mut bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 18.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    let track = rect.shrink2(vec2(0.0, 2.0));
    p.rect_filled(track, CornerRadius::same(7), if *on { BLUE } else { SEG_BG });
    let x = egui::lerp(track.left() + 8.0..=track.right() - 8.0, t);
    p.circle_filled(pos2(x, rect.center().y), 9.0, hex(0xd5d9de));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn dot(ui: &mut Ui, c: Color32, r: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(r * 2.0 + 2.0, r * 2.0 + 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), r, c);
}

fn arc_points(c: Pos2, r: f32, from_deg: f32, to_deg: f32) -> Vec<Pos2> {
    let n = 48;
    (0..=n)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / n as f32).to_radians();
            pos2(c.x + r * a.cos(), c.y - r * a.sin())
        })
        .collect()
}

/// Kenar çubuğu simgeleri (⏻ 🌐 🔎 ⚙ ☰) her yazı tipinde bulunmadığı için çizilir.
fn nav_icon(p: &egui::Painter, page: Page, c: Pos2, col: Color32) {
    let s = sk(1.6, col);
    match page {
        Page::Home => {
            p.add(egui::Shape::line(arc_points(c, 6.0, 120.0, 420.0), s));
            p.line_segment([c + vec2(0.0, -8.0), c + vec2(0.0, -1.5)], s);
        }
        Page::Sites => {
            p.circle_stroke(c, 7.0, s);
            p.line_segment([c + vec2(-7.0, 0.0), c + vec2(7.0, 0.0)], s);
            let meridian: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                    pos2(c.x + 3.2 * a.cos(), c.y + 7.0 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(meridian, s));
            p.line_segment([c + vec2(-6.0, -3.5), c + vec2(6.0, -3.5)], sk(1.0, col));
            p.line_segment([c + vec2(-6.0, 3.5), c + vec2(6.0, 3.5)], sk(1.0, col));
        }
        Page::Test => {
            let m = c + vec2(-1.5, -1.5);
            p.circle_stroke(m, 5.0, sk(2.0, col));
            p.line_segment([m + vec2(3.6, 3.6), m + vec2(8.0, 8.0)], sk(2.6, col));
        }
        Page::Settings => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 4.5, c + d * 7.5], sk(2.2, col));
            }
            p.circle_stroke(c, 4.8, sk(1.8, col));
            p.circle_filled(c, 1.6, col);
        }
        Page::Log => {
            for dy in [-5.0, 0.0, 5.0] {
                p.line_segment([c + vec2(-7.0, dy), c + vec2(7.0, dy)], s);
            }
        }
    }
}

/// Ortadaki büyük VPN düğmesi; açıkken nabız gibi parlar (Python `PowerButton`).
fn power_button(ui: &mut Ui, on: bool, phase: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(220.0, 220.0), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    let (ring, core, icon) = if on {
        let pulse = (phase.sin() + 1.0) / 2.0;
        for (i, col) in [hex(0x0d2a1f), hex(0x10372a), hex(0x134634)].into_iter().enumerate() {
            p.circle_filled(c, 104.0 - i as f32 * 10.0 + pulse * 4.0, col);
        }
        (GREEN, hex(0x0f2a1f), GREEN)
    } else {
        for (i, col) in [hex(0x121922), hex(0x151d28)].into_iter().enumerate() {
            p.circle_filled(c, 100.0 - i as f32 * 10.0, col);
        }
        (hex(0x334155), hex(0x111823), hex(0x94a3b8))
    };
    p.circle_filled(c, 70.0, core);
    p.circle_stroke(c, 68.0, sk(4.0, ring));
    p.add(egui::Shape::line(arc_points(c, 30.0, 120.0, 420.0), sk(7.0, icon)));
    // yuvarlak uçlu dikey çizgi
    p.line_segment([c + vec2(0.0, -40.0), c + vec2(0.0, -6.0)], sk(7.0, icon));
    p.circle_filled(c + vec2(0.0, -40.0), 3.5, icon);
    p.circle_filled(c + vec2(0.0, -6.0), 3.5, icon);
    for a in [120.0f32, 60.0] {
        let r = a.to_radians();
        p.circle_filled(pos2(c.x + 30.0 * r.cos(), c.y - 30.0 * r.sin()), 3.5, icon);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn page_title(ui: &mut Ui, title: &str) -> Rect {
    let r = ui.max_rect();
    let tr = Rect::from_min_size(r.min + vec2(24.0, 20.0), vec2(r.width() - 48.0, 40.0));
    ui.painter().text(tr.left_center(), Align2::LEFT_CENTER, title, bfont(22.0), TEXT);
    tr
}

// ------------------------------------------------------------------ bölümler
impl App {
    fn header(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(HEADER))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                let p = ui.painter();
                let cy = r.center().y;
                let logo = Rect::from_center_size(pos2(r.left() + 18.0 + 15.0, cy), vec2(30.0, 30.0));
                p.image(self.logo.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                let g = p.text(pos2(logo.right() + 8.0, cy), Align2::LEFT_CENTER, "GDPI", bfont(22.0), Color32::WHITE);
                let pill_g = ui.fonts(|f| f.layout_no_wrap(" KİŞİSEL ".into(), bfont(10.0), hex(0xcbd5e1)));
                let pr = Rect::from_min_size(pos2(g.right() + 10.0, cy - 15.0), vec2(pill_g.size().x + 20.0, 30.0));
                p.rect_filled(pr, CornerRadius::same(10), hex(0x1e3a7a));
                p.text(pr.center(), Align2::CENTER_CENTER, " KİŞİSEL ", bfont(10.0), hex(0xcbd5e1));
                p.text(
                    pos2(r.right() - 18.0, cy),
                    Align2::RIGHT_CENTER,
                    "Görüşürüz Derin Paket İnceleme",
                    font(12.0),
                    hex(0x93a4c3),
                );
            });
    }

    fn statusbar(&self, ctx: &egui::Context, c: &Core) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(SIDE))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                let p = ui.painter();
                let cy = r.center().y;
                let on = c.running();
                p.circle_filled(pos2(r.left() + 20.0, cy), 3.5, if on { GREEN } else { RED });
                p.text(pos2(r.left() + 28.0, cy), Align2::LEFT_CENTER, if on { "Çalışıyor" } else { "Kapalı" }, font(12.0), MUTE);
                let right = format!("Sahte paket: {}   ·   {} site korumada", c.cfg.fake.as_str(), c.cfg.bypass.len());
                p.text(pos2(r.right() - 16.0, cy), Align2::RIGHT_CENTER, right, font(12.0), MUTE);
            });
    }

    fn sidebar(&self, ctx: &egui::Context, c: &mut Core) {
        egui::SidePanel::left("side")
            .exact_width(200.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(SIDE))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                let items = [
                    (Page::Home, "Bağlantı"),
                    (Page::Sites, "Siteler"),
                    (Page::Test, "Site Testi"),
                    (Page::Settings, "Ayarlar"),
                    (Page::Log, "Kayıt"),
                ];
                let mut y = r.top() + 14.0;
                for (page, label) in items {
                    let br = Rect::from_min_size(pos2(r.left() + 12.0, y), vec2(r.width() - 24.0, 40.0));
                    let resp = ui.interact(br, ui.id().with(label), Sense::click());
                    let sel = c.page == page;
                    if sel || resp.hovered() {
                        ui.painter().rect_filled(br, CornerRadius::same(8), CARD2);
                    }
                    let col = if sel { Color32::WHITE } else { MUTE };
                    nav_icon(ui.painter(), page, pos2(br.left() + 20.0, br.center().y), col);
                    ui.painter().text(pos2(br.left() + 40.0, br.center().y), Align2::LEFT_CENTER, label, font(14.0), col);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        c.page = page;
                    }
                    y += 44.0;
                }
                ui.painter().text(
                    pos2(r.center().x, r.bottom() - 18.0),
                    Align2::CENTER_CENTER,
                    format!("v{VERSION}"),
                    font(11.0),
                    hex(0x4b5563),
                );
            });
    }

    // ---------- Bağlantı
    fn page_home(&mut self, ui: &mut Ui, c: &mut Core) {
        let on = c.running();
        let full = ui.max_rect();
        let phase = self.started.elapsed().as_secs_f32() * 2.0;
        ui.add_space(34.0);
        ui.vertical_centered(|ui| {
            if power_button(ui, on, phase) {
                c.toggle();
            }
        });
        let on = c.running();
        ui.add_space(6.0);
        let (title, tcol, sub, scol) = match (&c.error, on) {
            (_, true) => ("Açık".to_string(), GREEN, "Engeller aşılıyor. Bankalara ve devlet sitelerine dokunulmuyor.".to_string(), MUTE),
            (Some(e), false) => ("Açılamadı".to_string(), RED, e.clone(), RED),
            (None, false) => ("Şu an kapalı".to_string(), TEXT, "Açmak için ortadaki düğmeye bas".to_string(), MUTE),
        };
        ui.vertical_centered(|ui| {
            ui.label(btxt(title, 24.0, tcol));
            ui.add_space(2.0);
            ui.add(egui::Label::new(txt(sub, 13.0, scol)).wrap());
        });
        ui.add_space(22.0);

        // 4 istatistik kartı
        let (row, _) = ui.allocate_exact_size(vec2(full.width(), 76.0), Sense::hover());
        let inner = Rect::from_min_max(pos2(row.left() + 30.0, row.top()), pos2(row.right() - 30.0, row.bottom()));
        let w = inner.width() / 4.0;
        let (time, split, skip) = match c.started_at {
            Some(t) if on => {
                let s = t.elapsed().as_secs();
                (
                    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60),
                    c.stats.split().to_string(),
                    c.stats.skip().to_string(),
                )
            }
            _ => ("—".into(), "—".into(), "—".into()),
        };
        let mode = if c.cfg.mode == Mode::Hepsi { "Her site" } else { "Liste" };
        for (i, (label, val)) in [("Süre", time), ("Aşılan", split), ("Dokunulmayan", skip), ("Mod", mode.to_string())]
            .into_iter()
            .enumerate()
        {
            let r = Rect::from_min_size(pos2(inner.left() + w * i as f32 + 6.0, inner.top()), vec2(w - 12.0, 76.0));
            paint_card(ui, r);
            let p = ui.painter();
            p.text(r.min + vec2(14.0, 24.0), Align2::LEFT_CENTER, label, font(11.0), MUTE);
            p.text(r.min + vec2(14.0, 52.0), Align2::LEFT_CENTER, val, bfont(18.0), TEXT);
        }
        ui.add_space(18.0);

        // Hızlı profil
        let names: Vec<&str> = PRESETS.iter().map(|p| p.0).collect();
        let label_w = ui.fonts(|f| f.layout_no_wrap("Hızlı profil:".into(), font(12.0), MUTE).size().x);
        let seg_w: f32 = names
            .iter()
            .map(|n| ui.fonts(|f| f.layout_no_wrap(n.to_string(), font(13.0), TEXT).size().x) + 12.0)
            .sum::<f32>()
            + 6.0;
        let total = label_w + 16.0 + seg_w;
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(((full.width() - total) / 2.0).max(0.0));
            ui.add_space(8.0);
            ui.label(txt("Hızlı profil:", 12.0, MUTE));
            ui.add_space(8.0);
            if let Some(i) = segmented(ui, &names, self.preset) {
                self.preset = Some(i);
                c.apply_preset(names[i]);
            }
        });
    }

    // ---------- Siteler
    fn page_sites(&mut self, ui: &mut Ui, c: &mut Core) {
        let tr = page_title(ui, "Siteler");
        let r = ui.max_rect();
        let top = tr.bottom() + 8.0;
        let area = Rect::from_min_max(pos2(r.left() + 24.0, top), pos2(r.right() - 24.0, r.bottom() - 20.0));
        let w = (area.width() - 16.0) / 2.0;
        let cards = [
            (Rect::from_min_size(area.min, vec2(w, area.height())), "Engelli siteler", "Sadece-liste modunda bunlara uygulanır", BLUE, 0),
            (
                Rect::from_min_size(pos2(area.left() + w + 16.0, area.top()), vec2(w, area.height())),
                "Asla dokunma",
                "Banka, devlet, X / Grok… hiç ellenmez",
                GREEN,
                1,
            ),
        ];
        for (cr, title, sub, accent, which) in cards {
            paint_card(ui, cr);
            let p = ui.painter();
            p.circle_filled(pos2(cr.left() + 20.0, cr.top() + 24.0), 4.5, accent);
            p.text(pos2(cr.left() + 32.0, cr.top() + 24.0), Align2::LEFT_CENTER, title, bfont(15.0), TEXT);
            p.text(pos2(cr.left() + 14.0, cr.top() + 46.0), Align2::LEFT_CENTER, sub, font(11.0), MUTE);
            let er = Rect::from_min_max(pos2(cr.left() + 12.0, cr.top() + 62.0), pos2(cr.right() - 12.0, cr.bottom() - 12.0));
            ui.painter().rect_filled(er, CornerRadius::same(8), BG);
            let text = if which == 0 { &mut c.t_targets } else { &mut c.t_bypass };
            ui.scope_builder(UiBuilder::new().max_rect(er.shrink(6.0)), |ui| {
                egui::ScrollArea::vertical().id_salt(("sites", which)).auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), ui.available_height()],
                        TextEdit::multiline(text)
                            .frame(false)
                            .font(FontId::monospace(12.0))
                            .text_color(TEXT)
                            .desired_rows(4),
                    );
                });
            });
        }
    }

    // ---------- Site testi
    fn page_test(&mut self, ui: &mut Ui, c: &Core) {
        let tr = page_title(ui, "Site Testi");
        let r = ui.max_rect();
        let bar = Rect::from_min_size(pos2(r.left() + 24.0, tr.bottom() + 8.0), vec2(r.width() - 48.0, 62.0));
        paint_card(ui, bar);
        let mut run = false;
        let mut run_all = false;
        ui.scope_builder(UiBuilder::new().max_rect(bar.shrink2(vec2(12.0, 12.0))), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                run_all = button(ui, "Listemdekiler", [130.0, 38.0], CARD2, LINE);
                ui.add_space(0.0);
                run = button(ui, "Test et", [110.0, 38.0], BLUE, BLUE_HOVER);
                ui.add_space(6.0);
                let er = Rect::from_min_max(
                    pos2(bar.left() + 12.0, bar.top() + 12.0),
                    pos2(bar.right() - 12.0 - 130.0 - 6.0 - 110.0 - 12.0, bar.bottom() - 12.0),
                );
                ui.painter().rect_filled(er, CornerRadius::same(6), BG);
                let resp = ui.put(
                    er.shrink2(vec2(10.0, 9.0)),
                    TextEdit::singleline(&mut self.test_entry)
                        .frame(false)
                        .hint_text(txt("ör. discord.com", 13.0, MUTE))
                        .font(font(13.0))
                        .text_color(TEXT),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    run = true;
                    resp.request_focus();
                }
            });
        });
        if run {
            let h = self.test_entry.trim().to_lowercase();
            let h = h.strip_prefix("https://").or_else(|| h.strip_prefix("http://")).unwrap_or(&h);
            let h = h.split('/').next().unwrap_or("").to_string();
            if !h.is_empty() {
                self.start_test(h);
            }
        }
        if run_all {
            for h in c.cfg.targets.clone() {
                self.start_test(h);
            }
        }
        let list = Rect::from_min_max(pos2(r.left() + 14.0, bar.bottom() + 10.0), pos2(r.right() - 14.0, r.bottom() - 10.0));
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for row in &self.tests {
                    let res = row.result.lock().unwrap().clone();
                    let (col, msg) = match &res {
                        None => (MUTE, "test ediliyor…".to_string()),
                        Some((ok, m)) => (if *ok { GREEN } else { RED }, m.clone()),
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        card_frame().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                            ui.set_width(ui.available_width() - 10.0 - 28.0);
                            ui.horizontal(|ui| {
                                dot(ui, if res.is_none() { MUTE } else { col }, 5.0);
                                ui.add_space(4.0);
                                ui.label(btxt(&row.host, 13.0, TEXT));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(txt(msg, 12.0, col));
                                });
                            });
                        });
                    });
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn start_test(&mut self, host: String) {
        let result = Arc::new(Mutex::new(None));
        let (h, r) = (host.clone(), result.clone());
        std::thread::spawn(move || {
            let v = crate::sitecheck::site_check(&h);
            *r.lock().unwrap() = Some(v);
            crate::state::repaint();
        });
        self.tests.push(TestRow { host, result });
    }

    // ---------- Ayarlar
    fn page_settings(&mut self, ui: &mut Ui, c: &mut Core) {
        let tr = page_title(ui, "Ayarlar");
        let br = Rect::from_min_size(pos2(tr.right() - 150.0, tr.center().y - 17.0), vec2(150.0, 34.0));
        let mut apply = false;
        ui.scope_builder(UiBuilder::new().max_rect(br), |ui| {
            apply = button(ui, "Kaydet ve uygula", [150.0, 34.0], BLUE, BLUE_HOVER);
        });
        if apply {
            c.apply();
        }
        let r = ui.max_rect();
        let area = Rect::from_min_max(pos2(r.left() + 24.0, tr.bottom() + 8.0), pos2(r.right() - 14.0, r.bottom()));
        ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mode_sel = Some(if c.draft.mode == Mode::Hepsi { 0 } else { 1 });
                self.setting(ui, "Çalışma modu", "En sorunsuzu “Sadece liste”. “Her site” dokunma listesindekiler dışında her yere uygular", |ui| {
                    if let Some(i) = segmented(ui, &["Her site", "Sadece liste"], mode_sel) {
                        c.draft.mode = if i == 0 { Mode::Hepsi } else { Mode::Liste };
                    }
                });
                let fake_sel = Some(match c.draft.fake {
                    Fake::Ttl => 0,
                    Fake::Checksum => 1,
                    Fake::Kapali => 2,
                });
                self.setting(ui, "Sahte paket", "Düşük TTL çoğu operatörde işe yarıyor. Bozuk checksum bazılarında hiç çalışmıyor", |ui| {
                    if let Some(i) = segmented(ui, &["Düşük TTL", "Bozuk checksum", "Kapalı"], fake_sel) {
                        c.draft.fake = [Fake::Ttl, Fake::Checksum, Fake::Kapali][i];
                    }
                });
                self.setting(ui, "TTL değeri", "Sadece Düşük TTL seçiliyken işe yarar. Çoğu zaman 3 yeterli, olmazsa 4-6 dene", |ui| {
                    egui::ComboBox::from_id_salt("ttl")
                        .width(80.0)
                        .selected_text(txt(c.draft.ttl.to_string(), 13.0, TEXT))
                        .show_ui(ui, |ui| {
                            for i in 1..=12u8 {
                                ui.selectable_value(&mut c.draft.ttl, i, i.to_string());
                            }
                        });
                });
                self.setting(ui, "Parçaları ters gönder", "Bazı operatörlerde fark yaratıyor, açılmayan site varsa dene", |ui| {
                    switch(ui, &mut c.draft.reverse);
                });
                self.setting(ui, "QUIC'i engelle", "Tarayıcıyı bölünebilen normal bağlantıya geçmeye zorlar", |ui| {
                    switch(ui, &mut c.draft.block_quic);
                });
                self.setting(ui, "DNS yönlendir", "Operatörün DNS engelini aşar (Yandex DNS kullanır)", |ui| {
                    switch(ui, &mut c.draft.dns_redirect);
                });
                let mut auto = c.auto;
                let mut changed = false;
                self.setting(ui, "Windows ile başlat", "Bilgisayar açılınca sessizce başlar, izin sormaz", |ui| {
                    changed = switch(ui, &mut auto);
                });
                if changed {
                    c.toggle_autostart(auto);
                }
            });
        });
    }

    fn setting(&self, ui: &mut Ui, title: &str, sub: &str, control: impl FnOnce(&mut Ui)) {
        ui.add_space(5.0);
        card_frame().inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
            ui.set_width(ui.available_width() - 14.0);
            let w = ui.available_width();
            ui.allocate_ui_with_layout(vec2(w, 40.0), Layout::right_to_left(Align::Center), |ui| {
                control(ui);
                ui.add_space(16.0);
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    ui.label(btxt(title, 14.0, TEXT));
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(txt(sub, 11.0, MUTE)).wrap());
                });
            });
        });
        ui.add_space(5.0);
    }

    // ---------- Kayıt
    fn page_log(&mut self, ui: &mut Ui, c: &mut Core) {
        let tr = page_title(ui, "Kayıt");
        let br = Rect::from_min_size(pos2(tr.right() - 188.0, tr.center().y - 16.0), vec2(188.0, 32.0));
        let (mut copy, mut clear) = (false, false);
        ui.scope_builder(UiBuilder::new().max_rect(br).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            copy = button(ui, "Kopyala", [90.0, 32.0], CARD2, LINE);
            clear = button(ui, "Temizle", [90.0, 32.0], CARD2, LINE);
        });
        let mut text = c.logs.lock().unwrap().iter().map(|l| format!("{l}\n")).collect::<String>();
        if copy {
            ui.ctx().copy_text(text.clone());
        }
        if clear {
            c.logs.lock().unwrap().clear();
            text.clear();
        }
        let r = ui.max_rect();
        let box_r = Rect::from_min_max(pos2(r.left() + 24.0, tr.bottom() + 8.0), pos2(r.right() - 24.0, r.bottom() - 20.0));
        paint_card(ui, box_r);
        ui.scope_builder(UiBuilder::new().max_rect(box_r.shrink(10.0)), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.add_sized(
                    [ui.available_width(), ui.available_height()],
                    TextEdit::multiline(&mut text.as_str())
                        .frame(false)
                        .font(FontId::monospace(12.0))
                        .text_color(LOG_GREEN),
                );
            });
        });
    }
}
