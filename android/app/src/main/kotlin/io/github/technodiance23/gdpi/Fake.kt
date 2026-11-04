package io.github.technodiance23.gdpi

/** native/fake.c: root olmadan sahte paket (sahte içerik düşük TTL ile gider, gerçeği yeniden gönderimde). */
object Fake {
    val available: Boolean = try {
        System.loadLibrary("gdpi-fake"); true
    } catch (_: Throwable) {
        false
    }

    /** Gönderilen bayt sayısını, hata olursa eksi errno döndürür. */
    @JvmStatic external fun send(fd: Int, fake: ByteArray, real: ByteArray, ttl: Int, v6: Boolean): Int
}
