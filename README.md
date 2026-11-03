<p align="center">
  <img src="docs/acik.png" width="720" alt="GDPI ana ekran">
</p>

<h1 align="center">GDPI — Görüşürüz Derin Paket İnceleme 👋</h1>

<p align="center">
  Windows için tek tıkla çalışan, arayüzlü ve <b>siteye göre akıllı</b> bir DPI aşma aracı.<br>
  GoodbyeDPI'ın mantığı, ama bankanı, e-Devlet'ini ve Grok'unu bozmadan.
</p>

<p align="center">
  <a href="../../releases/latest"><b>⬇️ GDPI.exe'yi indir</b></a>
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
1. [Releases](../../releases/latest) sayfasından **GDPI.exe**'yi indir.
2. Çift tıkla, yönetici iznine "Evet" de. (Ağ paketlerini işleyebilmek için gerekiyor.)
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

Kendi exe'ni derlemek istersen: `powershell -ExecutionPolicy Bypass -File build.ps1` → `dist\GDPI.exe`

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
- GoodbyeDPI veya benzeri bir araçla **aynı anda çalıştırma**, ikisi de aynı sürücüyü (WinDivert) kullanır.
- Ayarların `%APPDATA%\GDPI\ayarlar.json` dosyasında durur.
- GDPI bir VPN değildir. IP adresini gizlemez, sadece operatörün paket incelemesini atlatır.
- Hiçbir veri toplamaz, hiçbir yere bağlanmaz. Sadece senin bilgisayarındaki paketleri yeniden düzenler.

## Nasıl çalışıyor? (meraklısına)

Operatörler, şifreli bağlantının ilk paketindeki açık metin alan adına (SNI) bakarak engelleme yapar. GDPI,
[WinDivert](https://reqrypt.org/windivert.html) ile giden paketi yakalar, alan adının tam ortasından iki TCP
parçasına böler ve önüne operatörün göreceği, ama sunucunun asla kabul etmeyeceği bir sahte paket koyar.
Operatörün DPI'ı alan adını bütün halinde göremez, sunucu ise parçaları normal şekilde birleştirir.

## Teşekkürler ve lisans

- Fikir: [GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI) (ValdikSS) ve [zapret](https://github.com/bol-van/zapret). GDPI onların kodunu içermez, aynı yaklaşımı Python ile sıfırdan uygular.
- Paket yakalama: [WinDivert](https://reqrypt.org/windivert.html) (LGPLv3) + [pydivert](https://github.com/ffalcinelli/pydivert) (LGPLv3).
- Arayüz: [CustomTkinter](https://github.com/TomSchimansky/CustomTkinter) (MIT), [pystray](https://github.com/moses-palmer/pystray) (LGPLv3), Pillow.

GDPI'ın kendi kodu **MIT** lisanslıdır, bkz. [LICENSE](LICENSE). Üçüncü parti lisanslar için [THIRD_PARTY.md](THIRD_PARTY.md).

Görüşürüz, derin paket inceleme. 👋
