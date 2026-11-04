# Üçüncü parti bileşenler ve lisanslar

GDPI'ın kendi kaynak kodu (`gdpi.py`, betikler) **MIT** lisanslıdır, bkz. [LICENSE](LICENSE). MIT lisansı yalnızca
GDPI'ın kendi koduna uygulanır; aşağıdaki bileşenler **kendi lisansları altında** kalır ve bu lisanslar MIT ile
değiştirilmez.

> Bu belge, bileşenlerin resmî lisans dosyaları incelenerek hazırlanmış teknik bir uyumluluk kaydıdır, hukuki görüş
> değildir. Kesin değerlendirme için bir hukukçuya danışın.

## Bileşen listesi

`GDPI-windows.zip` (PyInstaller, onedir) aşağıdakileri içerir. Tam lisans metinleri depoda [`licenses/`](licenses/)
klasöründe, dağıtım paketinde ise `licenses/` klasöründedir (derlemede kurulu sürümlerden otomatik toplanır, bkz.
`tools/collect_licenses.py`; gerçek sürümler `licenses/MANIFEST.txt` içinde yazılıdır).

| Bileşen | Türü | Lisans (SPDX) | Lisans dosyası | Kaynak |
|---|---|---|---|---|
| WinDivert 2.2.2 (`WinDivert.dll`, `WinDivert64.sys`; resmî imzalı ikililer, değiştirilmemiş) | doğrudan | `LGPL-3.0-only OR GPL-2.0-only` → GDPI dağıtımı için **LGPL-3.0** seçilmiştir | `WinDivert-2.2.2-LICENSE.txt` (LGPLv3, GPLv3, GPLv2 metinleri) | https://github.com/basil00/WinDivert (etiket `v2.2.2`) |
| pystray 0.19.5 | doğrudan | `LGPL-3.0` (LGPLv3 + GPLv3 metni) | `pystray-COPYING-LGPL-3.0.txt`, `pystray-COPYING-GPL-3.0.txt` | https://github.com/moses-palmer/pystray (etiket `v0.19.5`) |
| CustomTkinter | doğrudan | `MIT` | `CustomTkinter-LICENSE.txt` | https://github.com/TomSchimansky/CustomTkinter |
| Pillow | doğrudan | `MIT-CMU` (HPND türevi) | `Pillow-LICENSE.txt` | https://github.com/python-pillow/Pillow |
| Pillow içine gömülü kütüphaneler (FreeType, HarfBuzz, libjpeg-turbo, libpng, libtiff, libwebp, libavif, OpenJPEG, LittleCMS, Brotli, xz, zlib-ng…) | dolaylı | her biri kendi lisansı; FreeType için FTL seçilebilir (`FTL OR GPL-2.0-or-later`) | Pillow wheel'inin tam `LICENSE` dosyası (dağıtımda `pillow-<sürüm>-LICENSE`) | Pillow kaynak deposu |
| darkdetect (CustomTkinter bağımlılığı) | dolaylı | `BSD-3-Clause` | `darkdetect-LICENSE.txt` | https://github.com/albertosottile/darkdetect |
| packaging (CustomTkinter bağımlılığı) | dolaylı | `Apache-2.0 OR BSD-2-Clause` → BSD-2-Clause seçilebilir | `packaging-LICENSE*.txt`, `Apache-2.0.txt` | https://github.com/pypa/packaging |
| six (pystray bağımlılığı) | dolaylı | `MIT` | `six-LICENSE.txt` | https://github.com/benjaminp/six |
| Python 3.12 çalışma zamanı + standart kütüphane, Tcl/Tk, OpenSSL, libffi, bzip2, SQLite, Microsoft VC++ çalışma zamanı | gömülü | `PSF-2.0`; diğerleri Python'un Windows `LICENSE.txt` dosyasındaki koşullar (OpenSSL 3: Apache-2.0, Tcl/Tk: TCL lisansı…) | `Python-LICENSE.txt`, dağıtımda `tcl8.6-license.terms`, `tk8.6-license.terms` | https://www.python.org/downloads/source/ |
| PyInstaller bootloader ve run-time hook'lar | gömülü (derleme aracı) | `GPL-2.0-or-later` + **bootloader istisnası**; run-time hook'lar `Apache-2.0` | `PyInstaller-COPYING.txt`, `Apache-2.0.txt` | https://github.com/pyinstaller/pyinstaller |
| pyinstaller-hooks-contrib run-time hook'ları (yalnızca ilgili paket kullanılıyorsa gömülür) | gömülü (derleme aracı) | gömülen run-time hook'lar `Apache-2.0` (diğer hook'lar GPL-2.0-or-later, yalnızca derlemede çalışır) | dağıtımda `pyinstaller-hooks-contrib-<sürüm>-*` | https://github.com/pyinstaller/pyinstaller-hooks-contrib |

### Android sürümü

`GDPI-android.apk` GDPI'ın kendi Kotlin kodunu ve aşağıdaki yerel kütüphaneyi içerir (`libhev-socks5-tunnel.so`,
derleme sırasında `android/fetch_hev.sh` ile sabit sürümden derlenir, değiştirilmemiştir). Lisans metinleri APK
içinde `assets/licenses/` klasöründedir.

