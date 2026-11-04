//! GDPI masaüstü uygulaması (Python `gdpi.py` arayüz kısmının karşılığı).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
mod engine;
mod selftest;
mod shield;
mod sitecheck;
mod state;
mod tray;
mod ui;

use gdpi_core::Config;

/// Pencereli exe hatayı gösteremez: dosyaya yaz.
fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let _ = gdpi_core::config::safe_write(&Config::data_dir().join("hata.txt"), format!("{info}\n\n{bt}\n").as_bytes());
        prev(info);
    }));
}

#[cfg(target_os = "linux")]
fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p).map(|d| d.join(name)).find(|f| {
            use std::os::unix::fs::PermissionsExt;
            f.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
        })
    })
}

/// Linux: pkexec (yoksa sudo) ile root olarak yeniden başlatır; masaüstü oturum değişkenleri korunur.
#[cfg(target_os = "linux")]
fn linux_elevate() -> ! {
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    const KEEP: &[&str] = &[
        "DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "HOME",
        "DBUS_SESSION_BUS_ADDRESS", "LANG",
    ];
    let set = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let env: Vec<String> = KEEP.iter().filter_map(|k| set(k).map(|v| format!("{k}={v}"))).collect();
    if set("WAYLAND_DISPLAY").is_some() && which("xhost").is_some() {
        // XWayland'de root'a pencere izni
        let _ = Command::new("xhost").arg("+SI:localuser:root").output();
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| "gdpi".into());
    let mut args: Vec<std::ffi::OsString> = vec![exe.into(), "--no-elevate".into()];
    args.extend(std::env::args_os().skip(1));
    let tool = if which("pkexec").is_some() && set("DISPLAY").is_some() { "pkexec" } else { "sudo" };
    let err = Command::new(tool).arg("env").args(&env).args(&args).exec();
    eprintln!("{tool} çalıştırılamadı: {err}");
    std::process::exit(1);
}

fn main() {
    install_panic_hook();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |a: &str| args.iter().any(|x| x == a);

    #[cfg(target_os = "linux")]
    if !engine::is_admin() && !has("--no-elevate") {
        linux_elevate();
    }

    if has("--selftest") {
        if let Err(e) = selftest::run() {
            eprintln!("selftest.txt yazılamadı: {e}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = ui::run(has("--tray")) {
        let _ = gdpi_core::config::safe_write(&Config::data_dir().join("hata.txt"), format!("{e}\n").as_bytes());
        eprintln!("{e}");
        std::process::exit(1);
    }
}
