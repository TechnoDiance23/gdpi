//! Site testi (Python `site_check`): DNS + TLS el sıkışması dener.
use std::io::{self, ErrorKind};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("tls")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

/// `socket.gethostbyname`: ilk IPv4 adresi.
fn resolve(host: &str) -> Option<std::net::Ipv4Addr> {
    (host, 443).to_socket_addrs().ok()?.find_map(|a| match a.ip() {
        IpAddr::V4(v) => Some(v),
        _ => None,
    })
}

/// OS hata metni ("... (os error 111)" eki olmadan), Python `e.strerror` gibi.
fn strerror(e: &io::Error) -> String {
    let s = e.to_string();
    match s.rfind(" (os error ") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

enum Fail {
    Io(io::Error),
    Tls(rustls::Error),
}

fn handshake(ip: std::net::Ipv4Addr, host: &str, timeout: Duration) -> Result<(), Fail> {
    let mut sock = TcpStream::connect_timeout(&SocketAddr::new(IpAddr::V4(ip), 443), timeout).map_err(Fail::Io)?;
    sock.set_read_timeout(Some(timeout)).map_err(Fail::Io)?;
    sock.set_write_timeout(Some(timeout)).map_err(Fail::Io)?;
    let name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| Fail::Tls(rustls::Error::General("ad".into())))?;
    let mut conn = rustls::ClientConnection::new(tls_config(), name).map_err(Fail::Tls)?;
    while conn.is_handshaking() {
        if let Err(e) = conn.complete_io(&mut sock) {
            // rustls hataları io::Error(InvalidData) içinde gelir
            if e.kind() == ErrorKind::InvalidData {
                if let Some(inner) = e.get_ref().and_then(|r| r.downcast_ref::<rustls::Error>()) {
                    return Err(Fail::Tls(inner.clone()));
                }
            }
            return Err(Fail::Io(e));
        }
    }
    Ok(())
}

pub fn site_check(host: &str) -> (bool, String) {
    site_check_timeout(host, Duration::from_secs(6))
}

pub fn site_check_timeout(host: &str, timeout: Duration) -> (bool, String) {
    let t0 = Instant::now();
    let Some(ip) = resolve(host) else {
        return (false, "Adres bulunamadı (DNS)".into());
    };
    let ips = ip.to_string();
    if ips.starts_with("195.175.254.") || ips.starts_with("193.192.98.") || ips == "127.0.0.1" || ips == "0.0.0.0" {
        return (false, format!("DNS engellenmiş ({ips}). Ayarlar'dan DNS yönlendirmeyi aç."));
    }
    match handshake(ip, host, timeout) {
        Ok(()) => (true, format!("Açılıyor ({} ms)", t0.elapsed().as_millis())),
        Err(Fail::Tls(rustls::Error::InvalidCertificate(_))) => {
            (false, "Sertifika tutmuyor, başka yere yönlendiriliyor olabilir".into())
        }
        Err(Fail::Tls(_)) => (false, "Site bağlantıyı kabul etmedi (engel ya da sitenin kendi koruması)".into()),
        Err(Fail::Io(e)) => match e.kind() {
            ErrorKind::ConnectionReset => (false, "Bağlantı kesildi, muhtemelen engelli".into()),
            // Python: el sıkışmada karşı taraf kapatırsa SSLEOFError (bir SSLError)
            ErrorKind::UnexpectedEof => {
                (false, "Site bağlantıyı kabul etmedi (engel ya da sitenin kendi koruması)".into())
            }
            ErrorKind::TimedOut | ErrorKind::WouldBlock => (false, "Cevap gelmedi".into()),
            _ => (false, format!("Hata: {}", strerror(&e).chars().take(50).collect::<String>())),
        },
    }
}

/// Python `repr(str)`.
pub fn py_str_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::with_capacity(s.len() + 2);
    o.push(q);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == q => {
                o.push('\\');
                o.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push(q);
    o
}

/// Python `repr((ok, msg))`.
pub fn py_repr(r: &(bool, String)) -> String {
    format!("({}, {})", if r.0 { "True" } else { "False" }, py_str_repr(&r.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repr_like_python() {
        assert_eq!(py_repr(&(true, "Açılıyor (123 ms)".into())), "(True, 'Açılıyor (123 ms)')");
        assert_eq!(
            py_repr(&(false, "DNS engellenmiş (1.2.3.4). Ayarlar'dan DNS yönlendirmeyi aç.".into())),
            "(False, \"DNS engellenmiş (1.2.3.4). Ayarlar'dan DNS yönlendirmeyi aç.\")"
        );
        assert_eq!(py_str_repr("a'b\"c\\\n"), "'a\\'b\"c\\\\\\n'");
    }

    #[test]
    fn strerror_strips_code() {
        let e = io::Error::from_raw_os_error(111);
        assert!(!strerror(&e).contains("os error"));
    }

    #[test]
    fn blocked_dns() {
        assert!(!site_check("localhost").0);
    }
}
