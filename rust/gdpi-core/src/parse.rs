//! TLS SNI ve HTTP Host bulma. Python'daki `find_sni` / `find_http_host` ile aynı sonucu verir;
//! bu yüzden Python'un dilim davranışı (kısa dilim = küçük sayı, taşan indeks = None) taklit edilir.

/// Python `int.from_bytes(p[i:i+n], "big")`: dilim taşarsa kısalır, boşsa 0.
fn be(p: &[u8], i: usize, n: usize) -> usize {
    if i >= p.len() {
        return 0;
    }
    let end = i.saturating_add(n).min(p.len());
    p[i..end].iter().fold(0usize, |a, &b| (a << 8) | b as usize)
}

/// TLS ClientHello içindeki SNI'nin (başlangıç, uzunluk) değeri; yoksa None.
pub fn find_sni(p: &[u8]) -> Option<(usize, usize)> {
    let at = |i: usize| p.get(i).copied().map(usize::from);
    if at(0)? != 0x16 || at(5)? != 0x01 {
        return None;
    }
    let mut i: usize = 43; // kayıt(5)+el sıkışma(4)+sürüm(2)+random(32)
    i += 1 + at(i)?; // session id
    i += 2 + be(p, i, 2); // şifre takımları
    i += 1 + at(i)?; // sıkıştırma
    let end = i + 2 + be(p, i, 2);
    i += 2;
    while i + 4 <= end {
        let etype = be(p, i, 2);
        let elen = be(p, i + 2, 2);
        if etype == 0 {
            let nlen = be(p, i + 7, 2);
            if i + 9 + nlen <= p.len() {
                return Some((i + 9, nlen));
            }
            return None;
        }
        i += 4 + elen;
    }
    None
}

fn find(h: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > h.len() {
        return None;
    }
    h[from..].windows(needle.len()).position(|w| w == needle).map(|x| x + from)
}

/// HTTP isteğindeki Host değerinin (başlangıç, uzunluk) değeri; yoksa None.
pub fn find_http_host(p: &[u8]) -> Option<(usize, usize)> {
    const METHODS: [&[u8]; 5] = [b"GET ", b"POST ", b"HEAD ", b"PUT ", b"OPTIONS "];
    if !METHODS.iter().any(|m| p.starts_with(m)) {
        return None;
    }
    let pos = find(p, b"\r\nHost:", 0)?;
    let mut s = pos + 7;
    while s < p.len() && p[s] == 0x20 {
        s += 1;
    }
    let e = find(p, b"\r\n", s).unwrap_or(p.len());
    Some((s, e - s))
}

/// Python `bytes.decode("ascii", "ignore")`.
pub fn host_text(b: &[u8]) -> String {
    b.iter().filter(|&&c| c < 0x80).map(|&c| c as char).collect()
}

pub fn domain_in<S: AsRef<str>>(host: &str, domains: &[S]) -> bool {
    let host = host.to_ascii_lowercase();
    let host = host.trim_end_matches('.');
    domains.iter().any(|d| {
        let d = d.as_ref();
        host == d || (host.len() > d.len() && host.ends_with(d) && host.as_bytes()[host.len() - d.len() - 1] == b'.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http() {
        let p = b"GET / HTTP/1.1\r\nHost:   discord.com\r\nX: y\r\n\r\n";
        let (s, l) = find_http_host(p).unwrap();
        assert_eq!(&p[s..s + l], b"discord.com");
        assert_eq!(find_http_host(b"GET /\r\nHost:"), Some((12, 0)));
        assert_eq!(find_http_host(b"BREW / HTTP"), None);
    }

    #[test]
    fn domains() {
        let d = ["gov.tr", "x.com"];
        assert!(domain_in("www.Turkiye.GOV.TR.", &d));
        assert!(domain_in("x.com", &d));
        assert!(!domain_in("xx.com", &d));
        assert!(!domain_in("gov.tr.evil.com", &d));
    }

    #[test]
    fn sni_garbage() {
        assert_eq!(find_sni(&[]), None);
        assert_eq!(find_sni(&[0x16, 0, 0, 0, 0, 1]), None);
    }
}
