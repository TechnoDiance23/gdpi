//! `gdpi --selftest`: motoru gerçekten başlatır, siteleri dener, sonucu
//! data_dir()/selftest.txt dosyasına yazar ve çıkar (Python `selftest`).
use crate::engine;
use crate::sitecheck::{py_repr, site_check};
use gdpi_core::{Config, LogFn, Stats, VERSION};
use std::sync::{Arc, Mutex};

const HOSTS: &[&str] = &[
    "discord.com", "cdn.discordapp.com", "www.isbank.com.tr", "www.resmigazete.gov.tr",
    "grok.com", "x.com", "www.google.com",
];

fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

pub fn run() -> std::io::Result<std::path::PathBuf> {
    let out = Arc::new(Mutex::new(Vec::<String>::new()));
    let say = |s: String| out.lock().unwrap().push(s);
    say(format!(
        "GDPI {VERSION} öz test  {}  yönetici={}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        py_bool(engine::is_admin())
    ));
    say("-- GDPI kapalı:".into());
    for h in HOSTS {
        say(format!("   {h:28} {}", py_repr(&site_check(h))));
    }
    let cfg = Config::load();
    let stats = Arc::new(Stats::default());
    let o2 = out.clone();
    let log: LogFn = Arc::new(move |m: &str| o2.lock().unwrap().push(format!("   log {m}")));
    match engine::start(cfg, stats.clone(), log) {
        Ok(mut e) => {
            say("-- GDPI açık (motor başladı):".into());
            for h in HOSTS {
                say(format!("   {h:28} {}", py_repr(&site_check(h))));
            }
            say(format!("-- bölünen={}  dokunulmayan={}", stats.split(), stats.skip()));
            e.stop();
        }
        Err(ex) => say(format!("-- MOTOR BAŞLAMADI: {ex}")),
    }
    let dir = Config::data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("selftest.txt");
    let text = out.lock().unwrap().join("\n") + "\n";
    std::fs::write(&path, text)?;
    Ok(path)
}
