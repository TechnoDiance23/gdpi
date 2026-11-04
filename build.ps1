# GDPI derleme betiği (yerelde ve GitHub Actions'ta aynı)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
python -m pip install -r requirements.txt pyinstaller
if ($LASTEXITCODE) { throw "pip install başarısız" }

& "$PSScriptRoot\fetch_windivert.ps1"          # WinDivert 2.2.2, SHA256 + imza doğrulamalı
Get-FileHash windivert\* | Format-Table Hash, Path -AutoSize

# onedir: dosyalar %TEMP%'e açılmaz, yönetici olarak çalışırken kullanıcı klasöründen DLL yüklenmez
python -m PyInstaller --noconfirm --clean --onedir --windowed --uac-admin `
    --name GDPI --icon assets\gdpi.ico `
    --collect-all customtkinter --hidden-import pystray._win32 `
    --add-binary "windivert\WinDivert.dll;windivert" --add-binary "windivert\WinDivert64.sys;windivert" `
    gdpi.py
if ($LASTEXITCODE) { throw "PyInstaller başarısız" }

# Lisans dosyaları: dist\GDPI\licenses (bileşenlerin tam lisans metinleri + MANIFEST.txt)
python tools\collect_licenses.py dist\GDPI\licenses
if ($LASTEXITCODE) { throw "Lisans dosyaları toplanamadı" }
Copy-Item LICENSE, THIRD_PARTY.md dist\GDPI\

Compress-Archive -Path dist\GDPI -DestinationPath dist\GDPI-windows.zip -Force
Write-Host "Hazir: dist\GDPI\GDPI.exe ve dist\GDPI-windows.zip"
