//! Ayarlar: Python'daki `DEFAULTS` ve `sanitize_cfg` ile aynı şema, aynı izin listeleri.
//! Ayar dosyası yöneticisiz yazılabilir; yönetici olarak çalışan motor ona körü körüne güvenmez.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Dokunma listesi hariç her site
    Hepsi,
    /// Sadece hedef listesi
    Liste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fake {
    Checksum,
    Ttl,
    Kapali,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Hepsi => "hepsi",
            Mode::Liste => "liste",
        }
    }
}

impl Fake {
    pub fn as_str(self) -> &'static str {
        match self {
            Fake::Checksum => "checksum",
            Fake::Ttl => "ttl",
            Fake::Kapali => "kapali",
        }
    }
}

/// Alan sırası Python'daki DEFAULTS ile aynı (kaydedilen JSON aynı görünsün).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub mode: Mode,
    pub fake: Fake,
    pub ttl: u8,
    pub reverse: bool,
    pub block_quic: bool,
    pub dns_redirect: bool,
    pub dns_addr: String,
    pub dns_port: u16,
    pub targets: Vec<String>,
    pub bypass: Vec<String>,
}

pub const DEFAULT_TARGETS: &[&str] = &[
    "discord.com", "discord.gg", "discordapp.com", "discordapp.net",
    "discord.media", "roblox.com", "wattpad.com", "pastebin.com",
];

/// Asla dokunulmayan siteler: bankalar, devlet, X + Grok (tarayıcı ve PC'deki Grok Bot uygulaması).
pub const DEFAULT_BYPASS: &[&str] = &[
    "isbank.com.tr", "gov.tr", "turkiye.gov.tr", "resmigazete.gov.tr",
    "x.ai", "grok.com", "x.com", "twimg.com", "statsig.com", "statsigapi.net",
    "sentry.io", "mcp.cloudflare.com", "challenges.cloudflare.com", "googleapis.com",
    "garantibbva.com.tr", "akbank.com", "yapikredi.com.tr", "ziraatbank.com.tr",
    "vakifbank.com.tr", "halkbank.com.tr", "qnb.com.tr", "denizbank.com",
    "enpara.com", "papara.com", "epicgames.com",
];

pub const DNS_SERVERS: &[&str] = &["77.88.8.8", "77.88.8.1", "1.1.1.1", "1.0.0.1", "8.8.8.8", "8.8.4.4", "9.9.9.9"];

/// Hızlı profiller: (ad, mod, sahte paket, ters, QUIC engeli)
pub const PRESETS: &[(&str, Mode, Fake, bool, bool)] = &[
    ("Hafif", Mode::Liste, Fake::Kapali, false, true),
    ("Dengeli", Mode::Hepsi, Fake::Checksum, false, true),
    ("Agresif", Mode::Hepsi, Fake::Ttl, true, true),
];

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Hepsi,
            fake: Fake::Checksum,
            ttl: 4,
            reverse: false,
            block_quic: true,
            dns_redirect: false,
            dns_addr: "77.88.8.8".into(),
            dns_port: 1253,
            targets: DEFAULT_TARGETS.iter().map(|s| s.to_string()).collect(),
            bypass: DEFAULT_BYPASS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Python: `re.compile(r"^[a-z0-9-]+(\.[a-z0-9-]+)*$").match(d)`.
/// Python'da `$` sondaki tek bir "\n"den önce de eşleşir; aynı sonucu vermek için bu da kabul edilir.
pub fn domain_ok(d: &str) -> bool {
    if d.chars().count() >= 254 {
        return false;
    }
    let d = d.strip_suffix('\n').unwrap_or(d);
    !d.is_empty()
        && d.split('.').all(|l| !l.is_empty() && l.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'))
}

/// Python'da bool da int sayılır (True == 1).
fn as_int(v: &Value) -> Option<i64> {
    match v {
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|_| i64::MAX)),
        _ => None,
    }
}

