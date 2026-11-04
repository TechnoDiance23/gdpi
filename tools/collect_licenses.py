"""
Derleme sırasında dağıtım paketine (dist/GDPI/licenses) lisans dosyalarını toplar.

Neyi toplar:
  * Depodaki LICENSE (GDPI, MIT), THIRD_PARTY.md ve licenses/ klasörü
  * GERÇEKTEN kurulu çalışma zamanı bağımlılıklarının (pystray, Pillow, CustomTkinter
    ve onların bağımlılıkları) .dist-info içindeki lisans dosyaları; böylece sürüm
    değişse bile pakete o sürümün kendi lisans metni girer
  * Derlemede kullanılan Python'un LICENSE.txt dosyası (Windows yapısında OpenSSL,
    libffi, bzip2, Microsoft Distributable Code vb. koşulları da içerir) ve Tcl/Tk
    license.terms dosyaları
  * PyInstaller COPYING.txt (bootloader istisnası, Apache-2.0 run-time hook'lar)
  * windivert/LICENSE (fetch_windivert.ps1 resmi zip'ten kopyalar)
  * Sürümleri ve kaynak kodu adreslerini listeleyen MANIFEST.txt

Eksik bir zorunlu lisans dosyası varsa hata verip derlemeyi durdurur.

Kullanım: python tools/collect_licenses.py <hedef_klasör>
"""
import re
import shutil
import sys
import sysconfig
from importlib import metadata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Uygulamanın doğrudan bağımlılıkları (requirements.txt ile aynı)
DIRECT = ["pystray", "pillow", "customtkinter"]

# Kaynak kodu adresleri (LGPL/GPL bileşenleri için bkz. THIRD_PARTY.md)
SOURCE = {
    "pystray": "https://github.com/moses-palmer/pystray (etiket v{v})",
    "pillow": "https://github.com/python-pillow/Pillow (etiket {v})",
    "customtkinter": "https://github.com/TomSchimansky/CustomTkinter (etiket v{v})",
    "darkdetect": "https://github.com/albertosottile/darkdetect",
    "packaging": "https://github.com/pypa/packaging",
    "six": "https://github.com/benjaminp/six",
}


# Meta verisi LICENSE dosyasıyla çelişen paketler (LICENSE dosyası esas alınır)
LICENSE_OVERRIDE = {"customtkinter": "MIT (LICENSE dosyası)",
                    "pyinstaller-hooks-contrib": "rthooks Apache-2.0; diğerleri GPL-2.0+"}


def norm(name):
    return re.sub(r"[-_.]+", "-", name).lower()


def runtime_deps():
    """Doğrudan bağımlılıklar + Windows'ta geçerli dolaylı bağımlılıklar."""
    seen, todo = {}, list(DIRECT)
    env = {"sys_platform": "win32", "platform_system": "Windows", "extra": ""}
    while todo:
        name = norm(todo.pop())
        if name in seen:
            continue
        dist = metadata.distribution(name)
        seen[name] = dist
        for req in dist.requires or []:
            base, _, marker = req.partition(";")
            if marker and not _marker_ok(marker, env):
                continue
            todo.append(re.split(r"[\s<>=!~\[(]", base.strip(), maxsplit=1)[0])
    return seen


def _marker_ok(marker, env):
    try:
        from packaging.markers import Marker
        return Marker(marker).evaluate(env)
    except ImportError:            # packaging yoksa: extra/darwin/linux içerenleri atla
        return not re.search(r"extra|darwin|linux", marker, re.I)


def license_files(dist):
    out = []
    for f in dist.files or []:
        p = str(f).replace("\\", "/")
        if ".dist-info/" in p and re.search(r"(LICEN[CS]E|COPYING|NOTICE|AUTHORS)", p.split("/")[-1], re.I):
            out.append(Path(dist.locate_file(f)))
    return out