| Bileşen | Lisans (SPDX) | Lisans dosyası | Kaynak |
|---|---|---|---|
| hev-socks5-tunnel 2.19.3 | `MIT` | `hev-socks5-tunnel-LICENSE.txt` | https://github.com/heiher/hev-socks5-tunnel (etiket `2.19.3`) |
| hev-socks5-core (alt modül) | `MIT` | `hev-socks5-core-LICENSE.txt` | https://github.com/heiher/hev-socks5-core |
| hev-task-system (alt modül) | `MIT` | `hev-task-system-LICENSE.txt` | https://github.com/heiher/hev-task-system |
| yaml (alt modül, hev'in libyaml uyarlaması) | `MIT` | `hev-yaml-LICENSE.txt` | https://github.com/heiher/yaml |
| lwIP (alt modül) | `BSD-3-Clause` | `lwip-LICENSE.txt` | https://github.com/heiher/lwip |

APK'da başka üçüncü parti kütüphane yok (AndroidX bile kullanılmıyor), yalnızca Android'in kendi API'leri var.

### Linux sürümü

Linux için hazır paket dağıtılmaz; GDPI kaynaktan (`gdpi.sh`) çalışır ve **hiçbir üçüncü parti bileşeni
dağıtmaz**. Bileşenler kullanıcının sistemine kullanıcı tarafından kurulur:

| Bileşen | Lisans | Nasıl kullanılır |
|---|---|---|
| python-xlib (Linux'ta pystray'in bağımlılığı, pip kurar) | `LGPL-2.1-or-later` | Python `import` |
| nftables (`nft` komutu, dağıtım paketi) | `GPL-2.0-only` | Ayrı bir program olarak çalıştırılır (`subprocess`); GDPI'a bağlanmaz/gömülmez |
| Linux çekirdeği NFQUEUE (`nfnetlink_queue`) | `GPL-2.0-only` (çekirdek) | Standart netlink soket arabirimi; GDPI saf Python ile konuşur, `libnetfilter_queue` kullanmaz |
| Tcl/Tk (`python3-tk`) | TCL lisansı | Python `tkinter` |

GPL lisanslı bir programı ayrı süreç olarak çalıştırmak ve çekirdeğin kullanıcı alanı arabirimini kullanmak, FSF'nin
SSS'sindeki "aggregate"/ayrı program değerlendirmesine göre genellikle çağıran programı GPL kapsamına almaz
(https://www.gnu.org/licenses/gpl-faq.html#MereAggregation; çekirdek için `COPYING` dosyasındaki "syscall" notu,
https://github.com/torvalds/linux/blob/master/LICENSES/exceptions/Linux-syscall-note). Bu yüzden GDPI'ın MIT lisansı
Linux'ta da korunur. İleride Linux için hazır paket (AppImage, .deb vb.) dağıtılırsa bu tablo ve `licenses/` aynı
Windows paketindeki gibi pakete eklenmelidir.

Windows'ta kaynaktan çalıştırma (`GDPI.bat`) da hiçbir üçüncü parti bileşeni dağıtmaz; paketler kullanıcı tarafından PyPI'dan,
WinDivert resmî GitHub sürümünden indirilir.

## Lisans değerlendirmesi

### GDPI'ın MIT lisansı

MIT; GPL/LGPL ile uyumlu, izin verici bir lisanstır. GDPI LGPL bileşenlerin koduna dahil edilmez; onları değişmeden
kullanır (WinDivert DLL'i `ctypes` ile çalışma anında yüklenir, pystray Python `import` ile kullanılır). Bu yüzden LGPLv3
§4 anlamında GDPI bir "Application", exe ise bir "Combined Work"tür; LGPLv3, Application'ın kendi lisansını seçmesine
izin verir. GDPI'ın kendi kodu MIT olarak kalır.

### WinDivert: LGPLv3 mü GPLv2 mi?

WinDivert'in `LICENSE` dosyası: *"dual-licensed under your choice of the GNU Lesser General Public License (LGPL)
Version 3 or the GNU General Public License (GPL) Version 2"*. GDPI için **LGPLv3 seçilmiştir**:

- GPLv2 seçilirse, birleşik eserin (exe) tamamının GPLv2 ile dağıtılması gerekirdi. Exe ayrıca LGPLv3 lisanslı pystray'i
  içerdiğinden ve LGPLv3/GPLv3 ile "GPLv2-only" uyumsuz olduğundan (FSF lisans uyumluluk tablosu) bu seçim çelişki
  yaratırdı.
- LGPLv3, GDPI kodunun MIT kalmasına izin verir.

### LGPLv3 yükümlülükleri ve nasıl karşılandıkları

Kaynak: LGPLv3 metni §4 ve §6 ile GPLv3 §6 (https://www.gnu.org/licenses/lgpl-3.0.html,
https://www.gnu.org/licenses/gpl-3.0.html).

| Yükümlülük | Karşılanma şekli |
|---|---|
| §4(a) Kütüphanenin kullanıldığına ve LGPL kapsamında olduğuna dair belirgin bildirim | Bu dosya, README "Teşekkürler ve lisans" bölümü, Release notu, dağıtımdaki `licenses/MANIFEST.txt` |
| §4(b) GPLv3 ve LGPLv3 metinlerinin kopyası | `licenses/WinDivert-2.2.2-LICENSE.txt`, `licenses/pystray-COPYING-*.txt` (zip'e otomatik eklenir) |
| §4(c) Çalışma sırasında telif bildirimi gösteriliyorsa kütüphane bildirimi ve lisans referansı | GDPI arayüzü telif bildirimi göstermez (yalnızca sürüm numarası); bu şart tetiklenmez. İleride bir "Hakkında" ekranı eklenirse bu bileşenler ve lisanslar orada da anılmalıdır. |
| §4(d) Kullanıcının kütüphanenin değiştirilmiş bir sürümüyle yeniden birleştirebilmesi | **WinDivert:** DLL ve sürücü exe'ye gömülü değildir; `_internal\windivert\` içinde ayrı dosyalardır ve çalışma anında yüklenir (§4(d)(1) paylaşımlı kütüphane mekanizması). Kullanıcı bunları uyumlu bir sürümle değiştirebilir (sürücü Windows'ta imza gerektirir; bu Windows'un kısıtıdır). **pystray:** PyInstaller saf Python modüllerini exe içindeki arşive koyar; bu yüzden §4(d)(0) yolu izlenir: "Corresponding Application Code" yani GDPI'ın tam kaynak kodu (`gdpi.py`) ve derleme betikleri (`build.ps1`, `fetch_windivert.ps1`, `requirements.txt`) bu depoda MIT lisansıyla sunulur. Değiştirilmiş pystray kurulup `build.ps1` çalıştırıldığında yeni exe üretilir. |
| §4(e) Kurulum bilgisi (yalnızca GPLv3 §6 anlamında "User Product" + gerektiği ölçüde) | Yeniden derleme ve kurulum adımları aşağıda "Değiştirilmiş sürümle yeniden derleme" bölümündedir; ek anahtar/imza gerekmez. |
| §6 / GPLv3 §6 Nesne kodu ile birlikte karşılık gelen kaynak kodu | Her Release'e `GDPI-third-party-sources.zip` eklenir: WinDivert `v2.2.2` ve pystray `v0.19.5` etiketlerinin resmi kaynak arşivleri. Bileşenler değiştirilmediği için karşılık gelen kaynak koddur. pystray 0.19.5'in PyPI'da kaynak paketi (sdist) bulunmadığından kaynak GitHub etiketinden alınır. |

### MIT, MIT-CMU, BSD, PSF, Apache-2.0

Bu lisansların ikili dağıtımdaki ortak yükümlülüğü telif ve izin bildiriminin (lisans metninin) dağıtımla birlikte
verilmesidir; kaynak kodu sunma zorunluluğu yoktur. Apache-2.0 bileşenler için lisans metni verilir; bu sürümlerde
ayrı bir `NOTICE` dosyası bulunmamaktadır (derlemedeki dist-info dosyaları otomatik kopyalanır, varsa `NOTICE` da).
PSF-2.0 ayrıca Python'da yapılan değişikliklerin özetlenmesini ister; GDPI Python'u değiştirmez.

### PyInstaller

PyInstaller'ın kendisi GPL-2.0-or-later'dır, ancak `COPYING.txt` içindeki **bootloader istisnası** exe'ye gömülen
bootloader ve ilgili dosyaların başka programlarla birleştirilip *"kısıtlama olmadan"* dağıtılmasına izin verir. Run-time
hook'lar Apache-2.0'dır. Bu nedenle PyInstaller kullanımı GDPI'ın MIT lisansını etkilemez.

## Değiştirilmiş sürümle yeniden derleme

1. Windows'ta Python 3.12 kurun, depoyu klonlayın.
2. Değiştirilmiş pystray'i kurun: `python -m pip install <değiştirilmiş-pystray-klasörü>` (veya `requirements.txt`'i düzenleyin).
3. Değiştirilmiş WinDivert için `windivert\` klasörüne kendi `WinDivert.dll`/`WinDivert64.sys` dosyalarınızı koyun
   (hazır derlemede `_internal\windivert\` altındaki dosyaları değiştirmek de yeterlidir). Not: `fetch_windivert.ps1`
   sürücü imzasını doğrular; imzasız sürücü Windows tarafından yüklenmez.
4. `./build.ps1` çalıştırın. Çıktı: `dist\GDPI\GDPI.exe` ve `dist\GDPI-windows.zip`.

## Bilinen sınırlamalar

- `requirements.txt` CustomTkinter ve Pillow için alt sınır kullanır; derlenen sürüm değişebilir. Dağıtımdaki lisans
  dosyaları bu yüzden derleme sırasında **kurulu sürümden** toplanır; depodaki `licenses/` kopyaları başvuru amaçlıdır.
- CustomTkinter 5.2.2 ve 6.0.0'ın PyPI meta verisinde "License: Creative Commons Zero v1.0 Universal" yazmaktadır, ancak
  paketin `LICENSE` dosyası ve sınıflandırıcısı **MIT**'tir. Bu belge `LICENSE` dosyasını esas alır.
