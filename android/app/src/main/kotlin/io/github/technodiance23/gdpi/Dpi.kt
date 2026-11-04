package io.github.technodiance23.gdpi

/**
 * gdpi.py'deki find_sni / find_http_host / domain_in ile aynı mantık.
 * Android'e bağımlılığı yok, birim testleri JVM'de çalışır.
 */
object Dpi {
    /** Masaüstündeki DEFAULTS["targets"]. */
    val DEFAULT_TARGETS = listOf(
        "discord.com", "discord.gg", "discordapp.com", "discordapp.net",
        "discord.media", "roblox.com", "wattpad.com", "pastebin.com",
    )

    /** Asla dokunulmayacaklar. Kullanıcının listesinden bağımsız olarak her zaman geçerli. */
    val ALWAYS_BYPASS = listOf(
        "isbank.com.tr", "gov.tr", "turkiye.gov.tr", "resmigazete.gov.tr",
        "x.ai", "grok.com", "x.com", "twimg.com", "statsig.com", "statsigapi.net",
        "sentry.io", "mcp.cloudflare.com", "challenges.cloudflare.com", "googleapis.com",
        "garantibbva.com.tr", "akbank.com", "yapikredi.com.tr", "ziraatbank.com.tr",
        "vakifbank.com.tr", "halkbank.com.tr", "qnb.com.tr", "denizbank.com",
        "enpara.com", "papara.com", "epicgames.com",
    )

    private fun u8(b: ByteArray, i: Int) = b[i].toInt() and 0xFF
    private fun u16(b: ByteArray, i: Int) = (u8(b, i) shl 8) or u8(b, i + 1)

    /** TLS ClientHello içindeki SNI'nin (başlangıç, uzunluk) değeri; yoksa null. */
    fun findSni(p: ByteArray, len: Int = p.size): Pair<Int, Int>? {
        try {
            if (len < 44 || p[0] != 0x16.toByte() || p[5] != 0x01.toByte()) return null
            var i = 43
            i += 1 + u8(p, i)                    // session id
            i += 2 + u16(p, i)                   // şifre takımları
            i += 1 + u8(p, i)                    // sıkıştırma
            val end = i + 2 + u16(p, i)
            i += 2
            while (i + 4 <= end && i + 4 <= len) {
                val type = u16(p, i)
                val elen = u16(p, i + 2)
                if (type == 0) {
                    if (i + 9 > len) return null
                    val nlen = u16(p, i + 7)
                    return if (i + 9 + nlen <= len) Pair(i + 9, nlen) else null
                }
                i += 4 + elen
            }
        } catch (_: IndexOutOfBoundsException) {
        }
        return null
    }

    private val METHODS = listOf("GET ", "POST ", "HEAD ", "PUT ", "OPTIONS ").map { it.toByteArray() }
    private val HOST = "\r\nHost:".toByteArray()
    private val CRLF = "\r\n".toByteArray()

    private fun indexOf(p: ByteArray, len: Int, needle: ByteArray, from: Int = 0): Int {
        outer@ for (i in from..len - needle.size) {
            for (j in needle.indices) if (p[i + j] != needle[j]) continue@outer
            return i
        }
        return -1
    }

    private fun startsWith(p: ByteArray, len: Int, pre: ByteArray) =
        len >= pre.size && pre.indices.all { p[it] == pre[it] }

    /** HTTP isteğindeki Host değerinin (başlangıç, uzunluk) değeri; yoksa null. */
    fun findHttpHost(p: ByteArray, len: Int = p.size): Pair<Int, Int>? {
        if (METHODS.none { startsWith(p, len, it) }) return null
        val pos = indexOf(p, len, HOST)
        if (pos == -1) return null
        var s = pos + 7
        while (s < len && p[s] == 0x20.toByte()) s++
        val e = indexOf(p, len, CRLF, s)
        return Pair(s, (if (e != -1) e else len) - s)
    }

    fun domainIn(host: String, domains: Collection<String>): Boolean {
        val h = host.lowercase().trimEnd('.')
        return domains.any { h == it || h.endsWith(".$it") }
    }

    /** Masaüstündeki Engine.should_touch. onlyTargets: "sadece hedef listesi" modu. */
    fun shouldTouch(host: String, bypass: Collection<String>, targets: Collection<String>, onlyTargets: Boolean): Boolean {
        if (domainIn(host, ALWAYS_BYPASS) || domainIn(host, bypass)) return false
        if (onlyTargets) return domainIn(host, targets)
        return true
    }

    class Plan(val host: String, val data: ByteArray, val cuts: List<Int>) {
        /** Kesim noktalarına göre parçalar (sıralı, boş parça yok). */
        fun parts(): List<ByteArray> {
            val c = cuts.filter { it in 1 until data.size }.distinct().sorted()
            val b = listOf(0) + c + listOf(data.size)
            return (0 until b.size - 1).map { data.copyOfRange(b[it], b[it + 1]) }
        }
    }

    /**
     * İlk yükte alan adını bulur, bulamazsa null döner.
     * Kesim noktaları masaüstüyle aynı: TLS'de kayıt başlığının içinden (2) ve alan adının ortasından;
     * HTTP'de "Host:" başlığı "hoSt:" yapılır, başlığın içinden ve adın ortasından kesilir.
     */
    fun locate(payload: ByteArray, len: Int, port: Int): Plan? {
        val loc = (if (port == 80) findHttpHost(payload, len) else findSni(payload, len)) ?: return null
        val (start, ln) = loc
        if (ln <= 0) return null
        val host = String(payload, start, ln, Charsets.US_ASCII)
        val data = payload.copyOf(len)
        val cuts = if (port == 80) {
            val hpos = indexOf(data, len, HOST)
            "\r\nhoSt:".toByteArray().copyInto(data, hpos)
            listOf(hpos + 4, start + ln / 2)
        } else {
            listOf(2, start + ln / 2)
        }
        return Plan(host, data, cuts)
    }

    /** Ayarlardaki satırları temizler: küçük harf, boşluk yok, geçerli alan adı. */
    private val DOMAIN_RE = Regex("^[a-z0-9-]+(\\.[a-z0-9-]+)*$")
    fun cleanList(text: String): List<String> =
        text.split('\n', ',', ' ').map { it.trim().lowercase().trimEnd('.') }
            .filter { it.isNotEmpty() && DOMAIN_RE.matches(it) }.distinct()
}
