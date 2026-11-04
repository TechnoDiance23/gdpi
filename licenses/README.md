# Lisans metinleri

Bu klasör GDPI ile dağıtılan üçüncü parti bileşenlerin resmî lisans dosyalarının değiştirilmemiş kopyalarını içerir.
Özet tablo ve yükümlülükler için bkz. [../THIRD_PARTY.md](../THIRD_PARTY.md).

| Dosya | Bileşen | Alındığı yer |
|---|---|---|
| `WinDivert-2.2.2-LICENSE.txt` | WinDivert 2.2.2 (LGPLv3, GPLv3, GPLv2 tam metinleri) | `WinDivert-2.2.2-A.zip` resmî sürüm |
| `pystray-COPYING-LGPL-3.0.txt`, `pystray-COPYING-GPL-3.0.txt` | pystray 0.19.5 | PyPI wheel `.dist-info` |
| `CustomTkinter-LICENSE.txt` | CustomTkinter (MIT) | PyPI wheel `.dist-info` |
| `Pillow-LICENSE.txt` | Pillow (MIT-CMU); gömülü kütüphanelerin lisansları derlemede wheel'den eklenir | PyPI wheel `.dist-info` |
| `darkdetect-LICENSE.txt` | darkdetect (BSD-3-Clause) | PyPI wheel `.dist-info` |
| `packaging-LICENSE.txt`, `packaging-LICENSE.BSD.txt`, `Apache-2.0.txt` | packaging (Apache-2.0 OR BSD-2-Clause); Apache-2.0 metni PyInstaller run-time hook'ları ve OpenSSL için de geçerlidir | PyPI wheel `.dist-info` |
| `six-LICENSE.txt` | six (MIT) | PyPI wheel `.dist-info` |
| `Python-LICENSE.txt` | Python 3.12 Windows yapısı (PSF-2.0 + gömülü kütüphaneler) | python.org 3.12.10 embeddable paket |
| `PyInstaller-COPYING.txt` | PyInstaller (GPL-2.0-or-later + bootloader istisnası) | PyPI wheel `.dist-info` |
| `hev-socks5-tunnel-LICENSE.txt`, `hev-socks5-core-LICENSE.txt`, `hev-task-system-LICENSE.txt`, `hev-yaml-LICENSE.txt` | Android: hev-socks5-tunnel 2.19.3 ve alt modülleri (MIT) | kaynak deposu, etiket `2.19.3` |
| `lwip-LICENSE.txt` | Android: lwIP (BSD-3-Clause, hev-socks5-tunnel alt modülü) | kaynak deposu, etiket `2.19.3` |

Derleme (`build.ps1`) bu klasörü `dist\GDPI\licenses\static\` altına kopyalar ve ayrıca kurulu sürümlerin kendi lisans
dosyalarını `tools/collect_licenses.py` ile toplar.
