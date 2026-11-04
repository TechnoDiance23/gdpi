"""Fark testi derlemi: gdpi.py'deki Python fonksiyonlarıyla beklenen çıktıları üretir.
Rust çekirdeği (tests/differential.rs) aynı girdilerde birebir aynı sonucu vermeli.

    python3 rust/gdpi-core/tests/gen_corpus.py   ->  rust/gdpi-core/tests/corpus.json
"""
import json
import random
import socket
import ssl
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SRC = (ROOT / "gdpi.py").read_text(encoding="utf-8")
CORE = SRC.split("# ----------------------------------------------------------------- arayüz")[0]
g = {"__name__": "gdpi_core"}
exec(compile(CORE, "gdpi.py", "exec"), g)
g["random"].getrandbits = lambda n: 0          # sahte paketin içeriği sabit: sıfırlar

rng = random.Random(4744)


def client_hello(host, alpn=None):
    ctx = ssl.create_default_context()
    if alpn:
        ctx.set_alpn_protocols(alpn)
    i, o = ssl.MemoryBIO(), ssl.MemoryBIO()
    s = ctx.wrap_bio(i, o, server_hostname=host)
    try:
        s.do_handshake()
    except ssl.SSLWantReadError:
        pass
    return o.read()


def hand_hello(host: bytes, session=32, ciphers=4, ext_before=b""):
    sni = struct.pack("!HHHBH", 0, len(host) + 5, len(host) + 3, 0, len(host)) + host
    exts = ext_before + sni
    body = b"\x03\x03" + bytes(32) + bytes([session]) + bytes(session) + struct.pack("!H", ciphers) + bytes(ciphers) \
        + b"\x01\x00" + struct.pack("!H", len(exts)) + exts
    hs = b"\x01" + len(body).to_bytes(3, "big") + body
    return b"\x16\x03\x01" + struct.pack("!H", len(hs)) + hs


def mutate(b: bytes, n=3):
    b = bytearray(b)
    for _ in range(n):
        op = rng.randrange(3)
        if op == 0 and b:
            b[rng.randrange(len(b))] = rng.randrange(256)
        elif op == 1 and b:
            k = rng.randrange(len(b))
            del b[k:k + rng.randrange(1, 8)]
        else:
            k = rng.randrange(len(b) + 1)
            b[k:k] = bytes(rng.randrange(256) for _ in range(rng.randrange(1, 4)))
    return bytes(b)


# ---------------------------------------------------------------- yükler
hosts = ["discord.com", "cdn.discordapp.com", "www.isbank.com.tr", "www.turkiye.gov.tr", "grok.com", "x.com",
         "a.b.c.d.e.example.org", "x" * 63 + ".com", "ŞÇ.com".encode("idna").decode(), "mcp.cloudflare.com"]
tls = [client_hello(h) for h in hosts] + [client_hello("discord.com", ["h2", "http/1.1"])]
tls += [hand_hello(b"discord.com"), hand_hello(b"weird\x80\xffhost.com"), hand_hello(b""), hand_hello(b"a", 0, 2),
        hand_hello(b"pad.com", ext_before=struct.pack("!HH", 21, 10) + bytes(10)), hand_hello(b"DISCORD.COM.")]
no_sni = ssl.create_default_context()
no_sni.check_hostname = False
i, o = ssl.MemoryBIO(), ssl.MemoryBIO()
try:
    no_sni.wrap_bio(i, o).do_handshake()
except ssl.SSLWantReadError:
    pass
tls.append(o.read())

http = [b"GET / HTTP/1.1\r\nHost: discord.com\r\nUser-Agent: x\r\n\r\n",
        b"POST /a HTTP/1.1\r\nHost:discord.gg\r\n\r\n", b"HEAD / HTTP/1.0\r\nHost:    roblox.com",
        b"PUT / HTTP/1.1\r\nHost: \r\n\r\n", b"OPTIONS * HTTP/1.1\r\nhost: lower.com\r\n\r\n",
        b"GET / HTTP/1.1\r\nX: 1\r\nHost: www.isbank.com.tr\r\n\r\n", b"BREW / HTTP/1.1\r\nHost: tea.com\r\n\r\n",
        b"GET / HTTP/1.1\r\nHost: \xc3\x87ay.com\r\n\r\n", b"GET /", b"GET / HTTP/1.1\r\nHost:", b"",
        b"GET / HTTP/1.1\r\nHost: x.com.\r\n\r\n", b"GET / HTTP/1.1\r\nHost: discord.com"]

payloads = list(tls) + list(http)
for t in tls[:4] + [tls[11]]:
    payloads += [t[:k] for k in range(0, len(t), 7)]
for base in tls + http:
    payloads += [mutate(base, rng.randrange(1, 6)) for _ in range(25)]
