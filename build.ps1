# GDPI.exe derleme betiği (yerelde ve GitHub Actions'ta aynı)
$ErrorActionPreference = "Stop"
python -m pip install -r requirements.txt pyinstaller
python -m PyInstaller --noconfirm --clean --onefile --windowed --uac-admin `
    --name GDPI --icon assets\gdpi.ico `
    --collect-all pydivert --collect-all customtkinter `
    --hidden-import pystray._win32 `
    gdpi.py
Write-Host "Hazir: dist\GDPI.exe"
