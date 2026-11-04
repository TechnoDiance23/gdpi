"""Lisans uyumluluğu kontrolleri: python -m unittest discover -s tests"""
import hashlib
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIC = ROOT / "licenses"


class LicenseFiles(unittest.TestCase):
    def test_gdpi_mit(self):
        text = (ROOT / "LICENSE").read_text(encoding="utf-8")
        self.assertTrue(text.startswith("MIT License"))

    def test_all_referenced_files_exist(self):
        doc = (ROOT / "THIRD_PARTY.md").read_text(encoding="utf-8") + (LIC / "README.md").read_text(encoding="utf-8")
        for name in set(re.findall(r"`([\w.-]+\.txt)`", doc)):
            if name.startswith(("pillow-", "packaging-LICENSE*", "MANIFEST")):
                continue
            if "*" in name or name in ("requirements.txt", "LICENSE.txt", "COPYING.txt"):
                continue
            self.assertTrue((LIC / name).exists(), name)

    def test_full_gnu_texts(self):
        wd = (LIC / "WinDivert-2.2.2-LICENSE.txt").read_text(encoding="utf-8")
        self.assertIn("GNU LESSER GENERAL PUBLIC LICENSE\n                       Version 3", wd)
        self.assertIn("Version 2, June 1991", wd)
        self.assertIn("Version 3, 29 June 2007", (LIC / "pystray-COPYING-GPL-3.0.txt").read_text(encoding="utf-8"))
        self.assertIn("LESSER GENERAL PUBLIC LICENSE", (LIC / "pystray-COPYING-LGPL-3.0.txt").read_text(encoding="utf-8"))
        self.assertIn("Bootloader Exception", (LIC / "PyInstaller-COPYING.txt").read_text(encoding="utf-8"))
        self.assertIn("Apache License", (LIC / "Apache-2.0.txt").read_text(encoding="utf-8"))

    def test_requirements_documented(self):
        doc = (ROOT / "THIRD_PARTY.md").read_text(encoding="utf-8").lower()
        for line in (ROOT / "requirements.txt").read_text().split():
            self.assertIn(re.split(r"[<>=]", line)[0].lower(), doc)

    def test_build_collects_licenses(self):
        build = (ROOT / "build.ps1").read_text(encoding="utf-8")
        self.assertIn("collect_licenses.py", build)
        fetch = (ROOT / "fetch_windivert.ps1").read_text(encoding="utf-8")
        self.assertIn('\\LICENSE"', fetch)
        wf = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("GDPI-third-party-sources.zip", wf)

    def test_windivert_zip_hash_format(self):
        h = re.search(r'\$zipSha256 = "([0-9A-F]{64})"', (ROOT / "fetch_windivert.ps1").read_text(encoding="utf-8"))
        self.assertIsNotNone(h)


if __name__ == "__main__":
    unittest.main()
