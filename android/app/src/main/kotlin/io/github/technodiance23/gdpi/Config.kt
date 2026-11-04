package io.github.technodiance23.gdpi

import android.content.Context

/** Ayarlar (SharedPreferences). Anahtarlar masaüstündeki ayarlar.json ile aynı anlamda. */
class Config(ctx: Context) {
    private val sp = ctx.getSharedPreferences("ayarlar", Context.MODE_PRIVATE)

    var onlyTargets: Boolean
        get() = sp.getString("mode", "hepsi") == "liste"
        set(v) = sp.edit().putString("mode", if (v) "liste" else "hepsi").apply()

    /** "fake": önce sahte istek (düşük TTL), "split": sadece böl, "ttl": ilk parça düşük TTL ile gider. */
    var fake: String
        get() = sp.getString("fake", "fake") ?: "fake"
        set(v) = sp.edit().putString("fake", v).apply()

    var ttl: Int
        get() = sp.getInt("ttl", 3)
        set(v) = sp.edit().putInt("ttl", v.coerceIn(1, 32)).apply()

    var blockQuic: Boolean
        get() = sp.getBoolean("block_quic", true)
        set(v) = sp.edit().putBoolean("block_quic", v).apply()

    var dnsRedirect: Boolean
        get() = sp.getBoolean("dns_redirect", false)
        set(v) = sp.edit().putBoolean("dns_redirect", v).apply()

    var targets: List<String>
        get() = sp.getString("targets", null)?.let { Dpi.cleanList(it) } ?: Dpi.DEFAULT_TARGETS
        set(v) = sp.edit().putString("targets", v.joinToString("\n")).apply()

    /** Kullanıcının "asla dokunma" listesi. Dpi.ALWAYS_BYPASS bundan bağımsız olarak hep geçerli. */
    var bypass: List<String>
        get() = sp.getString("bypass", null)?.let { Dpi.cleanList(it) } ?: Dpi.ALWAYS_BYPASS
        set(v) = sp.edit().putString("bypass", v.joinToString("\n")).apply()

    /** VPN'e hiç girmeyecek uygulamalar (paket adı). */
    var excludedApps: List<String>
        get() = sp.getString("apps", null)?.split('\n')?.map { it.trim() }?.filter { it.isNotEmpty() }
            ?: DEFAULT_EXCLUDED_APPS
        set(v) = sp.edit().putString("apps", v.joinToString("\n")).apply()

    companion object {
        const val DNS_ADDR = "77.88.8.8"
        const val DNS_PORT = 1253

        /** Bankalar, e-Devlet, X ve Grok. Telefonda kurulu olmayanlar sessizce atlanır. */
        val DEFAULT_EXCLUDED_APPS = listOf(
            "com.pozitron.iscep", "com.garanti.cepsubesi", "com.akbank.android.apps.akbank_direkt",
            "com.ykb.android", "com.ziraat.ziraatmobil", "com.vakifbank.mobile", "com.tmobtech.halkbank",
            "com.qnbfinansbank.mobile", "com.denizbank.mobildeniz", "finansbank.enpara", "com.mobillium.papara",
            "tr.gov.turkiye.edevlet.kapisi", "com.twitter.android", "ai.x.grok",
        )
    }
}
