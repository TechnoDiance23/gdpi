//! Motorun platformdan bağımsız kısmı (Python `Engine.handle_tcp/handle_udp/send_split`).
//! WinDivert ve NFQUEUE yalnızca paketi alıp buradaki kararı uygular.
use crate::checksum::fix_checksums;
use crate::config::{Config, Fake, Mode};
use crate::parse::{domain_in, find_http_host, find_sni, host_text};
use crate::pkt::Pkt;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub const NFQ_NUM: u16 = 4744;
/// GDPI'ın kendi gönderdiği paketler: kuyruğa geri girmez.
pub const NFQ_MARK: u32 = 0x4744_5049;
pub const NFT_TABLE: &str = "gdpi";

pub type LogFn = Arc<dyn Fn(&str) + Send + Sync>;
type JunkFn = Box<dyn FnMut(&mut [u8]) + Send>;

/// Arayüzde gösterilen sayaçlar.
#[derive(Debug, Default)]
pub struct Stats {
    pub n_split: AtomicU64,
    pub n_skip: AtomicU64,
}

impl Stats {
    pub fn split(&self) -> u64 {
        self.n_split.load(Ordering::Relaxed)
    }
    pub fn skip(&self) -> u64 {
        self.n_skip.load(Ordering::Relaxed)
    }
}

/// Gönderilecek paket. Sağlama toplamları çekirdek tarafından zaten doğru (sahte pakette bilerek bozuk)
/// hesaplanmıştır. `recalc`: Python'un bu pakette `WinDivertHelperCalcChecksums` çağırıp çağırmadığı;
/// Windows motoru bunu adres bayraklarını ayarlamak için kullanır. `recalc == false` olan pakette
/// içerik kesinlikle yeniden hesaplanmamalı (bozuk checksum'lı sahte paket düzelmesin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Out {
    pub data: Vec<u8>,
    pub recalc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Olduğu gibi geçir (sağlama toplamına dokunma).
    Pass,
    /// Düşür (QUIC).
    Drop,
    /// Aslı yerine bunları sırayla gönder.
    Send(Vec<Out>),
}

pub struct Engine {
    pub cfg: Config,
    pub stats: Arc<Stats>,
    log: LogFn,
    seen: HashSet<String>,
    /// yerel port -> asıl DNS sunucusu
    dns_map: HashMap<u16, [u8; 4]>,
    dns_ip: [u8; 4],
    junk: JunkFn,
}

