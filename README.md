<p align="center">
  <img src="docs/acik.png" width="720" alt="GDPI ana ekran">
</p>

<h1 align="center">GDPI — Görüşürüz Derin Paket İnceleme 👋</h1>

<p align="center">
  Windows için tek tıkla çalışan, arayüzlü ve <b>siteye göre akıllı</b> bir DPI aşma aracı.<br>
  GoodbyeDPI'ın mantığı, ama bankanı, e-Devlet'ini ve Grok'unu bozmadan.
</p>

<p align="center">
  <a href="../../releases/latest"><b>⬇️ GDPI'ı indir</b></a> · Windows (GDPI-windows.zip) · Linux (GDPI-linux.tar.gz)
</p>

---

## Neden yaptım?

GoodbyeDPI işini görüyor ama her siteye müdahale ettiği için bazı şeyler bozuluyordu:
İşbankası takılıyor, Resmî Gazete açılmıyor, Grok uygulaması sorun çıkarıyordu.
GDPI aynı fikri biraz daha nazik uyguluyor:

- 🛡️ **"Asla dokunma" listesi:** bankalar, `gov.tr` siteleri, X ve Grok'a hiç elini sürmez.
- 🎯 **İstersen sadece engelli sitelere** (Discord vb.) uygular, gerisi tamamen normal kalır.
- 🧩 **Daha güvenli sahte paket:** Sahte paketleri varsayılan olarak *bozuk checksum* ile yollar. Yakındaki
  Türk sunucuları bu paketi asla kabul etmez, bağlantı bozulmaz. Düşük TTL de seçilebilir.
- ✂️ **Alan adının tam ortasından bölme:** TLS'te SNI'yi, HTTP'de `Host` başlığını bulup tam ortasından ikiye ayırır.
- 🚫 **QUIC engeli** ve isteğe bağlı **DNS yönlendirme** (operatörün DNS zorlamasına karşı).
- 🔎 **Site testi:** Bir site açılıyor mu, açılmıyorsa neden (DNS mi, DPI mı) tek tıkla görürsün.
- 🔔 **Bildirim alanında yaşar:** Kapatınca sağ alttaki simgeye iner, Windows açılırken sessizce başlayabilir.

## Kurulum

### 1) En kolayı: hazır exe
1. [Releases](../../releases/latest) sayfasından **GDPI-windows.zip**'i indir ve bir klasöre çıkar.
2. İçindeki **GDPI.exe**'ye çift tıkla, yönetici iznine "Evet" de. (Ağ paketlerini işleyebilmek için gerekiyor.)
3. Ortadaki büyük düğmeye bas. Yeşil olduysa tamamdır. ✅

> Windows SmartScreen "tanınmayan uygulama" diyebilir, çünkü exe imzasız.
> **Ek bilgi → Yine de çalıştır** ile açabilirsin. Kaynak kodun tamamı burada, istersen kendin derle (aşağıda).

### 2) winget ile
```
winget install TechnoDiance23.GDPI
```
*(winget kataloğuna başvuru aşamasında. Onaylanana kadar bu komut çalışmayabilir, o zamana kadar 1. yolu kullan.)*

### 3) Kaynaktan çalıştır
```
git clone https://github.com/TechnoDiance23/gdpi.git
cd gdpi
GDPI.bat
```
`GDPI.bat` eksik Python paketlerini kendisi kurar. Python 3.10+ gerekir.

Kendi exe'ni derlemek istersen: `powershell -ExecutionPolicy Bypass -File build.ps1` → `dist\GDPI\GDPI.exe`

### 4) Linux
1. Gerekenleri kur (bir kere):
   ```
   sudo apt install python3-venv python3-tk nftables      # Fedora: sudo dnf install python3-tkinter nftables
   ```
2. [Releases](../../releases/latest) sayfasından **GDPI-linux.tar.gz**'i indir, aç ve çalıştır:
   ```
   tar -xzf GDPI-linux.tar.gz
   cd GDPI
   ./gdpi.sh
   ```

Kaynaktan da olur: `git clone https://github.com/TechnoDiance23/gdpi.git && cd gdpi && ./gdpi.sh`

`gdpi.sh` ilk açılışta `.venv` kurar, sonra GDPI'ı yönetici (pkexec/sudo) izniyle açar. Arayüz ve ayarlar
Windows'takiyle aynıdır; ayarlar `~/.config/GDPI/` altında durur. Linux'ta WinDivert yerine çekirdeğin NFQUEUE
özelliği ve nftables kullanılır (ek bir sürücü ya da kütüphane gerekmez). GDPI kapanınca kurallar silinir; çökerse
bile kural `bypass` modunda olduğundan internet kesilmez (kalıntıyı silmek için: `sudo nft delete table inet gdpi`).
Windows ile başlatma seçeneği Linux'ta yoktur. Tepsi simgesi masaüstüne göre (GNOME'da AppIndicator eklentisi
gerekebilir) çalışmayabilir; o durumda pencereyi kapatmak GDPI'ı kapatır.