for _ in range(300):
    n = rng.randrange(0, 300)
    b = bytes(rng.randrange(256) for _ in range(n))
    if rng.random() < 0.5 and n > 6:
        b = b"\x16" + b[1:5] + b"\x01" + b[6:]
    payloads.append(b)

parse = []
for p in payloads:
    s, h = g["find_sni"](p), g["find_http_host"](p)
    parse.append({"p": p.hex(), "sni": list(s) if s else None, "http": list(h) if h else None})

# ---------------------------------------------------------------- domain_in
dom_lists = [["gov.tr", "x.com"], g["DEFAULTS"]["bypass"], g["DEFAULTS"]["targets"], ["a"], []]
dom_hosts = ["gov.tr", "www.turkiye.gov.tr", "evilgov.tr", "X.COM.", "x.com..", "api.x.com", "xx.com", "a", "b.a",
             ".a", "", "discord.com", "cdn.discordapp.com", "grok.com", "accounts.x.ai", "x.ai.evil.com",
             "googleapis.com", "www.googleapis.com", "isbank.com.tr", "İSBANK.com.tr"]
domains = [{"h": h, "d": d, "r": g["domain_in"](h, d)} for h in dom_hosts for d in dom_lists]

# ---------------------------------------------------------------- sanitize_cfg
D = g["DEFAULTS"]
cfg_in = [None, [], "x", 5, {}, dict(D),
          {"mode": "liste"}, {"mode": "LISTE"}, {"mode": 1}, {"fake": "ttl"}, {"fake": "kapali"}, {"fake": "yok"},
          {"ttl": 1}, {"ttl": 32}, {"ttl": 0}, {"ttl": 33}, {"ttl": -5}, {"ttl": 4.0}, {"ttl": "4"}, {"ttl": True},
          {"ttl": False}, {"ttl": 10 ** 30}, {"reverse": True}, {"reverse": 1}, {"block_quic": False},
          {"dns_redirect": True}, {"dns_redirect": "true"}, {"dns_addr": "1.1.1.1"}, {"dns_addr": "6.6.6.6"},
          {"dns_addr": 7}, {"dns_port": 53}, {"dns_port": 5353}, {"dns_port": 54}, {"dns_port": True},
          {"dns_port": 53.0}, {"targets": ["a.com", "b-c.d"]}, {"targets": ["A.com"]}, {"targets": "a.com"},
          {"targets": []}, {"bypass": []}, {"bypass": ["ok.com", 5]}, {"bypass": ["a..b"]}, {"bypass": ["-.-"]},
          {"bypass": ["a.com\n"]}, {"bypass": ["a.com\n\n"]}, {"bypass": ["a" * 253]}, {"bypass": ["a" * 254]},
          {"bypass": ["ş.com"]}, {"bypass": [" a.com"]}, {"bypass": ["a.com."]}, {"bypass": ["xn--a.com", "1.2.3.4"]},
          {"targets": ["a.com"], "bypass": ["b.com"], "mode": "liste", "fake": "ttl", "ttl": 7, "extra": 1}]
configs = []
for c in cfg_in:
    out = json.loads(json.dumps(g["sanitize_cfg"](c)))
    if isinstance(out["ttl"], bool):                 # Python True'yu sayı gibi kullanır (int(True) == 1)
        out["ttl"] = int(out["ttl"])
    configs.append({"in": c, "out": out})

# ---------------------------------------------------------------- filtre / nft kuralları
rule_cfgs = [{}, {"block_quic": False}, {"dns_redirect": True}, {"dns_redirect": True, "dns_addr": "1.1.1.1",
             "dns_port": 53, "block_quic": False}]
rules = [{"cfg": c, "filter": g["Engine"](g["sanitize_cfg"](c), None).filter_str(),
          "nft": g["nft_rules"](g["sanitize_cfg"](c))} for c in rule_cfgs]

# ---------------------------------------------------------------- motor kararları
class FakeWD:
    def __init__(self):
        self.sent = []

    def outbound(self, addr):
        return addr

    def checksums(self, pkt, addr):
        g["fix_checksums"](pkt)

    def send(self, pkt, addr, fix_checksums=True):
        if fix_checksums:
            self.checksums(pkt, addr)
        self.sent.append([bytes(pkt).hex(), fix_checksums])


def ip4(proto, l4, src="10.0.0.2", dst="162.159.135.232", ttl=64):
    return struct.pack("!BBHHHBBH4s4s", 0x45, 0, 20 + len(l4), 7, 0x4000, ttl, proto, 0,
                       socket.inet_aton(src), socket.inet_aton(dst)) + l4


def ip6(proto, l4):
    return struct.pack("!IHBB", 6 << 28, len(l4), proto, 64) + socket.inet_pton(socket.AF_INET6, "fd00::2") \
        + socket.inet_pton(socket.AF_INET6, "2606:4700::6810:1") + l4


def tcp(dport, payload, seq=0xFFFFFF00, opts=b""):
    off = (20 + len(opts)) // 4
    return struct.pack("!HHIIBBHHH", 51000, dport, seq, 77, off << 4, 0x18, 64240, 0, 0) + opts + payload


