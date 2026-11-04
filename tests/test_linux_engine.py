"""Linux motoru (NFQUEUE) birim testleri; arayüz yüklenmeden çalışır.
GDPI_E2E=1 ve root ile gerçek ağ üzerinde uçtan uca test de yapılır (CI: Ubuntu)."""
import os
import socket
import struct
import subprocess
import sys
import unittest
from pathlib import Path

SRC = (Path(__file__).resolve().parent.parent / "gdpi.py").read_text(encoding="utf-8")
CORE = SRC.split("# ----------------------------------------------------------------- arayüz")[0]
g = {"__name__": "gdpi_core"}
exec(compile(CORE, "gdpi.py", "exec"), g)


def ref_checksum(data):
    if len(data) % 2:
        data += b"\0"
    s = sum(struct.unpack("!%dH" % (len(data) // 2), data))
    while s >> 16:
        s = (s & 0xFFFF) + (s >> 16)
    return ~s & 0xFFFF


def tcp4(payload=b"hello", src="10.0.0.1", dst="93.184.216.34"):
    tcp = struct.pack("!HHIIBBHHH", 40000, 443, 1000, 0, 5 << 4, 0x18, 65535, 0, 0) + payload
    ip = struct.pack("!BBHHHBBH4s4s", 0x45, 0, 20 + len(tcp), 1, 0, 64, 6, 0,
                     socket.inet_aton(src), socket.inet_aton(dst))
    return bytearray(ip + tcp)


class Checksums(unittest.TestCase):
    def test_matches_reference(self):
        for n in (0, 1, 5, 517, 1400):
            self.assertEqual(g["inet_checksum"]((bytes(range(256)) * 6)[:n]),
                             ref_checksum((bytes(range(256)) * 6)[:n]))

    def test_ipv4_tcp_valid(self):
        r = tcp4(b"x" * 33)
        g["fix_checksums"](r)
        self.assertEqual(ref_checksum(bytes(r[:20])), 0)
        pseudo = bytes(r[12:20]) + b"\0\x06" + (len(r) - 20).to_bytes(2, "big")
        self.assertEqual(ref_checksum(pseudo + bytes(r[20:])), 0)

    def test_ipv6_tcp_valid(self):
        tcp = bytearray(tcp4(b"abc")[20:])
        src, dst = socket.inet_pton(socket.AF_INET6, "fd00::1"), socket.inet_pton(socket.AF_INET6, "2606::2")
        r = bytearray(struct.pack("!IHBB", 6 << 28, len(tcp), 6, 64) + src + dst) + tcp
        g["fix_checksums"](r)
        pseudo = src + dst + len(tcp).to_bytes(4, "big") + b"\0\0\0\x06"
        self.assertEqual(ref_checksum(pseudo + bytes(r[40:])), 0)


class Rules(unittest.TestCase):
    def test_rules_content(self):
        cfg = g["sanitize_cfg"]({"block_quic": True, "dns_redirect": True})
        r = g["nft_rules"](cfg)
        self.assertIn("queue num 4744 bypass", r)
        self.assertIn("udp dport 443 drop", r)
        self.assertIn("dnat ip to 77.88.8.8:1253", r)
        self.assertIn(f"meta mark {g['NFQ_MARK']:#x} return", r)
        self.assertNotIn("drop", g["nft_rules"](g["sanitize_cfg"]({"block_quic": False})))


class FakeNL:
    def __init__(self):
        self.sent = []

    def send(self, b):
        self.sent.append(b)


class Verdicts(unittest.TestCase):
    def make(self):
        q = g["NFQueue"].__new__(g["NFQueue"])
        q.nl, q.seq, q.pending, q.closed = FakeNL(), 0, None, False
        q.raw_sent = []
        q.raw4 = q.raw6 = type("R", (), {"sendto": lambda s, p, a: q.raw_sent.append((bytes(p), a))})()
        return q

    @staticmethod
    def verdict(msg):
        return struct.unpack("!II", msg[24:32])

    def test_untouched_packet_accepted(self):
        q = self.make()
        raw = tcp4()
        q.pending = 7
        q.send(raw, {"id": 7, "orig": raw}, fix_checksums=False)
        self.assertEqual(self.verdict(q.nl.sent[0]), (1, 7))
        self.assertEqual(q.raw_sent, [])

    def test_split_drops_original_and_injects(self):
        q = self.make()
        raw = tcp4(b"A" * 100)
        addr = {"id": 9, "orig": raw}
        q.pending = 9
        e = g["Engine"](g["sanitize_cfg"]({"fake": "checksum"}), lambda m: None)
        p = g["Pkt"](raw)
        e.send_split(q, p, addr, p.payload, [2, 50], (40, 20))     # (site adının yeri, uzunluğu)
        self.assertEqual(self.verdict(q.nl.sent[0]), (0, 9))       # aslı düşürüldü
        self.assertEqual(len(q.raw_sent), 4)                       # sahte + 3 parça
        self.assertEqual(q.raw_sent[0][1], ("93.184.216.34", 0))
        fake, part = q.raw_sent[0][0], q.raw_sent[1][0]
        pseudo = lambda b: b[12:20] + b"\0\x06" + (len(b) - 20).to_bytes(2, "big")
        self.assertNotEqual(ref_checksum(pseudo(fake) + fake[20:]), 0)   # sahte paket bozuk
        self.assertEqual(ref_checksum(pseudo(part) + part[20:]), 0)      # gerçek parçalar sağlam
        self.assertEqual(b"".join(x[0][40:] for x in q.raw_sent[1:]), b"A" * 100)


@unittest.skipUnless(os.environ.get("GDPI_E2E") and hasattr(os, "geteuid") and os.geteuid() == 0,
                     "GDPI_E2E=1 ve root gerekir")
class EndToEnd(unittest.TestCase):
    def test_real_traffic(self):
        hosts = ("www.google.com", "github.com", "example.com", "www.cloudflare.com", "pypi.org")
        base = {h: g["site_check"](h) for h in hosts}
        print("\nkapalı:", base)
        fails = {}
        for fake in ("kapali", "checksum", "ttl"):
            logs = []
            e = g["Engine"](g["sanitize_cfg"]({"mode": "hepsi", "fake": fake}), logs.append)
            e.start()
            try:
                on = {h: g["site_check"](h) for h in hosts}
                print(f"açık ({fake}):", on, e.n_split)
                fails[fake] = [h for h in hosts if base[h][0] and not on[h][0]]
                if fake == "kapali":
                    self.assertGreater(e.n_split, 0, logs)
                    code = subprocess.run(["curl", "-s", "-o", "/dev/null", "-w", "%{http_code}",
                                           "http://example.com/"], capture_output=True, text=True, timeout=20).stdout
                    print("http:", code)
            finally:
                e.stop()
        # "checksum" modu bilerek bozuk paket gönderir; bazı bulut ağları (ör. GitHub Actions/Azure) sağlama
        # toplamını yolda düzelttiği için Cloudflare sitelerinde başarısız olur. Aynı sonuç Windows/WinDivert
        # öz testinde de görülür, bu yüzden bu mod burada yalnızca raporlanır.
        print("başarısız:", fails)
        self.assertEqual(fails["kapali"], [])
        self.assertEqual(fails["ttl"], [])
        tables = subprocess.run(["nft", "list", "tables"], capture_output=True, text=True).stdout
        self.assertNotIn("gdpi", tables)


if __name__ == "__main__":
    unittest.main()