## Kullanım

| Ekran | Ne işe yarar |
|---|---|
| **Bağlantı** | Büyük düğme ile aç/kapat. Süre, aşılan ve dokunulmayan bağlantı sayısı. Hızlı profiller: Hafif / Dengeli / Agresif |
| **Siteler** | Solda engelli siteler, sağda asla dokunulmayacaklar. Her satıra bir alan adı yaz, alt alan adları da kapsanır |
| **Site Testi** | Bir siteyi ya da listendekilerin hepsini test et |
| **Ayarlar** | Mod, sahte paket türü, TTL, QUIC, DNS, Windows ile başlat. "Kaydet ve uygula" ile anında geçerli olur |
| **Kayıt** | Hangi sitelere müdahale edildiğini gösterir. Kopyala/Temizle |

<p align="center">
  <img src="docs/siteler.png" width="49%"> <img src="docs/test.png" width="49%">
</p>
<p align="center">
  <img src="docs/ayarlar.png" width="49%"> <img src="docs/kapali.png" width="49%">
</p>

### Bir site açılmıyorsa
1. **Site Testi**'nde dene. "DNS engelli" diyorsa Ayarlar'dan **DNS yönlendir**'i aç.
2. Hızlı profili **Agresif** yap ya da sahte paketi **Düşük TTL** (3–6) olarak dene.
3. Bir site GDPI açıkken bozuluyorsa, adını **Asla dokunma** listesine ekle.

### Bilmen gerekenler
- **GoodbyeDPI'ı tamamen kapat** (servisi varsa `service_remove.cmd`). İkisi aynı anda çalışırsa birbirini bozar,
  bankalar ve devlet siteleri açılmaz.
- Windows Defender, WinDivert'i (GoodbyeDPI'daki gibi) "istenmeyebilecek uygulama" diye işaretleyebilir. Sürücü
  WinDivert'in resmi, imzalı sürücüsüdür. Engellenirse Defender'da GDPI klasörüne izin vermen gerekir.
- Sorun olursa `%APPDATA%\GDPI\cikti.log` ve `GDPI.exe --selftest` (sonuç: `selftest.txt`) yardımcı olur.
- Ayarların `%APPDATA%\GDPI\ayarlar.json` dosyasında durur. GDPI bu dosyayı açarken her değeri kontrol eder,
  geçersiz bir değer (bilinmeyen DNS sunucusu, bozuk alan adı vb.) görürse güvenli varsayılana döner.
- **Windows ile başlat** açılınca GDPI kendi klasörünü `C:\Program Files\GDPI\` içine kopyalar ve oradan başlar.
  Bu klasöre sadece yöneticiler yazabilir, böylece başka bir program GDPI'ın yerine geçip yönetici yetkisi alamaz.
- GDPI bir VPN değildir. IP adresini gizlemez, sadece operatörün paket incelemesini atlatır.
- Hiçbir veri toplamaz, hiçbir yere bağlanmaz. Sadece senin bilgisayarındaki paketleri yeniden düzenler.

## Nasıl çalışıyor? (meraklısına)

Operatörler, şifreli bağlantının ilk paketindeki açık metin alan adına (SNI) bakarak engelleme yapar. GDPI,
[WinDivert](https://reqrypt.org/windivert.html) (Linux'ta NFQUEUE) ile giden paketi yakalar, alan adının tam ortasından iki TCP
parçasına böler ve önüne operatörün göreceği, ama sunucunun asla kabul etmeyeceği bir sahte paket koyar.
Operatörün DPI'ı alan adını bütün halinde göremez, sunucu ise parçaları normal şekilde birleştirir.

## Teşekkürler ve lisans

- Fikir: [GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI) (ValdikSS) ve [zapret](https://github.com/bol-van/zapret). GDPI onların kodunu içermez, aynı yaklaşımı Python ile sıfırdan uygular.
- Paket yakalama: [WinDivert 2.2.2](https://reqrypt.org/windivert.html) (LGPLv3 / GPLv2 çift lisans; GDPI dağıtımında LGPLv3), GoodbyeDPI ile aynı sürücü sürümü.
- Arayüz: [CustomTkinter](https://github.com/TomSchimansky/CustomTkinter) (MIT), [pystray](https://github.com/moses-palmer/pystray) (LGPLv3), [Pillow](https://github.com/python-pillow/Pillow) (MIT-CMU).

GDPI'ın kendi kodu **MIT** lisanslıdır, bkz. [LICENSE](LICENSE). Üçüncü parti lisanslar için [THIRD_PARTY.md](THIRD_PARTY.md); tam metinler [licenses/](licenses/) klasöründe ve
Release zip'indeki `licenses\` klasöründedir. LGPL bileşenlerinin kaynak kodu her Release'e eklenir.

Görüşürüz, derin paket inceleme. 👋