impl Config {
    /// Python `sanitize_cfg`: geçersiz her alan varsayılana döner.
    pub fn sanitize(raw: &Value) -> Config {
        let mut c = Config::default();
        let Some(m) = raw.as_object() else { return c };
        let s = |k: &str| m.get(k).and_then(Value::as_str);
        match s("mode") {
            Some("hepsi") => c.mode = Mode::Hepsi,
            Some("liste") => c.mode = Mode::Liste,
            _ => {}
        }
        match s("fake") {
            Some("checksum") => c.fake = Fake::Checksum,
            Some("ttl") => c.fake = Fake::Ttl,
            Some("kapali") => c.fake = Fake::Kapali,
            _ => {}
        }
        if let Some(t) = m.get("ttl").and_then(as_int).filter(|t| (1..=32).contains(t)) {
            c.ttl = t as u8;
        }
        for (k, dst) in [("reverse", &mut c.reverse), ("block_quic", &mut c.block_quic), ("dns_redirect", &mut c.dns_redirect)] {
            if let Some(b) = m.get(k).and_then(Value::as_bool) {
                *dst = b;
            }
        }
        if let Some(a) = s("dns_addr").filter(|a| DNS_SERVERS.contains(a)) {
            c.dns_addr = a.to_string();
        }
        // Python: isinstance(v, int) and v in (53, 1253, 5353) -> True(=1) geçmez
        if let Some(p) = m.get("dns_port").and_then(as_int).filter(|p| [53, 1253, 5353].contains(p)) {
            c.dns_port = p as u16;
        }
        for (k, dst) in [("targets", &mut c.targets), ("bypass", &mut c.bypass)] {
            if let Some(Value::Array(a)) = m.get(k) {
                if a.iter().all(|d| d.as_str().is_some_and(domain_ok)) {
                    *dst = a.iter().filter_map(|d| d.as_str().map(String::from)).collect();
                }
            }
        }
        c
    }

    pub fn from_json(text: &str) -> Config {
        serde_json::from_str::<Value>(text).map(|v| Config::sanitize(&v)).unwrap_or_default()
    }

    /// Python `json.dump(cfg, ensure_ascii=False, indent=2)` ile aynı biçim.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn apply_preset(&mut self, name: &str) -> bool {
        match PRESETS.iter().find(|p| p.0 == name) {
            Some(&(_, mode, fake, reverse, quic)) => {
                self.mode = mode;
                self.fake = fake;
                self.reverse = reverse;
                self.block_quic = quic;
                true
            }
            None => false,
        }
    }

    /// Ayar dosyası: Windows'ta %APPDATA%\GDPI, Linux'ta $XDG_CONFIG_HOME/GDPI ya da ~/.config/GDPI.
    pub fn data_dir() -> std::path::PathBuf {
        use std::path::PathBuf;
        let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
        if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("GDPI")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".config"))
                .join("GDPI")
        }
    }

    pub fn path() -> std::path::PathBuf {
        Self::data_dir().join("ayarlar.json")
    }

    pub fn load() -> Config {
        std::fs::read_to_string(Self::path()).map(|t| Config::from_json(&t)).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(Self::data_dir())?;
        std::fs::write(Self::path(), self.to_json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_never_dropped() {
        let c = Config::default();
        for d in ["isbank.com.tr", "gov.tr", "resmigazete.gov.tr", "x.ai", "grok.com", "x.com", "twimg.com",
                  "statsig.com", "sentry.io", "mcp.cloudflare.com", "challenges.cloudflare.com", "googleapis.com"] {
            assert!(c.bypass.iter().any(|b| b == d), "{d}");
        }
    }

    #[test]
    fn sanitize_rejects() {
        let c = Config::from_json(r#"{"mode":"x","ttl":99,"dns_addr":"6.6.6.6","bypass":["EVIL.com"],"fake":"ttl","dns_port":true}"#);
        assert_eq!(c.mode, Mode::Hepsi);
        assert_eq!(c.ttl, 4);
        assert_eq!(c.dns_addr, "77.88.8.8");
        assert_eq!(c.bypass, Config::default().bypass);
        assert_eq!(c.fake, Fake::Ttl);
        assert_eq!(c.dns_port, 1253);
        assert_eq!(Config::from_json(r#"{"ttl":true}"#).ttl, 1);
        assert_eq!(Config::from_json("[1]"), Config::default());
    }

    #[test]
    fn roundtrip() {
        let c = Config::default();
        assert_eq!(Config::from_json(&c.to_json()), c);
    }
}
