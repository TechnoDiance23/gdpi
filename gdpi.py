"""
GDPI - Görüşürüz Derin Paket İnceleme
Kişisel, arayüzlü DPI aşma aracı (WinDivert 2.2).

GoodbyeDPI'dan farkı:
  * Siteye göre çalışır: bankalar, e-Devlet, Resmî Gazete, Grok gibi siteler "dokunma" listesindedir,
    paketlerine hiç müdahale edilmez.
  * Sahte paket varsayılan olarak "bozuk checksum" ile gider. Sunucu ne kadar yakın olursa olsun
    (Türkiye'deki siteler 3-5 hop ötede) sahte paketi kabul etmez. Düşük TTL'de yakın sunucular
    sahte paketi alıp bağlantıyı bozuyordu.
  * İsteğe bağlı DNS yönlendirme (operatörün DNS zorlamasını aşar).

Kaynaktan: pip install -r requirements.txt  ->  GDPI.bat     Hazır: Releases'tan GDPI.exe
"""
import ctypes
import json
import math
import os
import queue
import random
import subprocess
import sys
import threading
import time

APP = "GDPI - Görüşürüz Derin Paket İnceleme"
VERSION = "1.2.1"
FROZEN = getattr(sys, "frozen", False)          # PyInstaller ile derlenmiş GDPI.exe
WINDOWS = sys.platform == "win32"
if WINDOWS:
    DATA_DIR = os.path.join(os.environ.get("APPDATA", os.path.expanduser("~")), "GDPI")
else:                                           # Linux: ~/.config/GDPI (pkexec ile HOME korunur)
    DATA_DIR = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"), "GDPI")
os.makedirs(DATA_DIR, exist_ok=True)
CONFIG = os.path.join(DATA_DIR, "ayarlar.json")
if sys.stderr is None or sys.stdout is None:    # pencereli exe: konsol yok, çıktıyı dosyaya yönlendir
    sys.stdout = sys.stderr = open(os.path.join(DATA_DIR, "cikti.log"), "w", encoding="utf-8", buffering=1)

DEFAULTS = {
    "mode": "hepsi",            # "hepsi": dokunma listesi hariç hepsi | "liste": sadece hedef listesi
    "fake": "checksum",         # "checksum" | "ttl" | "kapali"
    "ttl": 4,
    "reverse": False,
    "block_quic": True,
    "dns_redirect": False,
    "dns_addr": "77.88.8.8",
    "dns_port": 1253,
    "targets": ["discord.com", "discord.gg", "discordapp.com", "discordapp.net",
                "discord.media", "roblox.com", "wattpad.com", "pastebin.com"],
    "bypass": ["isbank.com.tr", "gov.tr", "turkiye.gov.tr", "resmigazete.gov.tr",
               # X + Grok (tarayıcı ve PC'deki Grok Bot uygulaması)
               "x.ai", "grok.com", "x.com", "twimg.com", "statsig.com", "statsigapi.net",
               "sentry.io", "mcp.cloudflare.com", "challenges.cloudflare.com", "googleapis.com",
               "garantibbva.com.tr", "akbank.com", "yapikredi.com.tr", "ziraatbank.com.tr",
               "vakifbank.com.tr", "halkbank.com.tr", "qnb.com.tr", "denizbank.com",
               "enpara.com", "papara.com", "epicgames.com"],
}


# ----------------------------------------------------------------- paket ayrıştırma
def find_sni(payload: bytes):
    """TLS ClientHello içindeki SNI'nin (başlangıç, uzunluk) değeri; yoksa None."""
    try:
        if payload[0] != 0x16 or payload[5] != 0x01:
            return None
        i = 43                                   # kayıt(5)+el sıkışma(4)+sürüm(2)+random(32)
        i += 1 + payload[i]                      # session id
        i += 2 + int.from_bytes(payload[i:i + 2], "big")
        i += 1 + payload[i]
        end = i + 2 + int.from_bytes(payload[i:i + 2], "big")
        i += 2
        while i + 4 <= end:
            etype = int.from_bytes(payload[i:i + 2], "big")
            elen = int.from_bytes(payload[i + 2:i + 4], "big")
            if etype == 0:
                nlen = int.from_bytes(payload[i + 7:i + 9], "big")
                if i + 9 + nlen <= len(payload):
                    return i + 9, nlen
                return None
            i += 4 + elen
    except IndexError:
        pass
    return None


def find_http_host(payload: bytes):
    """HTTP isteğindeki Host değerinin (başlangıç, uzunluk) değeri; yoksa None."""
    if not payload.startswith((b"GET ", b"POST ", b"HEAD ", b"PUT ", b"OPTIONS ")):
        return None
    pos = payload.find(b"\r\nHost:")
    if pos == -1:
        return None
    s = pos + 7
    while s < len(payload) and payload[s] == 0x20:
        s += 1
    e = payload.find(b"\r\n", s)
    return (s, (e if e != -1 else len(payload)) - s)


def domain_in(host: str, domains) -> bool:
    host = host.lower().rstrip(".")
    return any(host == d or host.endswith("." + d) for d in domains)


# ----------------------------------------------------------------- WinDivert 2.2
WD_ERRORS = {
    2: "WinDivert dosyaları eksik (WinDivert.dll / WinDivert64.sys).",
    5: "Yönetici izni yok. GDPI'ı yönetici olarak açın.",
    87: "Filtre geçersiz (iç hata).",
    577: "Sürücü imzası doğrulanamadı. Windows sürücüyü engelledi.",
    1058: "WinDivert sürücü servisi devre dışı.",
    1275: "Windows sürücüyü engelledi (Güvenlik/Bellek bütünlüğü ya da antivirüs).",
    1753: "Windows Base Filtering Engine (BFE) servisi kapalı.",
}


def windivert_dir():
    base = getattr(sys, "_MEIPASS", os.path.dirname(os.path.abspath(__file__)))
    return os.path.join(base, "windivert")


class WinDivert:
    """WinDivert 2.2 için küçük ctypes sarmalayıcı (GoodbyeDPI ile aynı sürücü sürümü)."""
    ADDR_SIZE = 80                               # sizeof(WINDIVERT_ADDRESS)

    def __init__(self, flt):
        from ctypes import wintypes
        dll = ctypes.WinDLL(os.path.join(windivert_dir(), "WinDivert.dll"), use_last_error=True)
        dll.WinDivertOpen.restype = wintypes.HANDLE
        dll.WinDivertOpen.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_int16, ctypes.c_uint64]
        for name in ("WinDivertRecv", "WinDivertSend"):
            getattr(dll, name).restype = wintypes.BOOL
            getattr(dll, name).argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_uint,
                                           ctypes.POINTER(ctypes.c_uint), ctypes.c_void_p]
        dll.WinDivertShutdown.argtypes = [wintypes.HANDLE, ctypes.c_int]
        dll.WinDivertClose.argtypes = [wintypes.HANDLE]
        dll.WinDivertHelperCalcChecksums.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_void_p,
                                                     ctypes.c_uint64]
        self.dll = dll
        h = dll.WinDivertOpen(flt.encode(), 0, 0, 0)          # NETWORK katmanı, öncelik 0
        if not h or h == wintypes.HANDLE(-1).value:
            err = ctypes.get_last_error()
            raise OSError(err, WD_ERRORS.get(err, f"WinDivert açılamadı (Windows hata kodu {err})."))
        self.h = h
        self.buf = ctypes.create_string_buffer(65535)

    def recv(self):
        """(paket, adres) döndürür; tutamaç kapandıysa None."""
        addr = ctypes.create_string_buffer(self.ADDR_SIZE)
        n = ctypes.c_uint(0)
        if not self.dll.WinDivertRecv(self.h, self.buf, len(self.buf), ctypes.byref(n), addr):
            return None
        return bytearray(self.buf.raw[:n.value]), addr

    @staticmethod
    def outbound(addr):
        return bool(int.from_bytes(addr.raw[8:12], "little") >> 17 & 1)

    def checksums(self, pkt: bytearray, addr):
        cbuf = (ctypes.c_char * len(pkt)).from_buffer(pkt)
        self.dll.WinDivertHelperCalcChecksums(cbuf, len(pkt), addr, 0)

    def send(self, pkt: bytearray, addr, fix_checksums=True):
        if fix_checksums:
            self.checksums(pkt, addr)
        cbuf = (ctypes.c_char * len(pkt)).from_buffer(pkt)
        self.dll.WinDivertSend(self.h, cbuf, len(pkt), None, addr)

    def close(self):
        self.dll.WinDivertShutdown(self.h, 3)                  # bekleyen recv'i uyandır
        self.dll.WinDivertClose(self.h)


