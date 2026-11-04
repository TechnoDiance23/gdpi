//! Fark testi: gen_corpus.py'nin Python (gdpi.py) ile ürettiği beklenen sonuçlarla birebir karşılaştırma.
//! Önce: python3 rust/gdpi-core/tests/gen_corpus.py
use gdpi_core::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};

fn corpus() -> Option<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus.json");
    match std::fs::read_to_string(path) {
        Ok(t) => Some(serde_json::from_str(&t).unwrap()),
        Err(_) if std::env::var_os("GDPI_REQUIRE_CORPUS").is_none() => {
            eprintln!("corpus.json yok, atlandı (python3 rust/gdpi-core/tests/gen_corpus.py)");
            None
        }
        Err(e) => panic!("corpus.json okunamadı: {e}"),
    }
}

fn loc(v: &Value) -> Option<(usize, usize)> {
    v.as_array().map(|a| (a[0].as_u64().unwrap() as usize, a[1].as_u64().unwrap() as usize))
}

#[test]
fn parse_matches_python() {
    let Some(c) = corpus() else { return };
    let cases = c["parse"].as_array().unwrap();
    assert!(cases.len() > 1000);
    let mut found = (0, 0);
    for case in cases {
        let p = hex::decode(case["p"].as_str().unwrap()).unwrap();
        let sni = loc(&case["sni"]);
        let http = loc(&case["http"]);
        found.0 += sni.is_some() as usize;
        found.1 += http.is_some() as usize;
        assert_eq!(find_sni(&p), sni, "find_sni {}", case["p"]);
        assert_eq!(find_http_host(&p), http, "find_http_host {}", case["p"]);
    }
    assert!(found.0 > 50 && found.1 > 20, "derlem çok az bulgu içeriyor: {found:?}");
}

#[test]
fn domains_match_python() {
    let Some(c) = corpus() else { return };
    for case in c["domains"].as_array().unwrap() {
        let d: Vec<String> = serde_json::from_value(case["d"].clone()).unwrap();
        let h = case["h"].as_str().unwrap();
        // Python str.lower() Unicode'u da küçültür; Rust tarafında host her zaman ASCII'dir (host_text)
        if !h.is_ascii() {
            continue;
        }
        assert_eq!(domain_in(h, &d), case["r"].as_bool().unwrap(), "{h} {d:?}");
    }
}

#[test]
fn sanitize_matches_python() {
    let Some(c) = corpus() else { return };
    for case in c["configs"].as_array().unwrap() {
        let got = serde_json::to_value(Config::sanitize(&case["in"])).unwrap();
        assert_eq!(got, case["out"], "girdi {}", case["in"]);
    }
}

#[test]
fn rules_match_python() {
    let Some(c) = corpus() else { return };
    for case in c["rules"].as_array().unwrap() {
        let cfg = Config::sanitize(&case["cfg"]);
        assert_eq!(filter_str(&cfg), case["filter"].as_str().unwrap());
        assert_eq!(nft_rules(&cfg), case["nft"].as_str().unwrap());
    }
}

#[test]
fn engine_matches_python() {
    let Some(c) = corpus() else { return };
    let mut sent_total = 0;
    for sc in c["engine"].as_array().unwrap() {
        let cfg = Config::sanitize(&sc["cfg"]);
        let logs = Arc::new(Mutex::new(Vec::<String>::new()));
        let l2 = logs.clone();
        let stats = Arc::new(Stats::default());
        let mut e = Engine::new(cfg, stats.clone(), Arc::new(move |m: &str| l2.lock().unwrap().push(m.to_string())));
        e.set_junk(|b| b.fill(0));
        for st in sc["steps"].as_array().unwrap() {
            let raw = hex::decode(st["raw"].as_str().unwrap()).unwrap();
            let got: Vec<(String, bool)> = match e.process(&raw, st["out"].as_bool().unwrap()) {
                Verdict::Pass => vec![(hex::encode(&raw), false)],
                Verdict::Drop => vec![],
                Verdict::Send(v) => v.into_iter().map(|o| (hex::encode(o.data), o.recalc)).collect(),
            };
            let want: Vec<(String, bool)> = st["sent"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| (s[0].as_str().unwrap().to_string(), s[1].as_bool().unwrap()))
                .collect();
            sent_total += want.len();
            assert_eq!(got, want, "cfg {} paket {}", sc["cfg"], st["raw"]);
        }
        assert_eq!(stats.split(), sc["split"].as_u64().unwrap(), "split {}", sc["cfg"]);
        assert_eq!(stats.skip(), sc["skip"].as_u64().unwrap(), "skip {}", sc["cfg"]);
        let want_logs: Vec<String> = serde_json::from_value(sc["logs"].clone()).unwrap();
        assert_eq!(*logs.lock().unwrap(), want_logs);
    }
    assert!(sent_total > 500);
}
