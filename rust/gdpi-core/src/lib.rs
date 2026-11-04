//! GDPI çekirdeği: paket ayrıştırma, bölme kararı, ayarlar.
//! Güvenilmeyen ağ verisini işler; `unsafe` tamamen yasak. Python sürümündeki (`gdpi.py`)
//! fonksiyonların birebir karşılığıdır, farklar fark testleriyle (tests/differential.rs) yakalanır.
#![forbid(unsafe_code)]

pub mod checksum;
pub mod config;
pub mod engine;
pub mod parse;
pub mod pkt;

pub use config::{Config, Fake, Mode, Split, DEFAULT_BYPASS, DEFAULT_TARGETS, DNS_SERVERS, PRESETS};
pub use engine::{filter_str, nft_rules, Engine, LogFn, Out, Stats, Verdict, NFQ_MARK, NFQ_NUM, NFT_TABLE};
pub use parse::{domain_in, find_http_host, find_sni, host_text};
pub use pkt::Pkt;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP: &str = "GDPI - Görüşürüz Derin Paket İnceleme";

/// Motor başlatılamadığında arayüzde kırmızıyla gösterilen hata.
#[derive(Debug, Clone)]
pub struct EngineError {
    pub code: i64,
    pub msg: String,
}

impl EngineError {
    pub fn new(code: i64, msg: impl Into<String>) -> Self {
        Self { code, msg: msg.into() }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for EngineError {}

/// Çalışan bir motor (WinDivert ya da NFQUEUE). `stop` çağrılınca ağ eski haline döner.
pub trait Backend: Send {
    fn stop(&mut self);
}
