//! IPv4/IPv6 + TCP/UDP sağlama toplamları (Python `fix_checksums`, WinDivertHelperCalcChecksums karşılığı).

pub fn inet_checksum(parts: &[&[u8]]) -> u16 {
    let mut s: u64 = 0;
    let mut odd: Option<u8> = None;
    for part in parts {
        for &b in part.iter() {
            match odd.take() {
                Some(h) => s += u64::from(u16::from_be_bytes([h, b])),
                None => odd = Some(b),
            }
        }
    }
    if let Some(h) = odd {
        s += u64::from(u16::from_be_bytes([h, 0]));
    }
    while s >> 16 != 0 {
        s = (s & 0xFFFF) + (s >> 16);
    }
    !(s as u16)
}

/// Paketin IP ve TCP/UDP sağlama toplamlarını yeniden hesaplar. Kısa/bozuk pakette hiçbir şey yapmaz.
pub fn fix_checksums(r: &mut [u8]) {
    if r.is_empty() {
        return;
    }
    let v6 = r[0] >> 4 == 6;
    let (l4, proto) = if v6 {
        if r.len() < 40 {
            return;
        }
        (40, r[6])
    } else {
        let l4 = usize::from(r[0] & 15) * 4;
        if r.len() < 20 || l4 < 20 || r.len() < l4 {
            return;
        }
        r[10] = 0;
        r[11] = 0;
        let c = inet_checksum(&[&r[..l4]]);
        r[10..12].copy_from_slice(&c.to_be_bytes());
        (l4, r[9])
    };
    if proto != 6 && proto != 17 {
        return;
    }
    let off = l4 + if proto == 6 { 16 } else { 6 };
    if r.len() < off + 2 {
        return;
    }
    let seg_len = r.len() - l4;
    r[off] = 0;
    r[off + 1] = 0;
    let c = if v6 {
        let mut ph = [0u8; 8];
        ph[..4].copy_from_slice(&(seg_len as u32).to_be_bytes());
        ph[7] = proto;
        inet_checksum(&[&r[8..40], &ph, &r[l4..]])
    } else {
        let mut ph = [0u8; 4];
        ph[1] = proto;
        ph[2..].copy_from_slice(&(seg_len as u16).to_be_bytes());
        inet_checksum(&[&r[12..20], &ph, &r[l4..]])
    };
    let c = if proto == 17 && c == 0 { 0xFFFF } else { c };
    r[off..off + 2].copy_from_slice(&c.to_be_bytes());
}
