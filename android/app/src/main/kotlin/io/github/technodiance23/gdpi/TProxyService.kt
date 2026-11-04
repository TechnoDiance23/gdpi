package io.github.technodiance23.gdpi

/**
 * hev-socks5-tunnel JNI arayüzü. Yerel kütüphane bu sınıfı PKGNAME/TProxyService adıyla aradığı
 * için sınıf adı ve paket değişmemeli (app/build.gradle.kts içindeki PKGNAME ile aynı).
 */
object TProxyService {
    val available: Boolean = try {
        System.loadLibrary("hev-socks5-tunnel")
        true
    } catch (_: Throwable) {
        false
    }

    @JvmStatic external fun TProxyStartService(configPath: String, fd: Int): Boolean
    @JvmStatic external fun TProxyStopService(): Boolean
    @JvmStatic external fun TProxyIsRunning(): Boolean
    @JvmStatic external fun TProxyGetStats(): LongArray?
}