def main(dest):
    if hasattr(sys.stdout, "reconfigure"):     # Windows konsolu (cp1252) Türkçe karakterde hata vermesin
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    dest = Path(dest)
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    missing, lines = [], []

    shutil.copy2(ROOT / "LICENSE", dest / "GDPI-LICENSE.txt")
    shutil.copy2(ROOT / "THIRD_PARTY.md", dest / "THIRD_PARTY.md")
    shutil.copytree(ROOT / "licenses", dest / "static", dirs_exist_ok=True)
    lines.append("GDPI (bu program)                 MIT        GDPI-LICENSE.txt")

    # Python paketleri: kurulu sürümün kendi lisans dosyaları
    for name, dist in sorted(runtime_deps().items()):
        files = license_files(dist)
        if not files:
            missing.append(f"{name}: .dist-info içinde lisans dosyası yok")
        for f in files:
            shutil.copy2(f, dest / f"{name}-{dist.version}-{f.name}")
        lic = LICENSE_OVERRIDE.get(name) or dist.metadata.get("License-Expression") \
            or dist.metadata.get("License") or "?"
        src = SOURCE.get(name, "https://pypi.org/project/" + name).format(v=dist.version)
        lines.append(f"{name} {dist.version:<24} {lic[:40]:<40} kaynak: {src}")

    # PyInstaller (bootloader + run-time hook'lar exe'ye gömülür)
    for tool in ("pyinstaller", "pyinstaller-hooks-contrib"):
        try:
            pyi = metadata.distribution(tool)
        except metadata.PackageNotFoundError:
            missing.append(f"{tool} kurulu değil")
            continue
        for f in license_files(pyi):
            shutil.copy2(f, dest / f"{tool}-{pyi.version}-{f.name}")
        lic = ("GPL-2.0-or-later + bootloader istisnası; rthooks Apache-2.0" if tool == "pyinstaller"
               else LICENSE_OVERRIDE[tool])
        lines.append(f"{tool} {pyi.version}  {lic}  kaynak: https://github.com/pyinstaller/{tool}")

    # Python ve Tcl/Tk
    base = Path(sys.base_prefix)
    py_lic = next((p for p in (base / "LICENSE.txt", base / "LICENSE",
                               Path(sysconfig.get_paths()["stdlib"]) / "LICENSE.txt") if p.exists()), None)
    if py_lic:
        shutil.copy2(py_lic, dest / f"Python-{sys.version.split()[0]}-LICENSE.txt")
    else:
        missing.append(f"Python LICENSE.txt bulunamadı ({base})")
    for terms in sorted(base.glob("tcl/*/license.terms")):
        shutil.copy2(terms, dest / f"{terms.parent.name}-license.terms")
    lines.append(f"Python {sys.version.split()[0]}  PSF-2.0 (+ Windows yapısındaki üçüncü parti koşulları)"
                 "  kaynak: https://www.python.org/downloads/source/")

    # WinDivert
    wd = ROOT / "windivert" / "LICENSE"
    if wd.exists():
        shutil.copy2(wd, dest / "WinDivert-LICENSE.txt")
    elif sys.platform == "win32":
        missing.append("windivert/LICENSE yok (fetch_windivert.ps1 çalıştırın)")
    lines.append("WinDivert 2.2.2  LGPL-3.0 seçildi (çift lisans: LGPL-3.0 veya GPL-2.0)"
                 "  kaynak: https://github.com/basil00/WinDivert (etiket v2.2.2)")

    if missing:
        raise SystemExit("Lisans toplama başarısız:\n  " + "\n  ".join(missing))

    (dest / "MANIFEST.txt").write_text(
        "GDPI dağıtım paketindeki bileşenler (derleme sırasında otomatik üretildi)\n"
        "Ayrıntılar ve yükümlülükler: THIRD_PARTY.md\n\n" + "\n".join(lines) + "\n",
        encoding="utf-8")
    print(f"Lisanslar toplandı: {dest} ({sum(1 for _ in dest.rglob('*'))} dosya)")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "dist/GDPI/licenses")
