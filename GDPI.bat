@echo off
cd /d "%~dp0"
python -c "import pydivert, pystray, PIL, customtkinter" 2>nul || python -m pip install -r requirements.txt
start "" pythonw gdpi.py
