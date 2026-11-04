//! Linux motoru: nftables + NFQUEUE (Python `NFQueue`, `nft*`, `Engine.loop`).
//! Netlink iletileri burada elle (sınır denetimli) kurulur/çözülür; soket işleri `nix` ile yapılır.
#![forbid(unsafe_code)]

use gdpi_core::{Backend, Config, EngineError, LogFn, Stats};
use std::sync::Arc;

pub mod nl;

/// Python `nft_rules` ile aynı; conntrack ifadesi desteklenmezse kullanılan yedek.
pub fn nft_rules_fallback(rules: &str) -> String {
    rules.replace(" ct original packets < 12", "")
}

#[cfg(target_os = "linux")]
mod linux;

/// Kuralları kurmadan yalnızca NFQUEUE'ya bağlanıp paket iş parçacığını açar (testler için).
#[doc(hidden)]
#[cfg(target_os = "linux")]
pub fn start_queue_only(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    linux::spawn(cfg, stats, log)
}

/// Euid 0 mı?
pub fn is_admin() -> bool {
    #[cfg(target_os = "linux")]
    {
        nix::unistd::geteuid().is_root()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Motoru başlatır: kuralları kurar, kuyruğa bağlanır, paket iş parçacığını açar.
pub fn start(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    #[cfg(target_os = "linux")]
    {
        linux::start(cfg, stats, log)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cfg, stats, log);
        Err(EngineError::new(1, "Bu motor yalnızca Linux'ta çalışır."))
    }
}
