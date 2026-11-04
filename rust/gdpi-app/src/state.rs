//! Uygulamanın arayüzden bağımsız durumu: ayarlar, motor, kayıt. Hem pencere hem tepsi menüsü kullanır.
use crate::engine;
use gdpi_core::{Backend, Config, LogFn, Stats};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Sites,
    Test,
    Settings,
    Log,
}

/// Pencere açıksa yeni kayıt/durum için yeniden çizim ister.
pub static CTX: OnceLock<eframe::egui::Context> = OnceLock::new();

pub fn repaint() {
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

/// "%H:%M:%S  msg" biçiminde kayıt; stdout'a da yazar.
pub fn make_logger(buf: Arc<Mutex<Vec<String>>>) -> LogFn {
    Arc::new(move |msg: &str| {
        let line = format!("{}  {}", chrono::Local::now().format("%H:%M:%S"), msg);
        println!("{line}");
        if let Ok(mut b) = buf.lock() {
            b.push(line);
        }
        repaint();
    })
}

/// Python `read_ui` içindeki `lines`: boş olmayan satırlar, kırpılmış ve küçük harf.
pub fn parse_lines(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_lowercase).collect()
}

pub struct Core {
    /// Kaydedilmiş/uygulanan ayarlar
    pub cfg: Config,
    /// Arayüzde düzenlenen (henüz okunmamış) ayarlar; targets/bypass metin kutularında
    pub draft: Config,
    pub t_targets: String,
    pub t_bypass: String,
    pub auto: bool,
    pub engine: Option<Box<dyn Backend>>,
    pub stats: Arc<Stats>,
    pub started_at: Option<Instant>,
    /// Açılamadı mesajı (ana ekranda kırmızı)
    pub error: Option<String>,
    pub page: Page,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub log: LogFn,
    pub tray: bool,
}

impl Core {
    pub fn new() -> Core {
        let cfg = Config::load();
        let logs = Arc::new(Mutex::new(Vec::new()));
        Core {
            draft: cfg.clone(),
            t_targets: cfg.targets.join("\n"),
            t_bypass: cfg.bypass.join("\n"),
            cfg,
            auto: crate::autostart::autostart_enabled(),
            engine: None,
            stats: Arc::new(Stats::default()),
            started_at: None,
            error: None,
            page: Page::Home,
            log: make_logger(logs.clone()),
            logs,
            tray: false,
        }
    }

    pub fn log(&self, msg: &str) {
        (self.log)(msg)
    }

    pub fn running(&self) -> bool {
        self.engine.is_some()
    }

    pub fn read_ui(&mut self) {
        let d = &self.draft;
        self.cfg.mode = d.mode;
        self.cfg.fake = d.fake;
        self.cfg.ttl = d.ttl;
        self.cfg.reverse = d.reverse;
        self.cfg.block_quic = d.block_quic;
        self.cfg.dns_redirect = d.dns_redirect;
        self.cfg.targets = parse_lines(&self.t_targets);
        self.cfg.bypass = parse_lines(&self.t_bypass);
        if let Err(e) = self.cfg.save() {
            self.log(&format!("Ayarlar kaydedilemedi: {e}"));
        }
    }

    fn after_state_change(&self) {
        crate::tray::set_state(self.running());
        repaint();
    }

    pub fn toggle(&mut self) {
        if let Some(mut e) = self.engine.take() {
            e.stop();
            self.started_at = None;
            self.log("GDPI kapatıldı.");
            self.after_state_change();
            return;
        }
        self.read_ui();
        self.stats = Arc::new(Stats::default());
        match engine::start(self.cfg.clone(), self.stats.clone(), self.log.clone()) {
            Ok(e) => {
                self.engine = Some(e);
                self.error = None;
                self.started_at = Some(Instant::now());
                self.log("GDPI açıldı.");
                self.after_state_change();
            }
            Err(e) => {
                let msg = e.to_string();
                self.log(&format!("Açılamadı: {msg}"));
                self.page = Page::Home; // hatayı Kayıt'ta saklama, ana ekranda göster
                self.error = Some(msg);
                repaint();
            }
        }
    }

    /// Ayarları kaydeder; motor açıksa yeni ayarlarla yeniden başlatır.
    pub fn apply(&mut self) {
        self.read_ui();
        if self.running() {
            self.toggle();
            self.toggle();
        }
    }

    pub fn apply_preset(&mut self, name: &str) {
        if self.draft.apply_preset(name) {
            self.log(&format!("{name} profiline geçildi."));
            self.apply();
        }
    }

    pub fn toggle_autostart(&mut self, on: bool) {
        self.auto = on;
        match crate::autostart::set_autostart(on) {
            Some(err) => {
                self.auto = false;
                self.log(&err);
            }
            None if on => self.log("Windows ile başlatma açık (Program Files\\GDPI\\GDPI.exe)."),
            None => {}
        }
    }

    /// Motoru durdurur, ayarları kaydeder.
    pub fn shutdown(&mut self) {
        if let Some(mut e) = self.engine.take() {
            e.stop();
        }
        self.read_ui();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_like_read_ui() {
        assert_eq!(
            parse_lines("  Discord.COM \n\n\t\nx.com\r\n  roblox.com"),
            vec!["discord.com", "x.com", "roblox.com"]
        );
        assert!(parse_lines("   \n").is_empty());
    }
}
