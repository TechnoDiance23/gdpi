package io.github.technodiance23.gdpi

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream

class DpiTest {
    /** Gerçekçi bir TLS ClientHello: önce başka eklentiler, sonra SNI. */
    private fun clientHello(host: String, sessionId: Int = 32): ByteArray {
        val name = host.toByteArray()
        val ext = ByteArrayOutputStream()
        fun u16(v: Int) { ext.write(v shr 8); ext.write(v and 0xFF) }
        u16(0x000b); u16(2); ext.write(1); ext.write(0)          // ec_point_formats
        u16(0x0000); u16(name.size + 5); u16(name.size + 3); ext.write(0); u16(name.size); ext.write(name)
        u16(0x002b); u16(3); ext.write(2); ext.write(3); ext.write(4)   // supported_versions
        val e = ext.toByteArray()

        val body = ByteArrayOutputStream()
        body.write(3); body.write(3)
        body.write(ByteArray(32) { 7 })
        body.write(sessionId); body.write(ByteArray(sessionId) { 1 })
        body.write(0); body.write(4); body.write(byteArrayOf(0x13, 0x01, 0x13, 0x02))
        body.write(1); body.write(0)
        body.write(e.size shr 8); body.write(e.size and 0xFF); body.write(e)
        val b = body.toByteArray()

        val hs = byteArrayOf(1, 0, (b.size shr 8).toByte(), b.size.toByte()) + b
        return byteArrayOf(0x16, 3, 1, (hs.size shr 8).toByte(), hs.size.toByte()) + hs
    }

    @Test fun sniFound() {
        val p = clientHello("discord.com")
        val (s, l) = Dpi.findSni(p)!!
        assertEquals("discord.com", String(p, s, l))
    }

    @Test fun sniWithoutSessionId() {
        val p = clientHello("cdn.discordapp.com", sessionId = 0)
        val (s, l) = Dpi.findSni(p)!!
        assertEquals("cdn.discordapp.com", String(p, s, l))
    }

    @Test fun sniTruncatedIsNull() {
        val p = clientHello("discord.com")
        val (s, _) = Dpi.findSni(p)!!
        assertNull(Dpi.findSni(p, s + 3))       // ad yarıda kesilmiş
        assertNull(Dpi.findSni(p, 20))
    }

    @Test fun notTls() {
        assertNull(Dpi.findSni("GET / HTTP/1.1\r\nHost: a.com\r\n\r\n".toByteArray()))
        assertNull(Dpi.findSni(ByteArray(0)))
        assertNull(Dpi.findSni(ByteArray(100) { 0x16 }))
    }

    @Test fun httpHost() {
        val p = "GET / HTTP/1.1\r\nHost:  example.com\r\nAccept: */*\r\n\r\n".toByteArray()
        val (s, l) = Dpi.findHttpHost(p)!!
        assertEquals("example.com", String(p, s, l))
    }

    @Test fun httpHostAtEnd() {
        val p = "POST /x HTTP/1.1\r\nHost: a.b.c".toByteArray()
        val (s, l) = Dpi.findHttpHost(p)!!
        assertEquals("a.b.c", String(p, s, l))
    }

    @Test fun httpNoHostOrNotHttp() {
        assertNull(Dpi.findHttpHost("GET / HTTP/1.1\r\n\r\n".toByteArray()))
        assertNull(Dpi.findHttpHost("SSH-2.0-OpenSSH\r\nHost: a\r\n".toByteArray()))
    }

    @Test fun domainMatching() {
        assertTrue(Dpi.domainIn("www.turkiye.gov.tr", listOf("gov.tr")))
        assertTrue(Dpi.domainIn("GROK.com.", listOf("grok.com")))
        assertFalse(Dpi.domainIn("notx.com", listOf("x.com")))
    }

    @Test fun neverTouchAlwaysWins() {
        // kullanıcı listesi boş olsa bile bankalar, devlet ve X / Grok korunur
        for (h in listOf("www.isbank.com.tr", "www.resmigazete.gov.tr", "grok.com", "api.x.ai",
                         "pbs.twimg.com", "accounts.googleapis.com", "challenges.cloudflare.com")) {
            assertFalse(h, Dpi.shouldTouch(h, emptyList(), listOf(h), onlyTargets = false))
            assertFalse(h, Dpi.shouldTouch(h, emptyList(), listOf(h), onlyTargets = true))
        }
        assertTrue(Dpi.ALWAYS_BYPASS.containsAll(listOf("isbank.com.tr", "gov.tr", "x.com", "statsigapi.net",
            "sentry.io", "mcp.cloudflare.com", "papara.com")))
    }

    @Test fun modes() {
        val t = Dpi.DEFAULT_TARGETS
        assertTrue(Dpi.shouldTouch("discord.com", emptyList(), t, onlyTargets = true))
        assertFalse(Dpi.shouldTouch("example.com", emptyList(), t, onlyTargets = true))
        assertTrue(Dpi.shouldTouch("example.com", emptyList(), t, onlyTargets = false))
        assertFalse(Dpi.shouldTouch("my.site", listOf("site"), t, onlyTargets = false))
    }

    @Test fun tlsSplitCutsHostInMiddle() {
        val p = clientHello("discord.com")
        val plan = Dpi.locate(p, p.size, 443)!!
        assertEquals("discord.com", plan.host)
        val parts = plan.parts()
        assertEquals(3, parts.size)
        assertEquals(2, parts[0].size)
        assertArrayEquals(p, parts.reduce { a, b -> a + b })    // TLS'de veri değişmez
        val (s, l) = Dpi.findSni(p)!!
        assertEquals(s + l / 2, parts[0].size + parts[1].size)
    }

    @Test fun httpSplitMangleHost() {
        val p = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".toByteArray()
        val plan = Dpi.locate(p, p.size, 80)!!
        assertEquals("example.com", plan.host)
        val joined = String(plan.parts().reduce { a, b -> a + b })
        assertEquals("GET / HTTP/1.1\r\nhoSt: example.com\r\n\r\n", joined)
        assertEquals(3, plan.parts().size)
    }

    @Test fun cleanList() {
        assertEquals(listOf("a.com", "b.org"), Dpi.cleanList(" A.com \n\nb.org.\nbad_host!\na.com"))
    }

    @Test
    fun decoyKeepsLengthAndHidesName() {
        val req = "GET / HTTP/1.1\r\nHost: discord.com\r\n\r\n".toByteArray()
        val (start, len) = Dpi.findHttpHost(req)!!
        val d = Dpi.decoy(req, start, len)
        assertEquals(req.size, d.size)
        assertEquals("aaaaaaa.com", String(d, start, len))
        assertEquals(String(req, 0, start), String(d, 0, start))
    }
}
