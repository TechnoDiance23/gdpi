//! Platforma göre motor seçimi: Windows'ta WinDivert, Linux'ta NFQUEUE.
use gdpi_core::{Backend, Config, EngineError, LogFn, Stats};
use std::sync::Arc;

#[cfg(windows)]
use gdpi_windows as imp;

#[cfg(target_os = "linux")]
use gdpi_linux as imp;

pub fn start(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    imp::start(cfg, stats, log)
}

pub fn is_admin() -> bool {
    imp::is_admin()
}

/// Kullanıcıya "yönetici izni yok" uyarısında gösterilen başlatıcı.
pub const LAUNCHER: &str = if cfg!(windows) { "GDPI.bat" } else { "./gdpi.sh" };