# ----------------------------------------------------------------- Linux: NFQUEUE + nftables
NFQ_NUM = 4744                                  # kuyruk numarası
NFQ_MARK = 0x47445049                           # GDPI'ın kendi gönderdiği paketler: kuyruğa geri girmez
NFT_TABLE = "gdpi"


def nft(script):
    try:
        r = subprocess.run(["nft", "-f", "-"], input=script, capture_output=True, text=True)
    except FileNotFoundError:
        raise OSError(2, "nftables kurulu değil (ör. sudo apt install nftables).") from None
    if r.returncode:
        raise OSError(r.returncode, (r.stderr.strip() or "nft hatası")[:300])


def nft_rules(cfg):
    """Giden HTTP/HTTPS paketlerinin ilk birkaçını kuyruğa gönderir; QUIC'i düşürür; DNS'i yönlendirir.
    'bypass': GDPI kapanır ya da çökerse paketler kuyruğa takılmadan normal akar."""
    out = [f"meta mark {NFQ_MARK:#x} return", 'oifname "lo" return',
           f"tcp dport {{ 80, 443 }} ct original packets < 12 queue num {NFQ_NUM} bypass"]
    if cfg["block_quic"]:
        out.append("udp dport 443 drop")
    rules = (f"table inet {NFT_TABLE} {{\n chain out {{ type filter hook output priority 0; policy accept;\n  "
             + "\n  ".join(out) + "\n }\n")
    if cfg["dns_redirect"]:
        a, port = cfg["dns_addr"], int(cfg["dns_port"])
        rules += (" chain dns { type nat hook output priority -100; policy accept;\n"
                  f"  ip daddr != 127.0.0.0/8 udp dport 53 dnat ip to {a}:{port}\n }}\n")
    return rules + "}\n"


def nft_remove():
    try:
        subprocess.run(["nft", "delete", "table", "inet", NFT_TABLE], capture_output=True)
    except OSError:
        pass


def inet_checksum(data: bytes) -> int:
    if len(data) % 2:
        data += b"\0"
    s = sum(memoryview(data).cast("H"))
    while s >> 16:
        s = (s & 0xFFFF) + (s >> 16)
    s = ~s & 0xFFFF
    return int.from_bytes(s.to_bytes(2, sys.byteorder), "big")


def fix_checksums(r: bytearray):
    """IPv4/IPv6 + TCP/UDP sağlama toplamlarını yeniden hesaplar (WinDivertHelperCalcChecksums karşılığı)."""
    v6 = r[0] >> 4 == 6
    if v6:
        l4, proto, src, dst = 40, r[6], r[8:24], r[24:40]
    else:
        l4, proto, src, dst = (r[0] & 15) * 4, r[9], r[12:16], r[16:20]
        r[10:12] = b"\0\0"
        r[10:12] = inet_checksum(bytes(r[:l4])).to_bytes(2, "big")
    if proto not in (6, 17):
        return
    off = l4 + (16 if proto == 6 else 6)
    seg_len = len(r) - l4
    r[off:off + 2] = b"\0\0"
    pseudo = bytes(src) + bytes(dst) + (seg_len.to_bytes(4, "big") + b"\0\0\0" + bytes([proto]) if v6
                                        else b"\0" + bytes([proto]) + seg_len.to_bytes(2, "big"))
    c = inet_checksum(pseudo + bytes(r[l4:]))
    r[off:off + 2] = (c or 0xFFFF if proto == 17 else c).to_bytes(2, "big")


class NFQueue:
    """Linux NFQUEUE'yu ek kütüphane olmadan (doğrudan netlink) kullanan, WinDivert sınıfıyla
    aynı arabirime sahip sarmalayıcı. Yalnızca giden TCP 80/443 kuyruğa gelir; QUIC engeli ve DNS
    yönlendirme nftables kurallarıyla çekirdekte yapılır."""
    import struct as _st

    def __init__(self, cfg):
        import socket
        if os.geteuid() != 0:
            raise OSError(1, "Yönetici (root) izni gerekli: ./gdpi.sh ile açın.")
        nft_remove()
        try:
            subprocess.run(["modprobe", "-a", "nfnetlink_queue", "nft_queue"], capture_output=True)
        except OSError:
            pass
        rules = nft_rules(cfg)
        try:
            nft(rules)
        except OSError:                             # conntrack ifadesi yoksa: tüm 80/443 paketleri
            try:
                nft(rules.replace(" ct original packets < 12", ""))
            except OSError as e:
                if "queue" in str(e):
                    raise OSError(e.errno, "Çekirdekte NFQUEUE desteği yok (nft_queue / nfnetlink_queue "
                                           "modülleri yüklenemedi).") from None
                raise
        try:
            self.nl = socket.socket(socket.AF_NETLINK, socket.SOCK_RAW, 12)   # NETLINK_NETFILTER
            self.nl.bind((0, 0))
            self.nl.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4 << 20)
            self.nl.settimeout(0.5)
            self.seq = 0
            self._config(self._attr(1, self._st.pack("!BxH", 1, 0)))              # CMD_BIND
            self._config(self._attr(2, self._st.pack("!IB", 0xFFFF, 2)))          # COPY_PACKET
            self.raw4 = socket.socket(socket.AF_INET, socket.SOCK_RAW, socket.IPPROTO_RAW)
            self.raw6 = socket.socket(socket.AF_INET6, socket.SOCK_RAW, socket.IPPROTO_RAW)
            for s in (self.raw4, self.raw6):
                s.setsockopt(socket.SOL_SOCKET, 36, NFQ_MARK)                     # SO_MARK
        except Exception:
            nft_remove()
            raise
        self.closed = False
        self.pending = None                         # karar bekleyen paket kimliği

    # --- netlink yardımcıları
    def _attr(self, t, data):
        a = self._st.pack("=HH", 4 + len(data), t) + data
        return a + b"\0" * (-len(a) % 4)

    def _msg(self, mtype, body, flags=1):
        self.seq += 1
        nfgen = self._st.pack("!BBH", 0, 0, NFQ_NUM)
        return self._st.pack("=IHHII", 16 + 4 + len(body), (3 << 8) | mtype, flags, self.seq, 0) + nfgen + body

    def _config(self, attr):
        self.nl.send(self._msg(2, attr, flags=1 | 4))                              # REQUEST|ACK
        while True:
            data = self.nl.recv(65536)
            ln, t = self._st.unpack_from("=IH", data)
            if t == 2:                                                             # NLMSG_ERROR
                err = -self._st.unpack_from("=i", data, 16)[0]
                if err:
                    raise OSError(err, f"NFQUEUE açılamadı ({os.strerror(err)}). "
                                       "Çekirdekte nfnetlink_queue modülü olmalı.")
                return

    def _verdict(self, pid, verdict):
        try:
            self.nl.send(self._msg(1, self._attr(2, self._st.pack("!II", verdict, pid))))
        except OSError:
            pass

    # --- WinDivert ile aynı arabirim
    def recv(self):
        import socket
        if self.pending is not None:                # işlenip yerine yenisi gönderilmiş: aslını düşür
            self._verdict(self.pending, 0)
            self.pending = None
        while not self.closed:
            try:
                data = self.nl.recv(70000)
            except socket.timeout:
                continue
            except OSError:
                if self.closed:
                    break
                continue                                                           # ENOBUFS vb.
            pos = 0
            while pos + 16 <= len(data):
                ln, t = self._st.unpack_from("=IH", data, pos)
                if ln < 16:
                    break
                if t == (3 << 8):                                                  # NFQNL_MSG_PACKET
                    pid, payload, a = None, None, pos + 20
                    while a + 4 <= pos + ln:
                        alen, atype = self._st.unpack_from("=HH", data, a)
                        if alen < 4:
                            break
                        atype &= 0x3FFF
                        if atype == 1:
                            pid = self._st.unpack_from("!I", data, a + 4)[0]
                        elif atype == 10:
                            payload = bytearray(data[a + 4:a + alen])
                        a += (alen + 3) & ~3
                    if pid is not None and payload:
                        self.pending = pid
                        return payload, {"id": pid, "orig": payload}
                    if pid is not None:
                        self._verdict(pid, 1)
                pos += (ln + 3) & ~3
        return None

    @staticmethod
    def outbound(addr):
        return True

    def checksums(self, pkt: bytearray, addr):
        fix_checksums(pkt)

    def send(self, pkt: bytearray, addr, fix_checksums=True):
        if pkt is addr["orig"]:                     # dokunulmayan paket: olduğu gibi geçsin
            if self.pending == addr["id"]:
                self._verdict(addr["id"], 1)
                self.pending = None
            return
        if self.pending == addr["id"]:              # yerine yeni paketler gidiyor: aslını düşür
            self._verdict(addr["id"], 0)
            self.pending = None
        if fix_checksums:
            self.checksums(pkt, addr)
        try:
            if pkt[0] >> 4 == 6:
                import socket
                self.raw6.sendto(pkt, (socket.inet_ntop(socket.AF_INET6, bytes(pkt[24:40])), 0))
            else:
                self.raw4.sendto(pkt, (".".join(map(str, pkt[16:20])), 0))
        except OSError:
            pass

    def close(self):
        self.closed = True
        nft_remove()
        try:                                        # kuyruğu hemen bırak: yeniden başlatma beklemeden çalışsın
            self.nl.send(self._msg(2, self._attr(1, self._st.pack("!BxH", 2, 0))))   # CMD_UNBIND
        except OSError:
            pass
        for s in (self.nl, self.raw4, self.raw6):
            try:
                s.close()
            except OSError:
                pass



