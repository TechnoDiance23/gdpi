//! Python `tests/test_linux_engine.py` EndToEnd karşılığı. GDPI_E2E=1 ve root ister.
#![forbid(unsafe_code)]
#![cfg(target_os = "linux")]
use gdpi_core::{Config, Fake, Mode, Stats};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

const HOSTS: [&str; 5] = ["www.google.com", "github.com", "example.com", "www.cloudflare.com", "pypi.org"];

fn check(h: &str) -> bool {
    Command::new("curl")
        .args(["-sS", "-o", "/dev/null", "--max-time", "10", &format!("https://{h}/")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn tables() -> String {
    String::from_utf8_lossy(&Command::new("nft").args(["list", "tables"]).output().unwrap().stdout).into()
}

#[test]
fn real_traffic() {
    if std::env::var("GDPI_E2E").is_err() || !gdpi_linux::is_admin() {
        eprintln!("atlandı: GDPI_E2E=1 ve root gerekir");
        return;
    }
    let base: BTreeMap<_, _> = HOSTS.iter().map(|h| (*h, check(h))).collect();
    println!("kapalı: {base:?}");
    let mut fails = BTreeMap::new();
    for fake in [Fake::Kapali, Fake::Checksum, Fake::Ttl] {
        let logs = Arc::new(Mutex::new(Vec::<String>::new()));
        let l2 = logs.clone();
        let stats = Arc::new(Stats::default());
        let cfg = Config { mode: Mode::Hepsi, fake, ..Config::default() };
        let mut be = match gdpi_linux::start(cfg, stats.clone(), Arc::new(move |m: &str| l2.lock().unwrap().push(m.into()))) {
            Ok(b) => b,
            Err(e) if e.msg.contains("NFQUEUE desteği yok") => {
                // Python sürümüyle aynı hata; yarım kalmış tablo olmamalı.
                assert!(!tables().contains("gdpi"));
                assert!(std::env::var_os("GDPI_E2E_STRICT").is_none(), "CI: NFQUEUE olmalı: {e}");
                eprintln!("atlandı: {e}");
                return;
            }
            Err(e) => panic!("start: {e}"),
        };
        assert!(tables().contains("gdpi"));
        let on: BTreeMap<_, _> = HOSTS.iter().map(|h| (*h, check(h))).collect();
        println!("açık ({}): {on:?} split={} logs={:?}", fake.as_str(), stats.split(), logs.lock().unwrap());
        fails.insert(fake.as_str(), HOSTS.iter().filter(|h| base[*h] && !on[*h]).copied().collect::<Vec<_>>());
        if fake == Fake::Kapali {
            assert!(stats.split() > 0, "{:?}", logs.lock().unwrap());
        }
        be.stop();
        be.stop(); // idempotent
        assert!(!tables().contains("gdpi"));
    }
    // checksum modu yalnızca raporlanır (bazı ağlar sağlama toplamını yolda düzeltir).
    println!("başarısız: {fails:?}");
    assert_eq!(fails["kapali"], Vec::<&str>::new());
    assert_eq!(fails["ttl"], Vec::<&str>::new());
    assert!(!tables().contains("gdpi"));
}

/// Bu çekirdekte nft_queue yoksa da kuyruk yolunu sınar: iptables NFQUEUE ile loopback'teki
/// 127.0.0.1:443 bağlantısını kuyruğa sokar, ClientHello'nun bölünüp eksiksiz ulaştığını doğrular.
#[test]
fn loopback_queue_iptables() {
    use std::io::{Read, Write};
    if std::env::var("GDPI_E2E").is_err() || !gdpi_linux::is_admin() {
        eprintln!("atlandı: GDPI_E2E=1 ve root gerekir");
        return;
    }
    let rule = ["OUTPUT", "-o", "lo", "-p", "tcp", "--dport", "443", "-m", "mark", "!", "--mark", "0x47445049",
        "-j", "NFQUEUE", "--queue-num", "4744", "--queue-bypass"];
    let ipt = |op: &str| Command::new("iptables").arg(op).args(rule).status().map(|s| s.success()).unwrap_or(false);
    if !ipt("-I") {
        eprintln!("atlandı: iptables NFQUEUE yok");
        return;
    }
    let stats = Arc::new(Stats::default());
    let cfg = Config { mode: Mode::Hepsi, fake: Fake::Kapali, ..Config::default() };
    let be = gdpi_linux::start_queue_only(cfg, stats.clone(), Arc::new(|m: &str| println!("log: {m}")));
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut be = be.expect("queue");
        let l = std::net::TcpListener::bind("127.0.0.1:443").unwrap();
        let hello = client_hello("example.com");
        let h2 = hello.clone();
        let t = std::thread::spawn(move || {
            let mut c = std::net::TcpStream::connect("127.0.0.1:443").unwrap();
            c.write_all(&h2).unwrap();
        });
        let (mut s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut got = vec![0u8; hello.len()];
        s.read_exact(&mut got).unwrap();
        t.join().unwrap();
        be.stop();
        assert_eq!(got, hello);
        assert_eq!(stats.split(), 1);
    }));
    ipt("-D");
    res.unwrap();
}

fn client_hello(host: &str) -> Vec<u8> {
    let h = host.as_bytes();
    let list_len = (h.len() + 3) as u16;
    let mut sni = vec![0, 0];
    sni.extend_from_slice(&(list_len + 2).to_be_bytes());
    sni.extend_from_slice(&list_len.to_be_bytes());
    sni.push(0);
    sni.extend_from_slice(&(h.len() as u16).to_be_bytes());
    sni.extend_from_slice(h);
    let mut body = vec![3, 3];
    body.extend_from_slice(&[0u8; 32]);
    body.extend_from_slice(&[0, 0, 2, 0x13, 0x01, 1, 0]);
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
