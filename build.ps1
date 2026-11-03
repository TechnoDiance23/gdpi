# GDPI.exe derleme betiği (yerelde ve GitHub Actions'ta aynı)
$ErrorActionPreference = "Stop"
python -m pip install -r requirements.txt pyinstaller
# onedir: dosyalar %TEMP%'e açılmaz, yönetici olarak çalışırken kullanıcı klasöründen DLL yüklenmez
python -m PyInstaller --noconfirm --clean --onedir --windowed --uac-admin `
    --name GDPI --icon assets\gdpi.ico `
    --collect-all pydivert --collect-all customtkinter `
    --hidden-import pystray._win32 `
    gdpi.py
Compress-Archive -Path dist\GDPI -DestinationPath dist\GDPI-windows.zip -Force
Write-Host "Hazir: dist\GDPI\GDPI.exe ve dist\GDPI-windows.zip"
