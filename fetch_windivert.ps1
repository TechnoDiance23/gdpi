# WinDivert 2.2.2'yi (GoodbyeDPI ile aynı sürücü) resmi sürümden windivert\ klasörüne getirir
# ve sürücü imzasını doğrular. build.ps1 ve GDPI.bat kullanır.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
if (-not (Test-Path windivert\WinDivert64.sys)) {
    $zip = "$env:TEMP\WinDivert-2.2.2-A.zip"
    Invoke-WebRequest https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip -OutFile $zip
    Expand-Archive $zip "$env:TEMP\windivert-x" -Force
    New-Item -ItemType Directory -Force windivert | Out-Null
    Copy-Item "$env:TEMP\windivert-x\WinDivert-2.2.2-A\x64\WinDivert.dll", "$env:TEMP\windivert-x\WinDivert-2.2.2-A\x64\WinDivert64.sys" windivert\
}
$sig = Get-AuthenticodeSignature windivert\WinDivert64.sys
if ($sig.Status -ne "Valid") { throw "WinDivert64.sys imzası geçersiz: $($sig.Status)" }
Write-Host "WinDivert hazır. İmza: $($sig.SignerCertificate.Subject)"