def udp(sport, dport, payload=b"\x12\x34\x01\x00"):
    return struct.pack("!HHHH", sport, dport, 8 + len(payload), 0) + payload


eng_cfgs = [{}, {"fake": "checksum"}, {"fake": "ttl", "ttl": 7}, {"fake": "kapali"}, {"split": "orta"}, {"split": "1"},
            {"split": "2", "fake_seq": True}, {"fake_seq": True, "fake": "checksum"}, {"reverse": True}, {"fake": "ttl", "reverse": True},
            {"mode": "liste"}, {"mode": "liste", "targets": ["discord.com"], "fake": "kapali", "reverse": True},
            {"bypass": []}, {"dns_redirect": True}, {"dns_redirect": True, "dns_addr": "1.1.1.1", "dns_port": 53}]

pkts = []
for p in tls[:14] + [tls[-1]]:
    pkts.append([ip4(6, tcp(443, p)), True])
    pkts.append([ip6(6, tcp(443, p)), True])
for p in http:
    pkts.append([ip4(6, tcp(80, p)), True])
pkts.append([ip4(6, tcp(443, tls[0], opts=b"\x01\x01\x08\x0a" + bytes(8))), True])
pkts.append([ip4(6, tcp(443, tls[0], seq=5)), True])
pkts.append([ip4(6, tcp(443, b"")), True])
pkts.append([ip4(6, tcp(80, http[0][:20])), True])
pkts.append([ip4(6, tcp(443, mutate(tls[0]))), True])
# UDP: QUIC, DNS giden/gelen sırası (durumlu), IPv6 DNS
dns_seq = [[ip4(17, udp(40000, 443)), True], [ip6(17, udp(40000, 443)), True],
           [ip4(17, udp(40001, 53), dst="192.168.1.1"), True],
           [ip4(17, udp(1253, 40001), src="77.88.8.8", dst="10.0.0.2"), False],
           [ip4(17, udp(1253, 40001), src="77.88.8.8", dst="10.0.0.2"), False],    # eşlemesi yok artık
           [ip4(17, udp(53, 40002), src="1.1.1.1", dst="10.0.0.2"), False],
           [ip4(17, udp(40003, 53), dst="8.8.8.8"), True], [ip4(17, udp(53, 40003), src="1.1.1.1"), False],
           [ip6(17, udp(40004, 53)), True], [ip4(17, udp(40005, 5000)), True], [ip4(17, udp(443, 5000)), False]]

engine = []
for c in eng_cfgs:
    for seqname, seq in (("tcp", pkts), ("udp", dns_seq)):
        logs = []
        ecfg = g["sanitize_cfg"](c)
        ecfg.update({k: c[k] for k in ("split", "fake_seq") if k in c})     # yalnız testlerin kullandığı anahtarlar
        e = g["Engine"](ecfg, logs.append)
        wd = FakeWD()
        steps = []
        for raw, out in seq:
            wd.sent = []
            p = g["Pkt"](bytearray(raw))
            done = (e.handle_udp(wd, p, out) if p.udp else e.handle_tcp(wd, p, out))
            if not done:
                wd.send(p.raw, out, fix_checksums=False)
            steps.append({"raw": raw.hex(), "out": out, "sent": wd.sent})
        engine.append({"cfg": c, "steps": steps, "split": e.n_split, "skip": e.n_skip, "logs": logs})

# ---------------------------------------------------------------- load_cfg göçü (cfg_version)
import tempfile
migrate = []
with tempfile.TemporaryDirectory() as td:
    g["CONFIG"] = str(Path(td) / "ayarlar.json")
    for text in ['{}', '{"fake": "checksum"}', '{"fake": "checksum", "ttl": 8}', '{"fake": "checksum", "cfg_version": 2}',
                 '{"fake": "kapali"}', '{"fake": "ttl", "ttl": 9}', '[]', 'bozuk', '{"cfg_version": 1, "fake": "checksum"}',
                 '{"cfg_version": 3, "fake": "checksum", "ttl": 5}', '{"cfg_version": true, "fake": "checksum"}']:
        Path(g["CONFIG"]).write_text(text, encoding="utf-8")
        migrate.append({"in": text, "out": json.loads(json.dumps(g["load_cfg"]()))})

corpus = {"migrate": migrate, "parse": parse, "domains": domains, "configs": configs, "rules": rules, "engine": engine,
          "python": sys.version.split()[0], "openssl": ssl.OPENSSL_VERSION}
out = Path(__file__).with_name("corpus.json")
out.write_text(json.dumps(corpus, ensure_ascii=False, indent=0) + "\n", encoding="utf-8")
print(f"{out}: {len(parse)} yük, {len(domains)} alan adı, {len(configs)} ayar, {len(engine)} motor senaryosu")