class Pkt:
    """IPv4/IPv6 + TCP/UDP başlıklarını doğrudan baytlar üzerinde okuyup yazar."""

    def __init__(self, raw: bytearray):
        self.raw = raw
        self.v6 = raw[0] >> 4 == 6
        self.l4 = 40 if self.v6 else (raw[0] & 15) * 4
        self.proto = raw[6] if self.v6 else raw[9]
        self.tcp = self.proto == 6
        self.udp = self.proto == 17
        p = self.l4
        self.sport = int.from_bytes(raw[p:p + 2], "big")
        self.dport = int.from_bytes(raw[p + 2:p + 4], "big")
        self.poff = p + ((raw[p + 12] >> 4) * 4 if self.tcp else 8)

    @property
    def payload(self):
        return bytes(self.raw[self.poff:])

    @property
    def seq(self):
        return int.from_bytes(self.raw[self.l4 + 4:self.l4 + 8], "big")

    def segment(self, data: bytes, seq: int, ttl=None) -> bytearray:
        """Aynı başlıklarla, yeni yük ve sıra numarasıyla bir TCP paketi üretir."""
        r = bytearray(self.raw[:self.poff]) + data
        if self.v6:
            r[4:6] = (len(r) - 40).to_bytes(2, "big")
        else:
            r[2:4] = len(r).to_bytes(2, "big")
        r[self.l4 + 4:self.l4 + 8] = (seq & 0xFFFFFFFF).to_bytes(4, "big")
        if ttl is not None:
            r[7 if self.v6 else 8] = ttl
        return r


