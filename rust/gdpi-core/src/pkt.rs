//! IPv4/IPv6 + TCP/UDP başlıklarını doğrudan baytlar üzerinde okur/yazar (Python `Pkt`).

#[derive(Debug, Clone)]
pub struct Pkt {
    pub raw: Vec<u8>,
    pub v6: bool,
    pub l4: usize,
    pub proto: u8,
    pub sport: u16,
    pub dport: u16,
    pub poff: usize,
}

impl Pkt {
    /// Başlıklar eksik ya da tutarsızsa None (paket olduğu gibi geçirilir).
    pub fn parse(raw: Vec<u8>) -> Option<Pkt> {
        let first = *raw.first()?;
        let v6 = first >> 4 == 6;
        let (l4, proto) = if v6 {
            (40, *raw.get(6)?)
        } else if first >> 4 == 4 {
            let l4 = usize::from(first & 15) * 4;
            if l4 < 20 {
                return None;
            }
            (l4, *raw.get(9)?)
        } else {
            return None;
        };
        let hdr = match proto {
            6 => usize::from(*raw.get(l4 + 12)? >> 4) * 4,
            17 => 8,
            _ => return None,
        };
        if hdr < 8 || raw.len() < l4 + hdr {
            return None;
        }
        let u16at = |i: usize| u16::from_be_bytes([raw[i], raw[i + 1]]);
        Some(Pkt { sport: u16at(l4), dport: u16at(l4 + 2), poff: l4 + hdr, v6, l4, proto, raw })
    }

    pub fn tcp(&self) -> bool {
        self.proto == 6
    }

    pub fn udp(&self) -> bool {
        self.proto == 17
    }

    pub fn payload(&self) -> &[u8] {
        &self.raw[self.poff..]
    }

    pub fn seq(&self) -> u32 {
        let i = self.l4 + 4;
        u32::from_be_bytes([self.raw[i], self.raw[i + 1], self.raw[i + 2], self.raw[i + 3]])
    }

    /// Aynı başlıklarla, yeni yük ve sıra numarasıyla bir TCP paketi üretir (sağlama toplamı hesaplanmaz).
    pub fn segment(&self, data: &[u8], seq: u32, ttl: Option<u8>) -> Vec<u8> {
        let mut r = Vec::with_capacity(self.poff + data.len());
        r.extend_from_slice(&self.raw[..self.poff]);
        r.extend_from_slice(data);
        let n = r.len();
        if self.v6 {
            r[4..6].copy_from_slice(&((n - 40) as u16).to_be_bytes());
        } else {
            r[2..4].copy_from_slice(&(n as u16).to_be_bytes());
        }
        r[self.l4 + 4..self.l4 + 8].copy_from_slice(&seq.to_be_bytes());
        if let Some(t) = ttl {
            r[if self.v6 { 7 } else { 8 }] = t;
        }
        r
    }
}
