//! nfnetlink_queue iletileri (platformdan bağımsız, saf kod; birim testli).
use gdpi_core::{Engine, Verdict, NFQ_NUM};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NFNL_SUBSYS_QUEUE: u16 = 3;
pub const NFQNL_MSG_PACKET: u16 = 0;
pub const NFQNL_MSG_VERDICT: u16 = 1;
pub const NFQNL_MSG_CONFIG: u16 = 2;
pub const NLMSG_ERROR: u16 = 2;
pub const NLM_F_REQUEST: u16 = 1;
pub const NLM_F_ACK: u16 = 4;
pub const NF_DROP: u32 = 0;
pub const NF_ACCEPT: u32 = 1;

const ATTR_PACKET_HDR: u16 = 1;
const ATTR_PAYLOAD: u16 = 10;
const ATTR_CFG_CMD: u16 = 1;
const ATTR_CFG_PARAMS: u16 = 2;
const ATTR_VERDICT_HDR: u16 = 2;

/// netlink özniteliği (4 bayta hizalı).
pub fn attr(t: u16, data: &[u8]) -> Vec<u8> {
    let mut a = Vec::with_capacity(data.len() + 8);
    a.extend_from_slice(&((4 + data.len()) as u16).to_ne_bytes());
    a.extend_from_slice(&t.to_ne_bytes());
    a.extend_from_slice(data);
    while a.len() % 4 != 0 {
        a.push(0);
    }
    a
}

/// nlmsghdr + nfgenmsg + gövde.
pub fn msg(seq: u32, mtype: u16, body: &[u8], flags: u16) -> Vec<u8> {
    let mut m = Vec::with_capacity(20 + body.len());
    m.extend_from_slice(&((20 + body.len()) as u32).to_ne_bytes());
    m.extend_from_slice(&((NFNL_SUBSYS_QUEUE << 8) | mtype).to_ne_bytes());
    m.extend_from_slice(&flags.to_ne_bytes());
    m.extend_from_slice(&seq.to_ne_bytes());
    m.extend_from_slice(&0u32.to_ne_bytes());
    m.extend_from_slice(&[0, 0]);
    m.extend_from_slice(&NFQ_NUM.to_be_bytes());
    m.extend_from_slice(body);
    m
}

fn cmd(c: u8) -> Vec<u8> {
    attr(ATTR_CFG_CMD, &[c, 0, 0, 0])
}

pub fn cmd_bind(seq: u32) -> Vec<u8> {
    msg(seq, NFQNL_MSG_CONFIG, &cmd(1), NLM_F_REQUEST | NLM_F_ACK)
}

pub fn cmd_unbind(seq: u32) -> Vec<u8> {
    msg(seq, NFQNL_MSG_CONFIG, &cmd(2), NLM_F_REQUEST)
}

/// COPY_PACKET, en fazla 0xFFFF bayt.
pub fn copy_packet(seq: u32) -> Vec<u8> {
    let mut p = 0xFFFFu32.to_be_bytes().to_vec();
    p.push(2);
    msg(seq, NFQNL_MSG_CONFIG, &attr(ATTR_CFG_PARAMS, &p), NLM_F_REQUEST | NLM_F_ACK)
}