# ----------------------------------------------------------------- motor
class Engine:
    def __init__(self, cfg, log):
        self.cfg = cfg
        self.log = log
        self.wd = None
        self.thread = None
        self.seen = set()
        self.dns_map = {}                        # yerel port -> asıl DNS sunucusu (4 bayt)
        self.n_split = 0                         # bölünen bağlantı
        self.n_skip = 0                          # dokunulmayan (listeden) bağlantı

    def should_touch(self, host):
        if domain_in(host, self.cfg["bypass"]):
            return False
        if self.cfg["mode"] == "liste":
            return domain_in(host, self.cfg["targets"])
        return True

    def filter_str(self):
        f = ("(outbound and !loopback and tcp.PayloadLength > 0 and "
             "(tcp.DstPort == 443 or tcp.DstPort == 80))")
        if self.cfg["block_quic"]:
            f += " or (outbound and !loopback and udp.DstPort == 443)"
        if self.cfg["dns_redirect"]:
            f += (f" or (outbound and ip and udp.DstPort == 53)"
                  f" or (inbound and ip and udp.SrcPort == {int(self.cfg['dns_port'])}"
                  f" and ip.SrcAddr == {self.cfg['dns_addr']})")
        return f

    def start(self):
        self.wd = WinDivert(self.filter_str()) if WINDOWS else NFQueue(self.cfg)
        self.thread = threading.Thread(target=self.loop, daemon=True)
        self.thread.start()

    def stop(self):
        wd, self.wd = self.wd, None
        if wd:
            try:
                wd.close()
            except Exception:
                pass

    def loop(self):
        wd = self.wd
        while self.wd is wd:
            got = wd.recv()
            if got is None:
                break                            # tutamaç kapatıldı
            raw, addr = got
            try:
                p = Pkt(raw)
                if p.udp:
                    if self.handle_udp(wd, p, addr):
                        continue
                elif p.tcp and self.handle_tcp(wd, p, addr):
                    continue
            except Exception as e:
                self.log(f"hata: {e}")
            wd.send(raw, addr, fix_checksums=False)

    # --- UDP: QUIC engeli + DNS yönlendirme (yalnız IPv4)
    def handle_udp(self, wd, p, addr):
        out = wd.outbound(addr)
        if out and p.dport == 443:
            return True                          # QUIC düşür -> tarayıcı TCP'ye döner
        r = p.raw
        dns_ip = bytes(int(x) for x in self.cfg["dns_addr"].split("."))
        if out and p.dport == 53 and not p.v6:
            self.dns_map[p.sport] = bytes(r[16:20])
            r[16:20] = dns_ip
            r[p.l4 + 2:p.l4 + 4] = int(self.cfg["dns_port"]).to_bytes(2, "big")
        elif not out and not p.v6 and bytes(r[12:16]) == dns_ip:
            orig = self.dns_map.pop(p.dport, None)
            if orig:
                r[12:16] = orig
                r[p.l4:p.l4 + 2] = (53).to_bytes(2, "big")
        wd.send(r, addr)
        return True

    # --- TCP: SNI / Host bölme
    def handle_tcp(self, wd, p, addr):
        payload = p.payload
        loc = find_sni(payload) if p.dport == 443 else find_http_host(payload)
        if not loc:
            return False
        start, ln = loc
        host = payload[start:start + ln].decode("ascii", "ignore")
        if not self.should_touch(host):
            self.n_skip += 1
            return False
        self.n_split += 1
        if host not in self.seen:
            self.seen.add(host)
            self.log(f"→ {host}")

        if p.dport == 80:
            hpos = payload.find(b"\r\nHost:")
            payload = payload[:hpos] + b"\r\nhoSt:" + payload[hpos + 7:]
            cuts = [hpos + 4, start + ln // 2]
        else:
            cuts = [2, start + ln // 2]          # kayıt başlığı + alan adının ortası
        self.send_split(wd, p, addr, payload, cuts)
        return True

    def send_split(self, wd, p, addr, data, cuts):
        cuts = sorted(c for c in set(cuts) if 0 < c < len(data))
        b = [0] + cuts + [len(data)]
        parts = [(b[k], data[b[k]:b[k + 1]]) for k in range(len(b) - 1)]
        fake = self.cfg["fake"]
        if fake != "kapali":
            junk = bytes(random.getrandbits(8) for _ in range(len(parts[0][1])))
            f = p.segment(junk, p.seq, ttl=int(self.cfg["ttl"]) if fake == "ttl" else None)
            wd.checksums(f, addr)
            if fake == "checksum":
                c = p.l4 + 16                    # TCP checksum alanı: bozuk -> sunucu kesin reddeder
                f[c:c + 2] = (int.from_bytes(f[c:c + 2], "big") ^ 0xFFFF).to_bytes(2, "big")
            wd.send(f, addr, fix_checksums=False)
        if self.cfg["reverse"]:
            parts.reverse()
        for off, chunk in parts:
            wd.send(p.segment(chunk, p.seq + off), addr)


# ----------------------------------------------------------------- ayarlar
DNS_SERVERS = {"77.88.8.8", "77.88.8.1", "1.1.1.1", "1.0.0.1", "8.8.8.8", "8.8.4.4", "9.9.9.9"}
DOMAIN_RE = __import__("re").compile(r"^[a-z0-9-]+(\.[a-z0-9-]+)*$")


def sanitize_cfg(raw):
    """Ayar dosyası yöneticisiz yazılabilir; yönetici olarak çalışan motor ona körü körüne güvenmesin."""
    cfg = json.loads(json.dumps(DEFAULTS))
    if not isinstance(raw, dict):
        return cfg
    pick = lambda k, ok: raw[k] if k in raw and ok(raw[k]) else cfg[k]
    cfg["mode"] = pick("mode", lambda v: v in ("hepsi", "liste"))
    cfg["fake"] = pick("fake", lambda v: v in ("checksum", "ttl", "kapali"))
    cfg["ttl"] = pick("ttl", lambda v: isinstance(v, int) and 1 <= v <= 32)
    for k in ("reverse", "block_quic", "dns_redirect"):
        cfg[k] = pick(k, lambda v: isinstance(v, bool))
    cfg["dns_addr"] = pick("dns_addr", lambda v: v in DNS_SERVERS)
    cfg["dns_port"] = pick("dns_port", lambda v: isinstance(v, int) and v in (53, 1253, 5353))
    for k in ("targets", "bypass"):
        cfg[k] = pick(k, lambda v: isinstance(v, list) and all(
            isinstance(d, str) and len(d) < 254 and DOMAIN_RE.match(d) for d in v))
    return cfg


def load_cfg():
    try:
        with open(CONFIG, encoding="utf-8") as f:
            return sanitize_cfg(json.load(f))
    except (OSError, ValueError):
        return sanitize_cfg({})


def save_cfg(cfg):
    with open(CONFIG, "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)


# ----------------------------------------------------------------- simge + otomatik başlatma
TASK = "GDPI"


def shield_image(on: bool, size=64):
    """VPN tarzı kalkan simgesi: açıkken yeşil + tik, kapalıyken gri + güç işareti."""
    from PIL import Image, ImageDraw
    S = 256
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    top, bot = ("#34d399", "#059669") if on else ("#64748b", "#334155")
    shield = [(128, 14), (226, 50), (222, 132), (196, 190), (128, 244), (60, 190), (34, 132), (30, 50)]
    # dikey degrade için satır satır boya, sonra kalkan maskesiyle kes
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    c1 = tuple(int(top[i:i + 2], 16) for i in (1, 3, 5))
    c2 = tuple(int(bot[i:i + 2], 16) for i in (1, 3, 5))
    for y in range(S):
        t = y / S
        gd.line([(0, y), (S, y)], fill=tuple(int(c1[k] + (c2[k] - c1[k]) * t) for k in range(3)) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).polygon(shield, fill=255)
    img.paste(grad, (0, 0), mask)
    if on:
        d.line([(84, 130), (116, 162), (176, 98)], fill="white", width=26, joint="curve")
    else:
        d.arc((78, 82, 178, 182), start=-60, end=240, fill="white", width=20)
        d.line([(128, 66), (128, 130)], fill="white", width=20)
    return img.resize((size, size), Image.LANCZOS)


def autostart_enabled():
    if not WINDOWS:
        return False
    return subprocess.run(["schtasks", "/query", "/tn", TASK], capture_output=True,
                          creationflags=0x08000000).returncode == 0


def set_autostart(on: bool):
    """Oturum açılınca yönetici olarak, UAC sormadan, tepside ve açık başlar.
    Görev yalnızca Program Files'a kopyalanan exe'yi çalıştırır: oraya sadece yöneticiler yazabilir,
    böylece kullanıcı klasöründeki dosyayı değiştiren biri yönetici yetkisi kazanamaz.
    Başarılıysa None, değilse açıklama döndürür."""
    if not WINDOWS:
        return None if not on else "Linux'ta otomatik başlatma yok; gdpi.sh'i oturum başlangıcına ekleyebilirsiniz."
    if not on:
        subprocess.run(["schtasks", "/delete", "/f", "/tn", TASK], capture_output=True, creationflags=0x08000000)
        return None
    if not FROZEN:
        return "Windows ile başlatma sadece GDPI.exe sürümünde var (Releases sayfası)."
    import shutil
    import winreg
    # Ortam değişkeni kullanıcı tarafından değiştirilebilir; yolu HKLM'den (yalnızca yöneticiler yazar) oku
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion") as k:
        pf = winreg.QueryValueEx(k, "ProgramFilesDir")[0]
    dest_dir = os.path.join(pf, "GDPI")
    dest = os.path.join(dest_dir, "GDPI.exe")
    src_dir = os.path.dirname(os.path.abspath(sys.executable))   # onedir: exe + _internal klasörü
    try:
        if src_dir.lower() != dest_dir.lower():
            # Sadece GDPI.exe ve _internal kopyalanır; klasördeki başka (araya sokulmuş) dosyalar taşınmaz.
            # Eski kurulum önce tamamen silinir ki önceden bırakılmış dosya kalmasın.
            if os.path.isdir(dest_dir):
                shutil.rmtree(dest_dir)
            os.makedirs(dest_dir)
            shutil.copy2(os.path.join(src_dir, "GDPI.exe"), dest)
            shutil.copytree(os.path.join(src_dir, "_internal"), os.path.join(dest_dir, "_internal"))
    except OSError as e:
        return f"Program Files'a kopyalanamadı: {e}"
    r = subprocess.run(["schtasks", "/create", "/f", "/tn", TASK, "/sc", "onlogon", "/rl", "highest",
                        "/tr", f'"{dest}" --tray'], capture_output=True, creationflags=0x08000000)
    return None if r.returncode == 0 else "Zamanlanmış görev oluşturulamadı."


PRESETS = {
    "Hafif":   {"mode": "liste", "fake": "kapali",   "reverse": False, "block_quic": True},
    "Dengeli": {"mode": "hepsi", "fake": "checksum", "reverse": False, "block_quic": True},
    "Agresif": {"mode": "hepsi", "fake": "ttl",      "reverse": True,  "block_quic": True},
}


def site_check(host, timeout=6):
    """DNS + TLS el sıkışması dener. (başarılı mı, açıklama) döndürür."""
    import socket
    import ssl
    t0 = time.time()
    try:
        ip = socket.gethostbyname(host)
    except OSError:
        return False, "Adres bulunamadı (DNS)"
    if ip.startswith(("195.175.254.", "193.192.98.")) or ip in ("127.0.0.1", "0.0.0.0"):
        return False, f"DNS engellenmiş ({ip}). Ayarlar'dan DNS yönlendirmeyi aç."
    try:
        with socket.create_connection((ip, 443), timeout=timeout) as s:
            with ssl.create_default_context().wrap_socket(s, server_hostname=host):
                pass
    except ssl.SSLCertVerificationError:
        return False, "Sertifika tutmuyor, başka yere yönlendiriliyor olabilir"
    except ConnectionResetError:
        return False, "Bağlantı kesildi, muhtemelen engelli"
    except ssl.SSLError:
        return False, "Site bağlantıyı kabul etmedi (engel ya da sitenin kendi koruması)"
    except socket.timeout:
        return False, "Cevap gelmedi"
    except OSError as e:
        return False, f"Hata: {(e.strerror or str(e))[:50]}"
    return True, f"Açılıyor ({int((time.time() - t0) * 1000)} ms)"


# ----------------------------------------------------------------- arayüz
import customtkinter as ctk  # noqa: E402

C = {
    "header": "#0b2a6b", "bg": "#0f141b", "side": "#0a0e13", "card": "#161d27", "card2": "#1c2532",
    "line": "#243042", "text": "#e6edf6", "mute": "#8a98ab", "blue": "#2f80ff", "green": "#22c55e",
    "red": "#ef4444", "glow_on": "#123d2a", "glow_off": "#1a2230",
}
FONT = "Segoe UI" if WINDOWS else "DejaVu Sans"


class PowerButton(ctk.CTkCanvas):
    """Ortadaki büyük VPN düğmesi; açıkken nabız gibi parlar."""

    def __init__(self, master, command):
        super().__init__(master, width=220, height=220, bg=C["bg"], highlightthickness=0)
        self.command = command
        self.on = False
        self.phase = 0.0
        self.bind("<Button-1>", lambda e: self.command())
        self.configure(cursor="hand2")
        self.draw()
        self.after(40, self.tick)

    def set(self, on):
        self.on = on
        self.draw()

    def tick(self):
        if self.on:
            self.phase = (self.phase + 0.08) % (2 * math.pi)
            self.draw()
        self.after(40, self.tick)

    def draw(self):
        self.delete("all")
        cx = cy = 110
        if self.on:
            pulse = (math.sin(self.phase) + 1) / 2
            for i, col in enumerate(("#0d2a1f", "#10372a", "#134634")):
                r = 104 - i * 10 + pulse * 4
                self.create_oval(cx - r, cy - r, cx + r, cy + r, fill=col, outline="")
            ring, core, icon = C["green"], "#0f2a1f", C["green"]
        else:
            for i, col in enumerate(("#121922", "#151d28")):
                r = 100 - i * 10
                self.create_oval(cx - r, cy - r, cx + r, cy + r, fill=col, outline="")
            ring, core, icon = "#334155", "#111823", "#94a3b8"
        self.create_oval(cx - 70, cy - 70, cx + 70, cy + 70, fill=core, outline=ring, width=4)
        self.create_arc(cx - 30, cy - 30, cx + 30, cy + 30, start=120, extent=300,
                        style="arc", outline=icon, width=7)
        self.create_line(cx, cy - 40, cx, cy - 6, fill=icon, width=7, capstyle="round")


class App(ctk.CTk):
    def __init__(self, tray_start=False):
        ctk.set_appearance_mode("dark")
        super().__init__(fg_color=C["bg"])
        self.title(APP)
        self.geometry("900x580")
        self.minsize(780, 520)
        self.cfg = load_cfg()
        self.engine = None
        self.tray = None
        self.started_at = None
        self.logq = queue.Queue()
        try:                                         # başlık ve görev çubuğu simgesi: kalkan
            ico = os.path.join(os.path.dirname(CONFIG), "gdpi.ico")
            shield_image(True, 256).save(ico, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (256, 256)])
            if not WINDOWS:
                raise OSError                       # Linux'ta .ico başlık simgesi desteklenmez
            self.after(300, lambda: self.iconbitmap(ico))   # CTk kendi simgesini 200 ms'de koyuyor
            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("gdpi.app")
        except Exception:
            pass

        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(1, weight=1)
        self.build_header()
        self.build_sidebar()
        self.pages = {}
        self.build_home()
        self.build_sites()
        self.build_test()
        self.build_settings()
        self.build_log()
        self.build_statusbar()
        self.show_page("home")

        self.protocol("WM_DELETE_WINDOW", self.hide)
        self.bind("<Unmap>", lambda e: self.hide() if e.widget is self and self.state() == "iconic" else None)
        self.after(200, self.drain_log)
        self.after(1000, self.refresh_stats)
        self.make_tray()
        self.after(50, self.style_titlebar)
        if not is_admin():
            self.log("Yönetici izni yok — " + ("GDPI.bat" if WINDOWS else "./gdpi.sh") + " ile açın.")
        if tray_start:
            self.withdraw()
            self.toggle()

    # ---------- iskelet
    def style_titlebar(self):
        """Windows 11 başlık çubuğunu üst barla aynı renge boyar; simge durumu/büyüt/kapat
        tuşları ve Snap düzenleri Windows'un kendi tuşları olarak sağ üstte kalır."""
        try:
            hwnd = ctypes.windll.user32.GetParent(self.winfo_id())
            dwm = ctypes.windll.dwmapi.DwmSetWindowAttribute
            rgb = lambda h: int(h[5:7] + h[3:5] + h[1:3], 16)            # #RRGGBB -> COLORREF
            for attr, val in ((20, 1), (35, rgb(C["header"])), (36, 0xFFFFFF), (34, rgb(C["header"]))):
                dwm(hwnd, attr, ctypes.byref(ctypes.c_int(val)), 4)       # koyu mod, başlık, yazı, kenar
        except Exception:
            pass

    def build_header(self):
        h = ctk.CTkFrame(self, fg_color=C["header"], corner_radius=0, height=58)
        h.grid(row=0, column=0, columnspan=2, sticky="ew")
        h.grid_propagate(False)
        h.grid_columnconfigure(3, weight=1)
        try:
            self._logo = ctk.CTkImage(shield_image(True, 64), size=(30, 30))
            ctk.CTkLabel(h, image=self._logo, text="").grid(row=0, column=0, padx=(18, 8), pady=14)
        except Exception:
            pass
        ctk.CTkLabel(h, text="gdpi", font=(FONT, 22, "bold"), text_color="white").grid(row=0, column=1)
        ctk.CTkLabel(h, text=" KİŞİSEL ", font=(FONT, 10, "bold"), text_color="#cbd5e1",
                     fg_color="#1e3a7a", corner_radius=10).grid(row=0, column=2, padx=10)
        ctk.CTkLabel(h, text="Görüşürüz Derin Paket İnceleme", font=(FONT, 12),
                     text_color="#93a4c3").grid(row=0, column=4, padx=18, sticky="e")

    def build_sidebar(self):
        s = ctk.CTkFrame(self, fg_color=C["side"], corner_radius=0, width=200)
        s.grid(row=1, column=0, sticky="ns")
        s.grid_propagate(False)
        self.nav = {}
        items = [("home", "⏻   Bağlantı"), ("sites", "🌐   Siteler"), ("test", "🔎   Site Testi"),
                 ("settings", "⚙   Ayarlar"), ("log", "☰   Kayıt")]
        for i, (key, label) in enumerate(items):
            b = ctk.CTkButton(s, text=label, anchor="w", height=40, corner_radius=8,
                              font=(FONT, 14), fg_color="transparent", hover_color=C["card2"],
                              text_color=C["mute"], command=lambda k=key: self.show_page(k))
            b.pack(fill="x", padx=12, pady=(14 if i == 0 else 4, 0))
            self.nav[key] = b
        ctk.CTkLabel(s, text=f"v{VERSION}", font=(FONT, 11), text_color="#4b5563").pack(side="bottom", pady=12)

    def build_statusbar(self):
        b = ctk.CTkFrame(self, fg_color=C["side"], corner_radius=0, height=30)
        b.grid(row=2, column=0, columnspan=2, sticky="ew")
        b.grid_propagate(False)
        self.sb_dot = ctk.CTkLabel(b, text="●", text_color=C["red"], font=(FONT, 13))
        self.sb_dot.pack(side="left", padx=(16, 4))
        self.sb_text = ctk.CTkLabel(b, text="Kapalı", text_color=C["mute"], font=(FONT, 12))
        self.sb_text.pack(side="left")
        self.sb_right = ctk.CTkLabel(b, text="", text_color=C["mute"], font=(FONT, 12))
        self.sb_right.pack(side="right", padx=16)

    def page(self, key):
        f = ctk.CTkFrame(self, fg_color=C["bg"], corner_radius=0)
        self.pages[key] = f
        return f

    def show_page(self, key):
        for k, f in self.pages.items():
            f.grid_forget()
            self.nav[k].configure(fg_color="transparent", text_color=C["mute"])
        self.pages[key].grid(row=1, column=1, sticky="nsew")
        self.nav[key].configure(fg_color=C["card2"], text_color="white")

    def card(self, parent, **kw):
        return ctk.CTkFrame(parent, fg_color=C["card"], corner_radius=12,
                            border_width=1, border_color=C["line"], **kw)

    # ---------- Bağlantı
    def build_home(self):
        p = self.page("home")
        p.grid_columnconfigure(0, weight=1)
        self.power = PowerButton(p, self.toggle)
        self.power.grid(row=0, column=0, pady=(34, 6))
        self.h_title = ctk.CTkLabel(p, text="Şu an kapalı", font=(FONT, 24, "bold"), text_color=C["text"])
        self.h_title.grid(row=1, column=0)
        self.h_sub = ctk.CTkLabel(p, text="Açmak için ortadaki düğmeye bas", font=(FONT, 13), text_color=C["mute"])
        self.h_sub.grid(row=2, column=0, pady=(2, 22))

        stats = ctk.CTkFrame(p, fg_color="transparent")
        stats.grid(row=3, column=0, padx=30, sticky="ew")
        self.stat = {}
        for i, (key, label) in enumerate([("time", "Süre"), ("split", "Aşılan"),
                                          ("skip", "Dokunulmayan"), ("mode", "Mod")]):
            stats.grid_columnconfigure(i, weight=1, uniform="s")
            c = self.card(stats)
            c.grid(row=0, column=i, padx=6, sticky="ew")
            ctk.CTkLabel(c, text=label, font=(FONT, 11), text_color=C["mute"]).pack(anchor="w", padx=14, pady=(10, 0))
            v = ctk.CTkLabel(c, text="—", font=(FONT, 18, "bold"), text_color=C["text"])
            v.pack(anchor="w", padx=14, pady=(0, 10))
            self.stat[key] = v

        pr = ctk.CTkFrame(p, fg_color="transparent")
        pr.grid(row=4, column=0, pady=(18, 0))
        ctk.CTkLabel(pr, text="Hızlı profil:", font=(FONT, 12), text_color=C["mute"]).pack(side="left", padx=8)
        self.v_preset = ctk.StringVar(value="")
        ctk.CTkSegmentedButton(pr, values=list(PRESETS), variable=self.v_preset, command=self.apply_preset,
                               selected_color=C["blue"], selected_hover_color="#2563eb").pack(side="left")

    def apply_preset(self, name):
        p = PRESETS[name]
        self.v_mode.set("Her site" if p["mode"] == "hepsi" else "Sadece liste")
        self.v_fake.set({"checksum": "Bozuk checksum", "ttl": "Düşük TTL", "kapali": "Kapalı"}[p["fake"]])
        self.v_rev.set(p["reverse"])
        self.v_quic.set(p["block_quic"])
        self.log(f"{name} profiline geçildi.")
        self.apply()

    def apply(self):
        """Ayarları kaydeder; motor açıksa yeni ayarlarla yeniden başlatır."""
        self.read_ui()
        if self.engine:
            self.toggle()
            self.toggle()

    # ---------- Site testi
    def build_test(self):
        p = self.page("test")
        p.grid_columnconfigure(0, weight=1)
        p.grid_rowconfigure(2, weight=1)
        ctk.CTkLabel(p, text="Site Testi", font=(FONT, 22, "bold"), text_color=C["text"]).grid(
            row=0, column=0, sticky="w", padx=24, pady=(20, 8))
        bar = self.card(p)
        bar.grid(row=1, column=0, sticky="ew", padx=24)
        bar.grid_columnconfigure(0, weight=1)
        self.e_test = ctk.CTkEntry(bar, placeholder_text="ör. discord.com", height=38, border_width=0,
                                   fg_color=C["bg"], font=(FONT, 13))
        self.e_test.grid(row=0, column=0, sticky="ew", padx=12, pady=12)
        self.e_test.bind("<Return>", lambda e: self.run_test())
        ctk.CTkButton(bar, text="Test et", height=38, width=110, fg_color=C["blue"],
                      command=self.run_test).grid(row=0, column=1, padx=(0, 6))
        ctk.CTkButton(bar, text="Listemdekiler", height=38, width=130, fg_color=C["card2"],
                      hover_color=C["line"], command=self.run_test_all).grid(row=0, column=2, padx=(0, 12))
        self.test_box = ctk.CTkScrollableFrame(p, fg_color="transparent")
        self.test_box.grid(row=2, column=0, sticky="nsew", padx=14, pady=10)
        self.test_box.grid_columnconfigure(0, weight=1)
        self.test_rows = 0

    def run_test(self):
        host = self.e_test.get().strip().lower().removeprefix("https://").removeprefix("http://").split("/")[0]
        if host:
            self.start_test(host)

    def run_test_all(self):
        for h in self.cfg["targets"]:
            self.start_test(h)

    def start_test(self, host):
        row = self.card(self.test_box)
        row.grid(row=self.test_rows, column=0, sticky="ew", padx=10, pady=4)
        self.test_rows += 1
        row.grid_columnconfigure(1, weight=1)
        dot = ctk.CTkLabel(row, text="●", text_color=C["mute"], font=(FONT, 14))
        dot.grid(row=0, column=0, padx=(14, 6), pady=10)
        ctk.CTkLabel(row, text=host, font=(FONT, 13, "bold"), text_color=C["text"]).grid(row=0, column=1, sticky="w")
        res = ctk.CTkLabel(row, text="test ediliyor…", font=(FONT, 12), text_color=C["mute"])
        res.grid(row=0, column=2, padx=14)

        def work():
            ok, msg = site_check(host)
            self.after(0, lambda: (dot.configure(text_color=C["green"] if ok else C["red"]),
                                   res.configure(text=msg, text_color=C["green"] if ok else C["red"])))
        threading.Thread(target=work, daemon=True).start()

    # ---------- Siteler
    def build_sites(self):
        p = self.page("sites")
        p.grid_columnconfigure((0, 1), weight=1, uniform="l")
        p.grid_rowconfigure(1, weight=1)
        ctk.CTkLabel(p, text="Siteler", font=(FONT, 22, "bold"), text_color=C["text"]).grid(
            row=0, column=0, sticky="w", padx=24, pady=(20, 8))
        self.t_targets = self.list_card(p, 0, "Engelli siteler", "Sadece-liste modunda bunlara uygulanır",
                                        C["blue"], self.cfg["targets"])
        self.t_bypass = self.list_card(p, 1, "Asla dokunma", "Banka, devlet, X / Grok… hiç ellenmez",
                                       C["green"], self.cfg["bypass"])

    def list_card(self, p, col, title, sub, accent, items):
        c = self.card(p)
        c.grid(row=1, column=col, sticky="nsew", padx=(24 if col == 0 else 8, 8 if col == 0 else 24), pady=(0, 20))
        c.grid_rowconfigure(2, weight=1)
        c.grid_columnconfigure(0, weight=1)
        top = ctk.CTkFrame(c, fg_color="transparent")
        top.grid(row=0, column=0, sticky="ew", padx=14, pady=(12, 0))
        ctk.CTkLabel(top, text="●", text_color=accent, font=(FONT, 13)).pack(side="left")
        ctk.CTkLabel(top, text=" " + title, font=(FONT, 15, "bold"), text_color=C["text"]).pack(side="left")
        ctk.CTkLabel(c, text=sub, font=(FONT, 11), text_color=C["mute"]).grid(row=1, column=0, sticky="w", padx=14)
        t = ctk.CTkTextbox(c, fg_color=C["bg"], border_width=0, corner_radius=8,
                           font=("Consolas", 12), text_color=C["text"])
        t.grid(row=2, column=0, sticky="nsew", padx=12, pady=12)
        t.insert("1.0", "\n".join(items))
        return t

    # ---------- Ayarlar
    def build_settings(self):
        p = self.page("settings")
        p.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(p, text="Ayarlar", font=(FONT, 22, "bold"), text_color=C["text"]).grid(
            row=0, column=0, sticky="w", padx=24, pady=(20, 8))
        ctk.CTkButton(p, text="Kaydet ve uygula", width=150, height=34, fg_color=C["blue"],
                      command=self.apply).grid(row=0, column=0, sticky="e", padx=24, pady=(20, 8))
        box = ctk.CTkScrollableFrame(p, fg_color="transparent")
        box.grid(row=1, column=0, sticky="nsew", padx=14)
        p.grid_rowconfigure(1, weight=1)
        box.grid_columnconfigure(0, weight=1)

        self.v_mode = ctk.StringVar(value={"hepsi": "Her site", "liste": "Sadece liste"}[self.cfg["mode"]])
        self.v_fake = ctk.StringVar(value={"checksum": "Bozuk checksum", "ttl": "Düşük TTL",
                                           "kapali": "Kapalı"}[self.cfg["fake"]])
        self.v_ttl = ctk.StringVar(value=str(self.cfg["ttl"]))
        self.v_rev = ctk.BooleanVar(value=self.cfg["reverse"])
        self.v_quic = ctk.BooleanVar(value=self.cfg["block_quic"])
        self.v_dns = ctk.BooleanVar(value=self.cfg["dns_redirect"])
        self.v_auto = ctk.BooleanVar(value=autostart_enabled())

        seg = lambda var, vals: lambda c: ctk.CTkSegmentedButton(
            c, values=vals, variable=var, selected_color=C["blue"], selected_hover_color="#2563eb")
        sw = lambda var, cmd=None: lambda c: ctk.CTkSwitch(
            c, text="", variable=var, command=cmd, progress_color=C["blue"])
        rows = [
            ("Çalışma modu", "“Sadece liste” en güvenlisi; “Her site” dokunma listesi hariç hepsine uygular",
             seg(self.v_mode, ["Her site", "Sadece liste"])),
            ("Sahte paket", "Bozuk checksum: yakın sunucuları bozmaz (önerilen)",
             seg(self.v_fake, ["Bozuk checksum", "Düşük TTL", "Kapalı"])),
            ("TTL değeri", "Sadece “Düşük TTL” seçiliyken; genelde 3–6",
             lambda c: ctk.CTkOptionMenu(c, values=[str(i) for i in range(1, 13)], variable=self.v_ttl,
                                         width=80, fg_color=C["card2"], button_color=C["line"])),
            ("Parçaları ters gönder", "Bazı operatörlerde işe yarar", sw(self.v_rev)),
            ("QUIC'i engelle", "Tarayıcıyı bölünebilen TLS'e zorlar", sw(self.v_quic)),
            ("DNS yönlendir", "Operatör DNS'ini aşar (Yandex 77.88.8.8:1253)", sw(self.v_dns)),
            ("Windows ile başlat", "Açılışta tepside ve açık başlar, onay sormaz",
             sw(self.v_auto, self.toggle_autostart)),
        ]
        for i, (title, sub, make) in enumerate(rows):
            c = self.card(box)
            c.grid(row=i, column=0, sticky="ew", padx=10, pady=5)
            c.grid_columnconfigure(0, weight=1)
            ctk.CTkLabel(c, text=title, font=(FONT, 14, "bold"), text_color=C["text"]).grid(
                row=0, column=0, sticky="w", padx=16, pady=(10, 0))
            ctk.CTkLabel(c, text=sub, font=(FONT, 11), text_color=C["mute"]).grid(
                row=1, column=0, sticky="w", padx=16, pady=(0, 10))
            make(c).grid(row=0, column=1, rowspan=2, padx=16)

    # ---------- Kayıt
    def build_log(self):
        p = self.page("log")
        p.grid_columnconfigure(0, weight=1)
        p.grid_rowconfigure(1, weight=1)
        ctk.CTkLabel(p, text="Kayıt", font=(FONT, 22, "bold"), text_color=C["text"]).grid(
            row=0, column=0, sticky="w", padx=24, pady=(20, 8))
        btns = ctk.CTkFrame(p, fg_color="transparent")
        btns.grid(row=0, column=0, sticky="e", padx=24)
        for text, cmd in (("Kopyala", self.copy_log), ("Temizle", self.clear_log)):
            ctk.CTkButton(btns, text=text, width=90, height=32, fg_color=C["card2"], hover_color=C["line"],
                          command=cmd).pack(side="left", padx=4)
        self.t_log = ctk.CTkTextbox(p, fg_color=C["card"], border_width=1, border_color=C["line"],
                                    corner_radius=12, font=("Consolas", 12), text_color="#86efac")
        self.t_log.grid(row=1, column=0, sticky="nsew", padx=24, pady=(0, 20))
        self.t_log.configure(state="disabled")

    def toggle_autostart(self):
        err = set_autostart(self.v_auto.get())
        if err:
            self.v_auto.set(False)
            self.log(err)
        elif self.v_auto.get():
            self.log("Windows ile başlatma açık (Program Files\\GDPI\\GDPI.exe).")

    def copy_log(self):
        self.clipboard_clear()
        self.clipboard_append(self.t_log.get("1.0", "end"))

    def clear_log(self):
        self.t_log.configure(state="normal")
        self.t_log.delete("1.0", "end")
        self.t_log.configure(state="disabled")

    # ---------- tepsi
    def notify(self, msg):
        """Pencere gizliyken Windows bildirimi gösterir."""
        if self.tray and self.state() == "withdrawn":
            try:
                self.tray.notify(msg, "GDPI")
            except Exception:
                pass

    def make_tray(self):
        try:
            import pystray
        except ImportError:
            self.log("pystray yok: pip install pystray pillow (tepsi simgesi kapalı)")
            return
        ui = lambda f: (lambda *_: self.after(0, f))
        menu = pystray.Menu(
            pystray.MenuItem("Göster", ui(self.show), default=True),
            pystray.MenuItem(lambda _: "Durdur" if self.engine else "Başlat", ui(self.toggle)),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Çıkış", ui(self.quit_app)))
        try:
            self.tray = pystray.Icon("GDPI", shield_image(False), "GDPI - Kapalı", menu)
            self.tray.run_detached()
        except Exception as e:                       # Linux: tepsi desteği olmayan masaüstü
            self.tray = None
            self.log(f"Tepsi simgesi kullanılamıyor: {e}")

    def update_ui_state(self):
        on = bool(self.engine)
        self.power.set(on)
        self.h_title.configure(text="Açık" if on else "Şu an kapalı",
                               text_color=C["green"] if on else C["text"])
        self.h_sub.configure(text="Engeller aşılıyor. Bankalara ve devlet sitelerine dokunulmuyor." if on
                             else "Açmak için ortadaki düğmeye bas")
        self.sb_dot.configure(text_color=C["green"] if on else C["red"])
        self.sb_text.configure(text="Çalışıyor" if on else "Kapalı")
        self.notify("GDPI açıldı" if on else "GDPI kapatıldı")
        if self.tray:
            self.tray.icon = shield_image(on)
            self.tray.title = "GDPI - Açık" if on else "GDPI - Kapalı"
            self.tray.update_menu()

    def refresh_stats(self):
        e = self.engine
        if e and self.started_at:
            s = int(time.time() - self.started_at)
            self.stat["time"].configure(text=f"{s // 3600:02}:{s % 3600 // 60:02}:{s % 60:02}")
            self.stat["split"].configure(text=str(e.n_split))
            self.stat["skip"].configure(text=str(e.n_skip))
        else:
            for k in ("time", "split", "skip"):
                self.stat[k].configure(text="—")
        self.stat["mode"].configure(text="Her site" if self.cfg["mode"] == "hepsi" else "Liste")
        self.sb_right.configure(text=f"Sahte paket: {self.cfg['fake']}   ·   "
                                     f"{len(self.cfg['bypass'])} site korumada")
        self.after(1000, self.refresh_stats)

    def hide(self):
        if self.tray:
            self.read_ui()
            self.withdraw()
        else:
            self.quit_app()

    def show(self):
        self.deiconify()
        self.lift()
        self.focus_force()

    # ---------- mantık
    def log(self, msg):
        line = time.strftime("%H:%M:%S  ") + msg
        self.logq.put(line)
        print(line)                              # exe'de %APPDATA%\GDPI\cikti.log dosyasına gider

    def drain_log(self):
        while not self.logq.empty():
            self.t_log.configure(state="normal")
            self.t_log.insert("end", self.logq.get() + "\n")
            self.t_log.see("end")
            self.t_log.configure(state="disabled")
        self.after(200, self.drain_log)

    def read_ui(self):
        lines = lambda t: [l.strip().lower() for l in t.get("1.0", "end").splitlines() if l.strip()]
        self.cfg.update(mode="hepsi" if self.v_mode.get() == "Her site" else "liste",
                        fake={"Bozuk checksum": "checksum", "Düşük TTL": "ttl"}.get(self.v_fake.get(), "kapali"),
                        ttl=int(self.v_ttl.get()), reverse=self.v_rev.get(), block_quic=self.v_quic.get(),
                        dns_redirect=self.v_dns.get(),
                        targets=lines(self.t_targets), bypass=lines(self.t_bypass))
        save_cfg(self.cfg)

    def toggle(self):
        if self.engine:
            self.engine.stop()
            self.engine = None
            self.started_at = None
            self.log("GDPI kapatıldı.")
            self.update_ui_state()
            return
        self.read_ui()
        try:
            self.engine = Engine(self.cfg, self.log)
            self.engine.start()
        except Exception as e:
            self.engine = None
            msg = e.strerror if isinstance(e, OSError) and e.strerror else str(e)
            self.log(f"Açılamadı: {msg}")
            self.show_page("home")                  # hatayı Kayıt'ta saklama, ana ekranda göster
            self.h_title.configure(text="Açılamadı", text_color=C["red"])
            self.h_sub.configure(text=msg, text_color=C["red"])
            return
        self.h_sub.configure(text_color=C["mute"])
        self.started_at = time.time()
        self.log("GDPI açıldı.")
        self.update_ui_state()

    def quit_app(self):
        if self.engine:
            self.engine.stop()
        self.read_ui()
        if self.tray:
            self.tray.stop()
        self.destroy()


def is_admin():
    if not WINDOWS:
        return os.geteuid() == 0
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def selftest():
    """GDPI.exe --selftest: motoru gerçekten başlatır, siteleri dener, sonucu
    %APPDATA%\\GDPI\\selftest.txt dosyasına yazar ve çıkar."""
    out = []
    say = lambda s: out.append(s)
    hosts = ["discord.com", "cdn.discordapp.com", "www.isbank.com.tr", "www.resmigazete.gov.tr",
             "grok.com", "x.com", "www.google.com"]
    say(f"GDPI {VERSION} öz test  {time.strftime('%Y-%m-%d %H:%M:%S')}  yönetici={is_admin()}")
    say("-- GDPI kapalı:")
    for h in hosts:
        say(f"   {h:28} {site_check(h)}")
    cfg = load_cfg()
    e = Engine(cfg, lambda m: say("   log " + m))
    try:
        e.start()
        say("-- GDPI açık (motor başladı):")
        for h in hosts:
            say(f"   {h:28} {site_check(h)}")
        say(f"-- bölünen={e.n_split}  dokunulmayan={e.n_skip}")
    except Exception as ex:
        say(f"-- MOTOR BAŞLAMADI: {ex}")
    finally:
        e.stop()
    with open(os.path.join(DATA_DIR, "selftest.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")


def linux_elevate():
    """Linux: pkexec (yoksa sudo) ile root olarak yeniden başlatır; masaüstü oturum değişkenleri korunur."""
    import shutil
    keep = ("DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "HOME",
            "DBUS_SESSION_BUS_ADDRESS", "LANG")
    env = [f"{k}={os.environ[k]}" for k in keep if os.environ.get(k)]
    if os.environ.get("WAYLAND_DISPLAY") and shutil.which("xhost"):     # XWayland'de root'a pencere izni
        subprocess.run(["xhost", "+SI:localuser:root"], capture_output=True)
    args = [sys.executable, os.path.abspath(__file__), "--no-elevate"] + sys.argv[1:]
    if shutil.which("pkexec") and os.environ.get("DISPLAY"):
        os.execvp("pkexec", ["pkexec", "env"] + env + args)
    os.execvp("sudo", ["sudo", "env"] + env + args)


if __name__ == "__main__":
    if not WINDOWS and not is_admin() and "--no-elevate" not in sys.argv:
        linux_elevate()
    if WINDOWS and not FROZEN and not is_admin() and "--no-elevate" not in sys.argv:   # exe zaten yönetici ister
        ctypes.windll.shell32.ShellExecuteW(None, "runas", sys.executable,
                                            f'"{os.path.abspath(__file__)}" --no-elevate', None, 1)
        sys.exit()
    try:
        if "--selftest" in sys.argv:
            selftest()
            sys.exit()
        App(tray_start="--tray" in sys.argv).mainloop()
    except SystemExit:
        raise
    except BaseException:                       # pencereli exe hatayı gösteremez: dosyaya yaz
        import traceback
        with open(os.path.join(DATA_DIR, "hata.txt"), "w", encoding="utf-8") as f:
            f.write(traceback.format_exc())
        raise
