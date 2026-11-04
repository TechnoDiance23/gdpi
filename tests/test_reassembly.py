# İki pakete bölünmüş ClientHello (Chrome/Brave/Discord ~1,8 KB): site adı ikinci pakete düşse de
# GDPI onu bulmalı, "asla dokunma" listesine uymalı ve hiçbir paketi ilk parçadan büyük göndermemeli.
import os
import sys
import threading
import time
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import gdpi  # noqa: E402


def client_hello(host, pad_before_sni=1600):
    """Site adı uzun bir uzantıdan (Chrome'daki ML-KEM anahtarı gibi) sonra gelen ClientHello."""
    h = host.encode()
    sni = (b"\x00\x00" + (len(h) + 5).to_bytes(2, "big") + (len(h) + 3).to_bytes(2, "big")
           + b"\x00" + len(h).to_bytes(2, "big") + h)
    pad = b"\x00\x15" + pad_before_sni.to_bytes(2, "big") + b"\x00" * pad_before_sni
    exts = pad + sni
    body = (b"\x03\x03" + b"\x11" * 32 + b"\x00" + b"\x00\x02\x13\x01" + b"\x01\x00"
            + len(exts).to_bytes(2, "big") + exts)
    hs = b"\x01" + len(body).to_bytes(3, "big") + body
    return b"\x16\x03\x01" + len(hs).to_bytes(2, "big") + hs


def tcp4(payload, seq):
    ip = bytearray(b"\x45\x00\x00\x00\x00\x00\x40\x00\x40\x06\x00\x00" + bytes([10, 0, 0, 2, 162, 159, 128, 233]))
    tcp = bytearray((50123).to_bytes(2, "big") + (443).to_bytes(2, "big") + seq.to_bytes(4, "big")
                    + b"\x00" * 4 + b"\x50\x18\xff\xff\x00\x00\x00\x00")
    raw = ip + tcp + payload
    raw[2:4] = len(raw).to_bytes(2, "big")
    return raw


class FakeWD:
    def __init__(self):
        self.sent = []

    def checksums(self, pkt, addr):
        pass

    def send(self, pkt, addr, fix_checksums=True):
        self.sent.append(bytes(pkt))


def engine(**cfg):
    return gdpi.Engine(gdpi.sanitize_cfg({"mode": "hepsi", "fake": "ttl", "ttl": 3, **cfg}), lambda m: None)


def feed(e, wd, data, mss=1452, seq=1000):
    first, second = data[:mss], data[mss:]
    r1 = e.handle_tcp(wd, gdpi.Pkt(tcp4(first, seq)), {})
    r2 = e.handle_tcp(wd, gdpi.Pkt(tcp4(second, seq + len(first))), {})
    return r1, r2


def real_stream(sent, base_seq):
    """TTL'i 64 olan (gerçek) parçaları sıra numarasına göre birleştir."""
    parts = []
    for raw in sent:
        p = gdpi.Pkt(bytearray(raw))
        if raw[8] == 64:
            parts.append((p.seq - base_seq, p.payload))
    out = b""
    for off, chunk in sorted(parts):
        assert off == len(out), (off, len(out))
        out += chunk
    return out


class Reassembly(unittest.TestCase):
    def test_sni_in_second_packet_is_found_and_split(self):
        data = client_hello("discord.com")
        self.assertGreater(len(data), 1452)
        self.assertIsNone(gdpi.find_sni(data[:1452]))          # gerçekten ikinci pakette
        e, wd = engine(), FakeWD()
        self.assertEqual(feed(e, wd, data), (True, True))
        self.assertEqual(e.n_split, 1)
        self.assertTrue(all(len(gdpi.Pkt(bytearray(r)).payload) <= 1452 for r in wd.sent))
        fakes = [r for r in wd.sent if r[8] == 3]
        self.assertTrue(fakes)                                 # TTL 3'lü sahte paket gitti
        fake_data = b"".join(gdpi.Pkt(bytearray(r)).payload for r in fakes)
        self.assertNotIn(b"discord.com", fake_data)
        self.assertEqual(real_stream(wd.sent, 1000), data)      # gerçek veri eksiksiz ve sırayla
        loc = gdpi.find_sni(data)
        mid = loc[0] + loc[1] // 2
        offsets = {gdpi.Pkt(bytearray(r)).seq - 1000 for r in wd.sent if r[8] == 64}
        self.assertIn(mid, offsets)                            # site adının ortasından bölündü

    def test_bypass_host_is_released_untouched(self):
        data = client_hello("www.isbank.com.tr")
        e, wd = engine(), FakeWD()
        feed(e, wd, data)
        self.assertEqual(e.n_split, 0)
        self.assertEqual(e.n_skip, 1)
        self.assertEqual(len(wd.sent), 2)                      # iki asıl paket, olduğu gibi
        self.assertEqual(real_stream(wd.sent, 1000), data)
        self.assertFalse(any(r[8] == 3 for r in wd.sent))

    def test_sni_in_first_packet_still_works(self):
        data = client_hello("discord.com", pad_before_sni=10)
        e, wd = engine(), FakeWD()
        self.assertTrue(e.handle_tcp(wd, gdpi.Pkt(tcp4(data, 1000)), {}))
        self.assertEqual(e.n_split, 1)
        self.assertEqual(e.held, {})

    def test_missing_second_packet_is_flushed(self):
        data = client_hello("discord.com")
        e, wd = engine(), FakeWD()
        e.HOLD_SECONDS = 0.05
        self.assertTrue(e.handle_tcp(wd, gdpi.Pkt(tcp4(data[:1452], 1000)), {}))
        self.assertEqual(wd.sent, [])
        e.wd = wd
        threading.Thread(target=e.flush_loop, args=(wd,), daemon=True).start()
        time.sleep(0.4)
        e.wd = None
        self.assertEqual(len(wd.sent), 1)                      # bekletilen paket kaybolmadı
        self.assertEqual(gdpi.Pkt(bytearray(wd.sent[0])).payload, data[:1452])

    def test_out_of_order_releases(self):
        data = client_hello("discord.com")
        e, wd = engine(), FakeWD()
        e.handle_tcp(wd, gdpi.Pkt(tcp4(data[:1452], 1000)), {})
        self.assertFalse(e.handle_tcp(wd, gdpi.Pkt(tcp4(b"x" * 10, 99999)), {}))
        self.assertEqual(len(wd.sent), 1)                      # bekletilen geri verildi
        self.assertEqual(e.held, {})

    def test_chunks_respect_mss(self):
        d = bytes(range(256)) * 8
        parts = gdpi.Engine.chunks(d, [2, 1000], 700)
        self.assertEqual(b"".join(c for _, c in parts), d)
        self.assertTrue(all(len(c) <= 700 for _, c in parts))
        self.assertIn(2, [o for o, _ in parts])
        self.assertIn(1000, [o for o, _ in parts])


if __name__ == "__main__":
    unittest.main()
