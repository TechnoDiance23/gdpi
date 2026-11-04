//! Gerçek NFQUEUE + nftables arka ucu.
use crate::nl::{self, QPacket, Sink};
use gdpi_core::{nft_rules, Backend, Config, Engine, EngineError, LogFn, Stats, NFQ_MARK, NFT_TABLE};
use nix::errno::Errno;
use nix::sys::socket::{
    bind, recv, send, sendto, setsockopt, socket, sockopt, AddressFamily, MsgFlags, NetlinkAddr, SockFlag,
    SockProtocol, SockType, SockaddrIn, SockaddrIn6,
};
use nix::sys::time::TimeVal;
use std::io::Write;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

fn err(e: impl std::fmt::Display) -> EngineError {
    EngineError::new(1, e.to_string())
}

/// `nft -f -` (Python `nft`).
fn nft(script: &str) -> Result<(), EngineError> {
    let mut c = match Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(EngineError::new(2, "nftables kurulu değil (ör. sudo apt install nftables)."))
        }
        Err(e) => return Err(err(e)),
    };
    if let Some(mut si) = c.stdin.take() {
        let _ = si.write_all(script.as_bytes());
    }
    let o = c.wait_with_output().map_err(err)?;
    if !o.status.success() {
        let s = String::from_utf8_lossy(&o.stderr).trim().to_string();
        let s = if s.is_empty() { "nft hatası".to_string() } else { s.chars().take(300).collect() };
        return Err(EngineError::new(o.status.code().unwrap_or(1) as i64, s));
    }
    Ok(())
}