impl Engine {
    pub fn new(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Engine {
        let mut dns_ip = [0u8; 4];
        for (d, s) in dns_ip.iter_mut().zip(cfg.dns_addr.split('.')) {
            *d = s.parse().unwrap_or(0);
        }
        Engine {
            cfg,
            stats,
            log,
            seen: HashSet::new(),
            dns_map: HashMap::new(),
            dns_ip,
            junk: Box::new(|b: &mut [u8]| b.iter_mut().for_each(|x| *x = fastrand::u8(..))),
        }
    }

    /// Testler için: sahte paketin rastgele içeriğini belirler.
    pub fn set_junk(&mut self, f: impl FnMut(&mut [u8]) + Send + 'static) {
        self.junk = Box::new(f);
    }

    pub fn should_touch(&self, host: &str) -> bool {
        if domain_in(host, &self.cfg.bypass) {
            return false;
        }
        if self.cfg.mode == Mode::Liste {
            return domain_in(host, &self.cfg.targets);
        }
        true
    }

    /// Bir paketi işler. `outbound`: WinDivert adres bayrağı (Linux'ta hep true).
    pub fn process(&mut self, raw: &[u8], outbound: bool) -> Verdict {
        let Some(p) = Pkt::parse(raw.to_vec()) else { return Verdict::Pass };
        if p.udp() {
            return self.handle_udp(p, outbound);
        }
        self.handle_tcp(&p).unwrap_or(Verdict::Pass)
    }

    /// UDP: QUIC engeli + DNS yönlendirme (yalnız IPv4).
    fn handle_udp(&mut self, mut p: Pkt, out: bool) -> Verdict {
        if out && p.dport == 443 {
            return Verdict::Drop; // QUIC düşür -> tarayıcı TCP'ye döner
        }
        let l4 = p.l4;
        let r = &mut p.raw;
        if out && p.dport == 53 && !p.v6 {
            let mut orig = [0u8; 4];
            orig.copy_from_slice(&r[16..20]);
            self.dns_map.insert(p.sport, orig);
            r[16..20].copy_from_slice(&self.dns_ip);
            r[l4 + 2..l4 + 4].copy_from_slice(&self.cfg.dns_port.to_be_bytes());
        } else if !out && !p.v6 && r[12..16] == self.dns_ip {
            if let Some(orig) = self.dns_map.remove(&p.dport) {
                r[12..16].copy_from_slice(&orig);
                r[l4..l4 + 2].copy_from_slice(&53u16.to_be_bytes());
            }
        }
        fix_checksums(r);
        Verdict::Send(vec![Out { data: p.raw, recalc: true }])
    }

    /// TCP: SNI / Host bölme. None: dokunma.
    fn handle_tcp(&mut self, p: &Pkt) -> Option<Verdict> {
        if !p.tcp() {
            return None;
        }
        let payload = p.payload();
        let (start, ln) = if p.dport == 443 { find_sni(payload) } else { find_http_host(payload) }?;
        let host = host_text(&payload[start..start + ln]);
        if !self.should_touch(&host) {
            self.stats.n_skip.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        self.stats.n_split.fetch_add(1, Ordering::Relaxed);
        if self.seen.insert(host.clone()) {
            (self.log)(&format!("→ {host}"));
        }
        let mut data = payload.to_vec();
        let cuts = if p.dport == 80 {
            // find_http_host bulduysa "\r\nHost:" kesin var
            let hpos = data.windows(7).position(|w| w == b"\r\nHost:")?;
            data[hpos + 2..hpos + 7].copy_from_slice(b"hoSt:");
            vec![hpos + 4, start + ln / 2]
        } else {
            vec![2, start + ln / 2] // kayıt başlığı + alan adının ortası
        };
        Some(Verdict::Send(self.split(p, &data, cuts)))
    }

    /// Python `send_split`: veriyi kesim noktalarından böler, önce sahte paketi ekler.
    pub fn split(&mut self, p: &Pkt, data: &[u8], cuts: Vec<usize>) -> Vec<Out> {
        let mut cuts: Vec<usize> = cuts.into_iter().filter(|&c| 0 < c && c < data.len()).collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut b = vec![0];
        b.extend(cuts);
        b.push(data.len());
        let mut parts: Vec<(usize, &[u8])> = b.windows(2).map(|w| (w[0], &data[w[0]..w[1]])).collect();
        let seq = p.seq();
        let mut out = Vec::with_capacity(parts.len() + 1);
        if self.cfg.fake != Fake::Kapali {
            let mut junk = vec![0u8; parts[0].1.len()];
            (self.junk)(&mut junk);
            let ttl = (self.cfg.fake == Fake::Ttl).then_some(self.cfg.ttl);
            let mut f = p.segment(&junk, seq, ttl);
            fix_checksums(&mut f);
            if self.cfg.fake == Fake::Checksum {
                // TCP checksum alanı: bozuk -> sunucu kesin reddeder
                let c = p.l4 + 16;
                f[c] ^= 0xFF;
                f[c + 1] ^= 0xFF;
            }
            out.push(Out { data: f, recalc: false });
        }
        if self.cfg.reverse {
            parts.reverse();
        }
        for (off, chunk) in parts {
            let mut s = p.segment(chunk, seq.wrapping_add(off as u32), None);
            fix_checksums(&mut s);
            out.push(Out { data: s, recalc: true });
        }
        out
    }
}

/// WinDivert filtresi (Python `Engine.filter_str`).
pub fn filter_str(cfg: &Config) -> String {
    let mut f = String::from("(outbound and !loopback and tcp.PayloadLength > 0 and (tcp.DstPort == 443 or tcp.DstPort == 80))");
    if cfg.block_quic {
        f += " or (outbound and !loopback and udp.DstPort == 443)";
    }
    if cfg.dns_redirect {
        f += &format!(
            " or (outbound and ip and udp.DstPort == 53) or (inbound and ip and udp.SrcPort == {} and ip.SrcAddr == {})",
            cfg.dns_port, cfg.dns_addr
        );
    }
    f
}

/// nftables kuralları (Python `nft_rules`). `bypass`: GDPI kapanır ya da çökerse paketler normal akar.
pub fn nft_rules(cfg: &Config) -> String {
    let mut out = vec![
        format!("meta mark {NFQ_MARK:#x} return"),
        "oifname \"lo\" return".to_string(),
        format!("tcp dport {{ 80, 443 }} ct original packets < 12 queue num {NFQ_NUM} bypass"),
    ];
    if cfg.block_quic {
        out.push("udp dport 443 drop".into());
    }
    let mut rules = format!(
        "table inet {NFT_TABLE} {{\n chain out {{ type filter hook output priority 0; policy accept;\n  {}\n }}\n",
        out.join("\n  ")
    );
    if cfg.dns_redirect {
        rules += &format!(
            " chain dns {{ type nat hook output priority -100; policy accept;\n  ip daddr != 127.0.0.0/8 udp dport 53 dnat ip to {}:{}\n }}\n",
            cfg.dns_addr, cfg.dns_port
        );
    }
    rules + "}\n"
}
