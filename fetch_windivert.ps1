# WinDivert 2.2.2'yi (GoodbyeDPI ile aynı sürücü) resmi sürümden windivert\ klasörüne getirir,
# zip'in SHA256 değerini ve sürücü imzasını doğrular. build.ps1 ve GDPI.bat kullanır.
# WinDivert LGPLv3 / GPLv2 çift lisanslıdır; lisans metni (LICENSE) dosyaları da kopyalanır.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$zipSha256 = "63CB41763BB4B20F600B6DE04E991A9C2BE73279E317D4D82F237B150C5F3F15"
if (-not (Test-Path windivert\WinDivert64.sys) -or -not (Test-Path windivert\LICENSE)) {
    $zip = "$env:TEMP\WinDivert-2.2.2-A.zip"
    Invoke-WebRequest https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip -OutFile $zip
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash
    if ($hash -ne $zipSha256) { throw "WinDivert zip SHA256 uyuşmuyor: $hash" }
    Expand-Archive $zip "$env:TEMP\windivert-x" -Force
    $src = "$env:TEMP\windivert-x\WinDivert-2.2.2-A"
    New-Item -ItemType Directory -Force windivert | Out-Null
    Copy-Item "$src\x64\WinDivert.dll", "$src\x64\WinDivert64.sys", "$src\LICENSE", "$src\VERSION" windivert\
}
$sig = Get-AuthenticodeSignature windivert\WinDivert64.sys
if ($sig.Status -ne "Valid") { throw "WinDivert64.sys imzası geçersiz: $($sig.Status)" }
Write-Host "WinDivert hazır. İmza: $($sig.SignerCertificate.Subject)"