pub fn verdict(seq: u32, id: u32, v: u32) -> Vec<u8> {
    let mut b = v.to_be_bytes().to_vec();
    b.extend_from_slice(&id.to_be_bytes());
    msg(seq, NFQNL_MSG_VERDICT, &attr(ATTR_VERDICT_HDR, &b), NLM_F_REQUEST)
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_ne_bytes(d.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// Alınan tampondaki netlink iletileri: (tür, ileti baytları).
fn messages(data: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 16 <= data.len() {
        let (Some(ln), Some(t)) = (u32_at(data, pos), u16_at(data, pos + 4)) else { break };
        let ln = ln as usize;
        if ln < 16 {
            break;
        }
        let end = (pos + ln).min(data.len());
        out.push((t, &data[pos..end]));
        pos = pos.saturating_add((ln + 3) & !3);
    }
    out
}

/// NLMSG_ERROR yanıtı varsa errno'su (0: ACK).
pub fn ack_error(data: &[u8]) -> Option<i32> {
    messages(data)
        .into_iter()
        .find(|(t, _)| *t == NLMSG_ERROR)
        .map(|(_, m)| u32_at(m, 16).map(|e| -(e as i32)).unwrap_or(0))
}

/// Kuyruktan gelen paket.
#[derive(Debug, PartialEq, Eq)]
pub struct QPacket {
    pub id: u32,
    pub payload: Vec<u8>,
}

/// Tampondaki NFQNL_MSG_PACKET iletilerini çözer.
pub fn packets(data: &[u8]) -> Vec<QPacket> {
    let mut out = Vec::new();
    for (t, m) in messages(data) {
        if t != NFNL_SUBSYS_QUEUE << 8 | NFQNL_MSG_PACKET {
            continue;
        }
        let (mut id, mut payload) = (None, Vec::new());
        let mut a = 20usize;
        while a + 4 <= m.len() {
            let (Some(alen), Some(at)) = (u16_at(m, a), u16_at(m, a + 2)) else { break };
            let alen = alen as usize;
            if alen < 4 {
                break;
            }
            let body = m.get(a + 4..(a + alen).min(m.len())).unwrap_or(&[]);
            match at & 0x3FFF {
                ATTR_PACKET_HDR if body.len() >= 4 => {
                    id = Some(u32::from_be_bytes([body[0], body[1], body[2], body[3]]))
                }
                ATTR_PAYLOAD => payload = body.to_vec(),
                _ => {}
            }
            a += (alen + 3) & !3;
        }
        if let Some(id) = id {
            out.push(QPacket { id, payload });
        }
    }
    out
}

/// Kararları ve enjekte edilen paketleri gönderen taraf (gerçek soketler ya da test sahtesi).
pub trait Sink {
    fn verdict(&mut self, id: u32, v: u32);
    fn inject(&mut self, pkt: &[u8]);
}

/// Python `Engine.loop` + `NFQueue.send`: paketi motora verir, kararı uygular.
/// Motor paniklerse paketin aslı geçirilir ve "hata: ..." yazılır.
pub fn handle(engine: &mut Engine, sink: &mut dyn Sink, q: &QPacket, log: &dyn Fn(&str)) {
    if q.payload.is_empty() {
        sink.verdict(q.id, NF_ACCEPT);
        return;
    }
    let v = match catch_unwind(AssertUnwindSafe(|| engine.process(&q.payload, true))) {
        Ok(v) => v,
        Err(e) => {
            let m = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "bilinmeyen".into());
            log(&format!("hata: {m}"));
            Verdict::Pass
        }
    };
    match v {
        Verdict::Pass => sink.verdict(q.id, NF_ACCEPT),
        Verdict::Drop => sink.verdict(q.id, NF_DROP),
        Verdict::Send(outs) => {
            sink.verdict(q.id, NF_DROP);
            for o in outs {
                sink.inject(&o.data);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdpi_core::{Config, Fake, Mode, Stats};
    use std::sync::Arc;

    fn csum(d: &[u8]) -> u16 {
        let mut s: u32 = d.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u32).sum();
        while s >> 16 != 0 {
            s = (s & 0xFFFF) + (s >> 16);
        }
        !(s as u16)
    }

    fn tcp4(payload: &[u8]) -> Vec<u8> {
        let mut tcp = vec![];
        tcp.extend_from_slice(&40000u16.to_be_bytes());
        tcp.extend_from_slice(&443u16.to_be_bytes());
        tcp.extend_from_slice(&1000u32.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.extend_from_slice(&[5 << 4, 0x18, 0xFF, 0xFF, 0, 0, 0, 0]);
        tcp.extend_from_slice(payload);
        let mut ip = vec![0x45, 0];
        ip.extend_from_slice(&((20 + tcp.len()) as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 1, 0, 0, 64, 6, 0, 0, 10, 0, 0, 1, 93, 184, 216, 34]);
        ip.extend(tcp);
        ip
    }

    /// Minimal ClientHello taşıyan TLS kaydı (SNI: example.com).
    fn client_hello(host: &str) -> Vec<u8> {
        let h = host.as_bytes();
        let mut sni = vec![0, 0];
        let list_len = (h.len() + 3) as u16;
        let ext_len = list_len + 2;
        sni.extend_from_slice(&ext_len.to_be_bytes());
        sni.extend_from_slice(&list_len.to_be_bytes());
        sni.push(0);
        sni.extend_from_slice(&(h.len() as u16).to_be_bytes());
        sni.extend_from_slice(h);
        let mut body = vec![3, 3];
        body.extend_from_slice(&[0u8; 32]);
        body.push(0); // session id
        body.extend_from_slice(&[0, 2, 0x13, 0x01]); // cipher suites
        body.extend_from_slice(&[1, 0]); // compression
        body.extend_from_slice(&(sni.len() as u16).to_be_bytes());
        body.extend(sni);
        let mut hs = vec![1];
        hs.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        hs.extend(body);
        let mut rec = vec![0x16, 3, 1];
        rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
        rec.extend(hs);
        rec
    }

    #[derive(Default)]
    struct Fake_ {
        verdicts: Vec<Vec<u8>>,
        raw: Vec<Vec<u8>>,
    }
    impl Sink for Fake_ {
        fn verdict(&mut self, id: u32, v: u32) {
            self.verdicts.push(verdict(1, id, v));
        }
        fn inject(&mut self, pkt: &[u8]) {
            self.raw.push(pkt.to_vec());
        }
    }

    fn vd(m: &[u8]) -> (u32, u32) {
        (u32::from_be_bytes(m[24..28].try_into().unwrap()), u32::from_be_bytes(m[28..32].try_into().unwrap()))
    }

    fn engine(fake: Fake) -> Engine {
        let cfg = Config { mode: Mode::Hepsi, fake, ..Config::default() };
        Engine::new(cfg, Arc::new(Stats::default()), Arc::new(|_: &str| {}))
    }

    #[test]
    fn verdict_layout() {
        let m = verdict(5, 7, NF_ACCEPT);
        assert_eq!(m.len(), 32);
        assert_eq!(u32::from_ne_bytes(m[0..4].try_into().unwrap()), 32);
        assert_eq!(u16::from_ne_bytes(m[4..6].try_into().unwrap()), 3 << 8 | 1);
        assert_eq!(u16::from_ne_bytes(m[6..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_ne_bytes(m[8..12].try_into().unwrap()), 5);
        assert_eq!(&m[16..20], &[0, 0, 0x12, 0x88]); // NFQ_NUM 4744
        assert_eq!(u16::from_ne_bytes(m[20..22].try_into().unwrap()), 12);
        assert_eq!(u16::from_ne_bytes(m[22..24].try_into().unwrap()), 2);
        assert_eq!(vd(&m), (1, 7));
        let c = copy_packet(1);
        assert_eq!(c.len(), 20 + 12);
        assert_eq!(&c[24..29], &[0, 0, 0xFF, 0xFF, 2]);
        assert_eq!(u16::from_ne_bytes(cmd_bind(1)[6..8].try_into().unwrap()), 5);
    }

    #[test]
    fn parse_packet_and_ack() {
        let mut body = attr(1, &[0, 0, 0, 9, 0x08, 0x00, 3]);
        body.extend(attr(10 | 0x8000, b"abcde"));
        let mut buf = msg(0, NFQNL_MSG_PACKET, &body, 0);
        buf.extend(verdict(2, 1, 1)); // ilgisiz ileti atlanır
        assert_eq!(packets(&buf), vec![QPacket { id: 9, payload: b"abcde".to_vec() }]);
        // bozuk/kısa tamponlar panik yapmaz
        for n in 0..buf.len() {
            let _ = packets(&buf[..n]);
            let _ = ack_error(&buf[..n]);
        }
        let mut err = 16u32.wrapping_add(20).to_ne_bytes().to_vec();
        err.extend_from_slice(&NLMSG_ERROR.to_ne_bytes());
        err.extend_from_slice(&[0; 10]);
        err.extend_from_slice(&(-1i32).to_ne_bytes());
        err.extend_from_slice(&[0; 16]);
        assert_eq!(ack_error(&err), Some(1));
        assert_eq!(ack_error(&buf), None);
    }

    #[test]
    fn untouched_packet_accepted() {
        let mut s = Fake_::default();
        handle(&mut engine(Fake::Checksum), &mut s, &QPacket { id: 7, payload: tcp4(b"hello") }, &|_| {});
        assert_eq!(s.verdicts.len(), 1);
        assert_eq!(vd(&s.verdicts[0]), (1, 7));
        assert!(s.raw.is_empty());
    }

    #[test]
    fn split_drops_original_and_injects() {
        let mut s = Fake_::default();
        let data = client_hello("example.com");
        let mut e = engine(Fake::Checksum);
        handle(&mut e, &mut s, &QPacket { id: 9, payload: tcp4(&data) }, &|_| {});
        assert_eq!(vd(&s.verdicts[0]), (0, 9)); // aslı düşürüldü
        assert_eq!(s.raw.len(), 4); // sahte + 3 parça
        assert_eq!(&s.raw[0][16..20], &[93, 184, 216, 34]);
        let pseudo = |b: &[u8]| {
            let mut p = b[12..20].to_vec();
            p.extend_from_slice(&[0, 6]);
            p.extend_from_slice(&((b.len() - 20) as u16).to_be_bytes());
            p.extend_from_slice(&b[20..]);
            p
        };
        assert_ne!(csum(&pseudo(&s.raw[0])), 0); // sahte bozuk
        for p in &s.raw[1..] {
            assert_eq!(csum(&pseudo(p)), 0);
            assert_eq!(csum(&p[..20]), 0);
        }
        let joined: Vec<u8> = s.raw[1..].iter().flat_map(|x| x[40..].to_vec()).collect();
        assert_eq!(joined, data);
        assert_eq!(e.stats.split(), 1);
    }

    #[test]
    fn empty_payload_accepted() {
        let mut s = Fake_::default();
        handle(&mut engine(Fake::Kapali), &mut s, &QPacket { id: 3, payload: vec![] }, &|_| {});
        assert_eq!(vd(&s.verdicts[0]), (1, 3));
    }
}
