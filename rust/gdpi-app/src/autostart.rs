//! Windows ile başlat (Python `autostart_enabled` / `set_autostart`).

#[cfg(windows)]
const TASK: &str = "GDPI";

#[cfg(windows)]
fn schtasks(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    schtasks(&["/query", "/tn", TASK])
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_autostart(on: bool) -> Option<String> {
    if on {
        Some("Linux'ta otomatik başlatma yok; gdpi'yi oturum başlangıcına ekleyebilirsiniz.".into())
    } else {
        None
    }
}

#[cfg(windows)]
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let t = e.file_type()?;
        if t.is_dir() {
            copy_dir(&e.path(), &dst.join(e.file_name()))?;
        } else if t.is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name()))?;
        }
    }
    Ok(())
}

/// Oturum açılınca yönetici olarak, UAC sormadan, tepside ve açık başlar.
/// Görev yalnızca Program Files'a kopyalanan exe'yi çalıştırır: oraya sadece yöneticiler yazabilir,
/// böylece kullanıcı klasöründeki dosyayı değiştiren biri yönetici yetkisi kazanamaz.
/// Başarılıysa None, değilse açıklama döndürür.
#[cfg(windows)]
pub fn set_autostart(on: bool) -> Option<String> {
    use std::path::{Path, PathBuf};
    if !on {
        schtasks(&["/delete", "/f", "/tn", TASK]);
        return None;
    }
    // Ortam değişkeni kullanıcı tarafından değiştirilebilir; yolu HKLM'den (yalnızca yöneticiler yazar) oku
    let pf: String = match winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion")
        .and_then(|k| k.get_value("ProgramFilesDir"))
    {
        Ok(v) => v,
        Err(e) => return Some(format!("Program Files'a kopyalanamadı: {e}")),
    };
    let dest_dir = Path::new(&pf).join("GDPI");
    let dest = dest_dir.join("GDPI.exe");
    let copy = || -> std::io::Result<()> {
        let exe = std::env::current_exe()?;
        let src_dir: PathBuf = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let same = src_dir.to_string_lossy().to_lowercase() == dest_dir.to_string_lossy().to_lowercase();
        if same {
            return Ok(());
        }
        // Sadece GDPI.exe, windivert klasörü ve lisanslar kopyalanır; klasördeki başka (araya sokulmuş)
        // dosyalar taşınmaz. Eski kurulum önce tamamen silinir ki önceden bırakılmış dosya kalmasın.
        if dest_dir.is_dir() {
            std::fs::remove_dir_all(&dest_dir)?;
        }
        std::fs::create_dir_all(&dest_dir)?;
        std::fs::copy(&exe, &dest)?;
        let wd = gdpi_windows::windivert_dir();
        let wd_dest = dest_dir.join(wd.file_name().unwrap_or_else(|| "windivert".as_ref()));
        std::fs::create_dir_all(&wd_dest)?;
        for f in ["WinDivert.dll", "WinDivert64.sys", "LICENSE", "VERSION"] {
            let s = wd.join(f);
            if f.starts_with("WinDivert") || s.is_file() {
                std::fs::copy(&s, wd_dest.join(f))?;
            }
        }
        let lic = src_dir.join("licenses");
        if lic.is_dir() {
            copy_dir(&lic, &dest_dir.join("licenses"))?;
        }
        for f in ["LICENSE", "THIRD_PARTY.md"] {
            let s = src_dir.join(f);
            if s.is_file() {
                std::fs::copy(&s, dest_dir.join(f))?;
            }
        }
        Ok(())
    };
    if let Err(e) = copy() {
        return Some(format!("Program Files'a kopyalanamadı: {e}"));
    }
    let tr = format!("\"{}\" --tray", dest.display());
    if schtasks(&["/create", "/f", "/tn", TASK, "/sc", "onlogon", "/rl", "highest", "/tr", &tr]) {
        None
    } else {
        Some("Zamanlanmış görev oluşturulamadı.".into())
    }
}