pub fn nft_remove() {
    let _ = Command::new("nft")
        .args(["delete", "table", "inet", NFT_TABLE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

struct Socks {
    nl: OwnedFd,
    raw4: OwnedFd,
    /// IPv6 kapalı çekirdeklerde yok (o zaman IPv6 paketi de kuyruğa gelmez).
    raw6: Option<OwnedFd>,
    seq: u32,
}

impl Socks {
    fn next(&mut self) -> u32 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    fn open() -> Result<Socks, EngineError> {
        let nl = socket(AddressFamily::Netlink, SockType::Raw, SockFlag::SOCK_CLOEXEC, SockProtocol::NetlinkNetFilter)
            .map_err(err)?;
        bind(nl.as_raw_fd(), &NetlinkAddr::new(0, 0)).map_err(err)?;
        setsockopt(&nl, sockopt::RcvBuf, &(4 << 20)).map_err(err)?;
        setsockopt(&nl, sockopt::ReceiveTimeout, &TimeVal::new(0, 500_000)).map_err(err)?;
        let raw = |f| -> Result<OwnedFd, EngineError> {
            let s = socket(f, SockType::Raw, SockFlag::SOCK_CLOEXEC, SockProtocol::Raw).map_err(err)?;
            setsockopt(&s, sockopt::Mark, &NFQ_MARK).map_err(err)?;
            Ok(s)
        };
        let mut s = Socks { raw4: raw(AddressFamily::Inet)?, raw6: raw(AddressFamily::Inet6).ok(), nl, seq: 0 };
        let m = nl::cmd_bind(s.next());
        s.config(&m)?;
        let m = nl::copy_packet(s.next());
        s.config(&m)?;
        Ok(s)
    }

    /// İsteği gönderir, ACK/hata yanıtını bekler.
    fn config(&mut self, m: &[u8]) -> Result<(), EngineError> {
        send(self.nl.as_raw_fd(), m, MsgFlags::empty()).map_err(err)?;
        let mut buf = vec![0u8; 65536];
        for _ in 0..20 {
            let n = match recv(self.nl.as_raw_fd(), &mut buf, MsgFlags::empty()) {
                Ok(n) => n,
                Err(Errno::EAGAIN | Errno::EINTR) => continue,
                Err(e) => return Err(err(e)),
            };
            if let Some(e) = nl::ack_error(&buf[..n]) {
                if e != 0 {
                    return Err(EngineError::new(
                        e as i64,
                        format!(
                            "NFQUEUE açılamadı ({}). Çekirdekte nfnetlink_queue modülü olmalı.",
                            Errno::from_raw(e).desc()
                        ),
                    ));
                }
                return Ok(());
            }
        }
        Err(EngineError::new(110, "NFQUEUE açılamadı (yanıt yok). Çekirdekte nfnetlink_queue modülü olmalı."))
    }
}

impl Sink for Socks {
    fn verdict(&mut self, id: u32, v: u32) {
        let m = nl::verdict(self.next(), id, v);
        let _ = send(self.nl.as_raw_fd(), &m, MsgFlags::empty());
    }

    fn inject(&mut self, p: &[u8]) {
        let _ = if p.first().map(|b| b >> 4) == Some(6) {
            let Some(fd) = &self.raw6 else { return };
            let Some(d) = p.get(24..40).and_then(|d| <[u8; 16]>::try_from(d).ok()) else { return };
            let a = SockaddrIn6::from(SocketAddrV6::new(Ipv6Addr::from(d), 0, 0, 0));
            sendto(fd.as_raw_fd(), p, &a, MsgFlags::empty())
        } else {
            let Some(d) = p.get(16..20).and_then(|d| <[u8; 4]>::try_from(d).ok()) else { return };
            let a = SockaddrIn::from(SocketAddrV4::new(Ipv4Addr::from(d), 0));
            sendto(self.raw4.as_raw_fd(), p, &a, MsgFlags::empty())
        }; // ENOBUFS vb. yok sayılır
    }
}

fn run(mut s: Socks, mut engine: Engine, stop: Arc<AtomicBool>, log: LogFn) {
    let mut buf = vec![0u8; 70000];
    while !stop.load(Ordering::Relaxed) {
        let n = match recv(s.nl.as_raw_fd(), &mut buf, MsgFlags::empty()) {
            Ok(n) => n,
            Err(_) => continue, // zaman aşımı, ENOBUFS vb.
        };
        for q in nl::packets(&buf[..n]) {
            let q: QPacket = q;
            nl::handle(&mut engine, &mut s, &q, &|m| log(m));
        }
    }
    // kuyruğu hemen bırak: yeniden başlatma beklemeden çalışsın
    let m = nl::cmd_unbind(s.next());
    let _ = send(s.nl.as_raw_fd(), &m, MsgFlags::empty());
    // soketler `s` düşerken kapanır
}

struct NfqBackend {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Backend for NfqBackend {
    fn stop(&mut self) {
        if let Some(t) = self.thread.take() {
            self.stop.store(true, Ordering::Relaxed);
            nft_remove();
            let _ = t.join();
        }
    }
}

impl Drop for NfqBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    if !crate::is_admin() {
        return Err(EngineError::new(1, "Yönetici (root) izni gerekli: sudo ile açın."));
    }
    nft_remove();
    let _ = Command::new("modprobe")
        .args(["-a", "nfnetlink_queue", "nft_queue"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let rules = nft_rules(&cfg);
    if nft(&rules).is_err() {
        // conntrack ifadesi yoksa: tüm 80/443 paketleri
        if let Err(e) = nft(&crate::nft_rules_fallback(&rules)) {
            if e.msg.contains("queue") {
                return Err(EngineError::new(
                    e.code,
                    "Çekirdekte NFQUEUE desteği yok (nft_queue / nfnetlink_queue modülleri yüklenemedi).",
                ));
            }
            return Err(e);
        }
    }
    spawn(cfg, stats, log).inspect_err(|_| nft_remove())
}

/// Kuralları kurmadan yalnızca kuyruğa bağlanır (test/teşhis için; kuralları çağıran kurar).
pub fn spawn(cfg: Config, stats: Arc<Stats>, log: LogFn) -> Result<Box<dyn Backend>, EngineError> {
    let socks = Socks::open()?;
    let stop = Arc::new(AtomicBool::new(false));
    let engine = Engine::new(cfg, stats, log.clone());
    let st = stop.clone();
    let thread = std::thread::Builder::new()
        .name("gdpi-nfq".into())
        .spawn(move || run(socks, engine, st, log))
        .map_err(err)?;
    Ok(Box::new(NfqBackend { stop, thread: Some(thread) }))
}
