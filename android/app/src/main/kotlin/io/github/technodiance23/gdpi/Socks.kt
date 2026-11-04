package io.github.technodiance23.gdpi

import android.os.ParcelFileDescriptor
import android.system.Os
import android.util.Log
import java.io.DataInputStream
import java.io.InputStream
import java.io.OutputStream
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread

/**
 * Yerel SOCKS5 sunucusu. hev-socks5-tunnel telefondaki TCP/UDP trafiğini buraya getirir.
 * 80/443 bağlantılarında ilk yük okunur, alan adı bulunur ve dokunulacaksa gerçek sokete
 * parça parça yazılır (TCP_NODELAY açık, her parça ayrı segment olarak çıkar).
 */
class Socks(
    private val protect: (Socket) -> Boolean,
    private val protectUdp: (DatagramSocket) -> Boolean,
    private val cfg: Snapshot,
) {
    class Snapshot(
        val onlyTargets: Boolean, val ttlMode: Boolean, val ttl: Int, val blockQuic: Boolean,
        val dnsRedirect: Boolean, val targets: List<String>, val bypass: List<String>,
    )

    /** Her açılışta rastgele kimlik: telefondaki başka uygulamalar bu yerel vekili kullanamasın. */
    val user: String = randomHex()
    val pass: String = randomHex()

    val nSplit = AtomicInteger()
    val nSkip = AtomicInteger()
    private val server = ServerSocket(0, 128, InetAddress.getByName("127.0.0.1"))
    val port: Int get() = server.localPort
    @Volatile private var running = true

    fun start() {
        thread(name = "socks-accept", isDaemon = true) {
            while (running) {
                val c = try { server.accept() } catch (_: Exception) { break }
                thread(isDaemon = true) { handle(c) }
            }
        }
    }

    fun stop() {
        running = false
        try { server.close() } catch (_: Exception) {}
    }

    private fun handle(c: Socket) {
        try {
            c.tcpNoDelay = true
            val inp = DataInputStream(c.getInputStream())
            val out = c.getOutputStream()
            // selamlaşma: yalnızca kullanıcı adı/parola (RFC 1929)
            if (inp.readUnsignedByte() != 5) return c.close()
            val methods = ByteArray(inp.readUnsignedByte()).also { inp.readFully(it) }
            if (2.toByte() !in methods) { out.write(byteArrayOf(5, 0xFF.toByte())); return c.close() }
            out.write(byteArrayOf(5, 2))
            if (inp.readUnsignedByte() != 1) return c.close()
            val u = String(ByteArray(inp.readUnsignedByte()).also { inp.readFully(it) })
            val pw = String(ByteArray(inp.readUnsignedByte()).also { inp.readFully(it) })
            val ok = MessageDigest.isEqual(u.toByteArray(), user.toByteArray()) and
                MessageDigest.isEqual(pw.toByteArray(), pass.toByteArray())
            out.write(byteArrayOf(1, if (ok) 0 else 1))
            if (!ok) return c.close()
            // istek
            val hdr = ByteArray(4)
            inp.readFully(hdr)
            val cmd = hdr[1].toInt()
            val addr: InetAddress = when (hdr[3].toInt()) {
                1 -> InetAddress.getByAddress(ByteArray(4).also { inp.readFully(it) })
                4 -> InetAddress.getByAddress(ByteArray(16).also { inp.readFully(it) })
                3 -> InetAddress.getByName(String(ByteArray(inp.readUnsignedByte()).also { inp.readFully(it) }))
                else -> return c.close()
            }
            val port = inp.readUnsignedShort()
            when (cmd) {
                1 -> connect(c, inp, out, addr, port)
                3 -> udpAssociate(c, inp, out)
                else -> { out.write(byteArrayOf(5, 7, 0, 1, 0, 0, 0, 0, 0, 0)); c.close() }
            }
        } catch (_: Exception) {
            try { c.close() } catch (_: Exception) {}
        }
    }

    private fun reply(out: OutputStream, ok: Boolean) =
        out.write(byteArrayOf(5, if (ok) 0 else 5, 0, 1, 0, 0, 0, 0, 0, 0))

    private fun connect(c: Socket, inp: InputStream, out: OutputStream, addr: InetAddress, port: Int) {
        val r = Socket()
        try {
            protect(r)
            r.tcpNoDelay = true
            r.connect(InetSocketAddress(addr, port), 10_000)
        } catch (_: Exception) {
            reply(out, false); c.close(); r.close(); return
        }
        reply(out, true)
        val rout = r.getOutputStream()
        if (port == 80 || port == 443) {
            val buf = ByteArray(16384)
            val n = inp.read(buf)
            if (n <= 0) { r.close(); c.close(); return }
            sendFirst(r, rout, buf, n, port)
        }
        val done = AtomicInteger()
        pipe(inp, rout, r, c, r, done)
        pipe(r.getInputStream(), out, c, c, r, done)
    }

    private fun sendFirst(r: Socket, out: OutputStream, buf: ByteArray, n: Int, port: Int) {
        val plan = Dpi.locate(buf, n, port)
        if (plan == null || !Dpi.shouldTouch(plan.host, cfg.bypass, cfg.targets, cfg.onlyTargets)) {
            if (plan != null) nSkip.incrementAndGet()
            out.write(buf, 0, n)
            return
        }
        nSplit.incrementAndGet()
        for ((k, part) in plan.parts().withIndex()) {
            // Düşük TTL modu: ilk parça DPI'a kadar gider ama sunucuya varmadan düşer.
            // Çekirdek onu birazdan normal TTL ile yeniden yollar, sunucu baytları doğru sırayla alır.
            val low = cfg.ttlMode && k == 0
            if (low) setTtl(r, cfg.ttl)
            out.write(part)
            if (low) { Thread.sleep(5); setTtl(r, 64) }
        }
    }

    private fun setTtl(s: Socket, ttl: Int) {
        try {
            ParcelFileDescriptor.fromSocket(s).use { pfd ->
                if (s.inetAddress is Inet6Address) Os.setsockoptInt(pfd.fileDescriptor, 41 /* IPPROTO_IPV6 */, 16 /* IPV6_UNICAST_HOPS */, ttl)
                else Os.setsockoptInt(pfd.fileDescriptor, 0 /* IPPROTO_IP */, 2 /* IP_TTL */, ttl)
            }
        } catch (e: Exception) {
            Log.w("GDPI", "TTL ayarlanamadı: $e")
        }
    }

    /** Bir yön bitince karşı tarafa FIN gider; iki yön de bitince iki soket kapanır. */
    private fun pipe(src: InputStream, dst: OutputStream, dstSock: Socket, a: Socket, b: Socket, done: AtomicInteger) {
        thread(isDaemon = true) {
            val buf = ByteArray(16384)
            try {
                while (true) {
                    val n = src.read(buf)
                    if (n < 0) break
                    dst.write(buf, 0, n)
                }
                dstSock.shutdownOutput()
            } catch (_: Exception) {
                done.incrementAndGet()          // hata: hemen kapat
            }
            if (done.incrementAndGet() >= 2) {
                try { a.close() } catch (_: Exception) {}
                try { b.close() } catch (_: Exception) {}
            }
        }
    }

    // ---- UDP ASSOCIATE: QUIC (443) düşürülür, istenirse DNS Yandex'in 1253 portuna gider
    private fun udpAssociate(c: Socket, inp: InputStream, out: OutputStream) {
        val local = DatagramSocket(0, InetAddress.getByName("127.0.0.1"))
        val remote = DatagramSocket()
        protectUdp(remote)
        val lp = local.localPort
        out.write(byteArrayOf(5, 0, 0, 1, 127, 0, 0, 1, (lp shr 8).toByte(), lp.toByte()))
        val client = AtomicReference<InetSocketAddress?>(null)
        val dns = InetAddress.getByName(Config.DNS_ADDR)
        val dnsOrig = HashMap<Int, InetSocketAddress>()   // DNS sorgu kimliği -> asıl sunucu

        thread(isDaemon = true) {               // telefon -> dışarı
            val d = ByteArray(65535)
            try {
                while (true) {
                    val p = DatagramPacket(d, d.size)
                    local.receive(p)
                    // ilk paketi gönderen istemciye kilitlen, başkasından geleni at
                    val src = InetSocketAddress(p.address, p.port)
                    if (!client.compareAndSet(null, src) && client.get() != src) continue
                    val len = p.length
                    if (len < 10 || d[2].toInt() != 0) continue          // parçalı paket desteklenmez
                    var i: Int
                    val addr = when (d[3].toInt()) {
                        1 -> { i = 8; InetAddress.getByAddress(d.copyOfRange(4, 8)) }
                        4 -> { i = 20; InetAddress.getByAddress(d.copyOfRange(4, 20)) }
                        3 -> { val l = d[4].toInt() and 0xFF; i = 5 + l; InetAddress.getByName(String(d, 5, l)) }
                        else -> continue
                    }
                    val port = ((d[i].toInt() and 0xFF) shl 8) or (d[i + 1].toInt() and 0xFF)
                    i += 2
                    if (port == 443 && cfg.blockQuic) continue           // QUIC düşer, tarayıcı TCP'ye döner
                    var to = InetSocketAddress(addr, port)
                    if (port == 53 && cfg.dnsRedirect && addr is Inet4Address && len - i >= 2) {
                        val id = ((d[i].toInt() and 0xFF) shl 8) or (d[i + 1].toInt() and 0xFF)
                        synchronized(dnsOrig) { dnsOrig[id] = to }
                        to = InetSocketAddress(dns, Config.DNS_PORT)
                    }
                    remote.send(DatagramPacket(d, i, len - i, to))
                }
            } catch (_: Exception) {
            }
        }
        thread(isDaemon = true) {               // dışarı -> telefon
            val b = ByteArray(65535)
            try {
                while (true) {
                    val p = DatagramPacket(b, 22, b.size - 22)
                    remote.receive(p)
                    var from = InetSocketAddress(p.address, p.port)
                    if (cfg.dnsRedirect && p.port == Config.DNS_PORT && p.address == dns && p.length >= 2) {
                        val id = ((b[22].toInt() and 0xFF) shl 8) or (b[23].toInt() and 0xFF)
                        synchronized(dnsOrig) { dnsOrig.remove(id) }?.let { from = it }
                    }
                    val a = from.address.address
                    val h = 4 + a.size + 2
                    val start = 22 - h
                    b[start] = 0; b[start + 1] = 0; b[start + 2] = 0
                    b[start + 3] = if (a.size == 4) 1 else 4
                    a.copyInto(b, start + 4)
                    b[start + 4 + a.size] = (from.port shr 8).toByte()
                    b[start + 5 + a.size] = from.port.toByte()
                    val cl = client.get() ?: continue
                    local.send(DatagramPacket(b, start, h + p.length, cl))
                }
            } catch (_: Exception) {
            }
        }
        // TCP bağlantısı kapanınca UDP eşleşmesi de biter
        try { while (inp.read() >= 0) { } } catch (_: Exception) {}
        local.close(); remote.close(); c.close()
    }

    private fun randomHex() = ByteArray(16).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) }
}
