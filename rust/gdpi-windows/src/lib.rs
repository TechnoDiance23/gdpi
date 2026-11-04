//! GDPI Windows arka ucu: WinDivert 2.2.2 (çalışma anında yüklenen DLL) + gdpi-core motoru.
#![deny(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

/// WINDIVERT_ADDRESS boyutu (bayt).
pub const ADDR_SIZE: usize = 80;

/// WinDivert açılış hatalarının Türkçe karşılıkları (Python `WD_ERRORS`).
pub fn wd_error_msg(err: u32) -> String {
    match err {
        2 => "WinDivert dosyaları eksik (WinDivert.dll / WinDivert64.sys).".into(),
        5 => "Yönetici izni yok. GDPI'ı yönetici olarak açın.".into(),
        87 => "Filtre geçersiz (iç hata).".into(),
        577 => "Sürücü imzası doğrulanamadı. Windows sürücüyü engelledi.".into(),
        1058 => "WinDivert sürücü servisi devre dışı.".into(),
        1275 => "Windows sürücüyü engelledi (Güvenlik/Bellek bütünlüğü ya da antivirüs).".into(),
        1753 => "Windows Base Filtering Engine (BFE) servisi kapalı.".into(),
        _ => format!("WinDivert açılamadı (Windows hata kodu {err})."),
    }
}

/// Adres yapısından giden-paket bayrağı: bayt 8'deki u32'nin 17. biti.
pub fn addr_outbound(addr: &[u8; ADDR_SIZE]) -> bool {
    let w = u32::from_le_bytes([addr[8], addr[9], addr[10], addr[11]]);
    (w >> 17) & 1 == 1
}

/// Çalışan exe'nin yanındaki `windivert` klasörü.
pub fn windivert_dir() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("windivert")
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{is_admin, start};

#[cfg(not(windows))]
pub fn start(
    _cfg: gdpi_core::Config,
    _stats: std::sync::Arc<gdpi_core::Stats>,
    _log: gdpi_core::LogFn,
) -> Result<Box<dyn gdpi_core::Backend>, gdpi_core::EngineError> {
    Err(gdpi_core::EngineError::new(-1, "WinDivert arka ucu yalnızca Windows'ta çalışır."))
}

#[cfg(not(windows))]
pub fn is_admin() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_bit() {
        let mut a = [0u8; ADDR_SIZE];
        assert!(!addr_outbound(&a));
        a[10] = 0x02; // bit 17
        assert!(addr_outbound(&a));
        a[10] = 0xFD;
        a[8] = 0xFF;
        assert!(!addr_outbound(&a));
    }

    #[test]
    fn errors() {
        assert!(wd_error_msg(5).contains("Yönetici"));
        assert!(wd_error_msg(2).contains("eksik"));
        assert_eq!(wd_error_msg(42), "WinDivert açılamadı (Windows hata kodu 42).");
    }

    #[test]
    fn dir_name() {
        assert!(windivert_dir().ends_with("windivert"));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows() {
        assert!(!is_admin());
        let log: gdpi_core::LogFn = std::sync::Arc::new(|_| {});
        assert!(start(gdpi_core::Config::default(), Default::default(), log).is_err());
    }
}
