//! Ayrıştırıcılara rastgele/bozuk veri: asla panik yok, dönen aralıklar hep sınırlar içinde.
#![allow(clippy::field_reassign_with_default)]
use gdpi_core::*;
use proptest::prelude::*;
use std::sync::Arc;

fn tls_like() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..600).prop_map(|mut v| {
        if v.len() > 5 {
            v[0] = 0x16;
            v[5] = 0x01;
        }
        v
    })
}

fn packet() -> impl Strategy<Value = Vec<u8>> {
    (prop::collection::vec(any::<u8>(), 0..120), tls_like(), any::<bool>(), any::<u8>()).prop_map(|(mut h, p, v6, ihl)| {
        if !h.is_empty() {
            h[0] = if v6 { 0x60 } else { 0x40 | (ihl & 15) };
        }
        h.extend(p);
        h
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn sni_no_panic(p in tls_like()) {
        if let Some((s, l)) = find_sni(&p) { prop_assert!(s + l <= p.len()); }
    }

    #[test]
    fn http_no_panic(p in prop::collection::vec(any::<u8>(), 0..300), m in 0usize..5) {
        let mut q = [&b"GET "[..], b"POST ", b"HEAD ", b"PUT ", b"OPTIONS "][m].to_vec();
        q.extend(p);
        if let Some((s, l)) = find_http_host(&q) { prop_assert!(s + l <= q.len()); }
    }

    #[test]
    fn engine_no_panic(raw in packet(), out in any::<bool>(), fake in 0u8..3, rev in any::<bool>()) {
        let mut cfg = Config::default();
        cfg.fake = [Fake::Checksum, Fake::Ttl, Fake::Kapali][fake as usize];
        cfg.reverse = rev;
        cfg.bypass.clear();
        let mut e = Engine::new(cfg, Arc::new(Stats::default()), Arc::new(|_: &str| {}));
        let _ = e.process(&raw, out);
        checksum::fix_checksums(&mut raw.clone());
    }

    #[test]
    fn sanitize_no_panic(s in ".{0,200}") {
        let _ = Config::from_json(&s);
    }
}
