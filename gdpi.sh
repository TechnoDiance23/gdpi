#!/usr/bin/env bash
# GDPI Linux başlatıcı: ilk çalıştırmada .venv kurar, sonra GDPI'ı root (pkexec/sudo) ile açar.
# Gerekenler: python3, python3-venv, python3-tk, nftables (ör. sudo apt install python3-venv python3-tk nftables)
set -e
cd "$(dirname "$0")"
if [ ! -x .venv/bin/python ]; then
    python3 -m venv .venv
    .venv/bin/python -m pip install -q -r requirements.txt
fi
exec .venv/bin/python gdpi.py "$@"
