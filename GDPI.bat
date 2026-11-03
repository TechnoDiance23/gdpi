@echo off
cd /d "%~dp0"
python -c "import pystray, PIL, customtkinter" 2>nul || python -m pip install -r requirements.txt
if not exist windivert\WinDivert64.sys powershell -NoProfile -ExecutionPolicy Bypass -File fetch_windivert.ps1
start "" pythonw gdpi.py
