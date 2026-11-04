package io.github.technodiance23.gdpi

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast

class MainActivity : Activity() {
    private lateinit var cfg: Config
    private lateinit var power: PowerButton
    private lateinit var title: TextView
    private lateinit var sub: TextView
    private lateinit var stats: TextView
    private lateinit var body: LinearLayout
    private val tabs = mutableListOf<TextView>()
    private val ui = Handler(Looper.getMainLooper())
    private var wantOn = false

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        cfg = Config(this)
        window.statusBarColor = C_HEADER
        window.navigationBarColor = C_BG

        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setBackgroundColor(C_BG) }
        root.addView(TextView(this).apply {
            text = "GDPI"
            setTextColor(Color.WHITE)
            textSize = 22f
            typeface = Typeface.DEFAULT_BOLD
            setBackgroundColor(C_HEADER)
            setPadding(dp(20), dp(16), dp(20), dp(16))
        }, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))

        body = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        root.addView(body, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))

        val bar = LinearLayout(this).apply { setBackgroundColor(C_SIDE) }
        for ((i, name) in listOf("Ana sayfa", "Siteler", "Ayarlar").withIndex()) {
            val t = TextView(this).apply {
                text = name
                gravity = Gravity.CENTER
                textSize = 14f
                setPadding(0, dp(14), 0, dp(14))
                setOnClickListener { show(i) }
            }
            tabs += t
            bar.addView(t, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        }
        root.addView(bar, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        // Android 15+ uygulamayı kenardan kenara çizer: başlık durum çubuğunun, sekmeler gezinme çubuğunun altına uzansın
        val header = root.getChildAt(0)
        root.setOnApplyWindowInsetsListener { _, ins ->
            @Suppress("DEPRECATION")
            header.setPadding(dp(20), dp(16) + ins.systemWindowInsetTop, dp(20), dp(16))
            @Suppress("DEPRECATION")
            bar.setPadding(0, 0, 0, ins.systemWindowInsetBottom)
            ins
        }
        setContentView(root)

        power = PowerButton(this).apply { setOnClickListener { toggle() } }
        title = text("", 24f, C_TEXT, bold = true)
        sub = text("", 14f, C_MUTE)
        stats = text("", 13f, C_MUTE)

        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 2)
        show(0)
        tick()
    }

    private fun text(s: String, size: Float, color: Int, bold: Boolean = false) = TextView(this).apply {
        text = s
        textSize = size
        setTextColor(color)
        gravity = Gravity.CENTER
        if (bold) typeface = Typeface.DEFAULT_BOLD
    }

    private fun card() = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = GradientDrawable().apply { setColor(C_CARD); cornerRadius = dp(14).toFloat() }
        setPadding(dp(16), dp(14), dp(16), dp(14))
    }

    private fun cardParams() = LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply {
        setMargins(dp(16), dp(12), dp(16), 0)
    }

    private fun show(i: Int) {
        tabs.forEachIndexed { k, t -> t.setTextColor(if (k == i) C_BLUE else C_MUTE) }
        body.removeAllViews()
        when (i) {
            0 -> home()
            1 -> sites()
            else -> settings()
        }
    }

    // ---------------------------------------------------------------- ana sayfa
    private fun home() {
        val col = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; gravity = Gravity.CENTER }
        (power.parent as? LinearLayout)?.removeView(power)
        listOf(title, sub, stats).forEach { (it.parent as? LinearLayout)?.removeView(it) }
        col.addView(power, LinearLayout.LayoutParams(dp(220), dp(220)))
        col.addView(title, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(16) })
        col.addView(sub, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply {
            topMargin = dp(6); leftMargin = dp(24); rightMargin = dp(24)
        })
        col.addView(stats, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(18) })
        body.addView(col, LinearLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        refresh()
    }

    private fun refresh() {
        val on = GdpiVpnService.running
        power.on = on
        title.text = if (on) "Açık" else "Şu an kapalı"
        title.setTextColor(if (on) C_GREEN else C_TEXT)
        val err = GdpiVpnService.lastError
        sub.text = when {
            on -> "Engeller aşılıyor. Bankalara ve devlet sitelerine dokunulmuyor."
            err != null -> "Açılamadı: $err"
            else -> "Açmak için ortadaki düğmeye bas."
        }
        stats.text = if (on) {
            val s = (System.currentTimeMillis() - GdpiVpnService.startedAt) / 1000
            "%02d:%02d:%02d   ·   bölünen %d   ·   dokunulmayan %d".format(
                s / 3600, s % 3600 / 60, s % 60, GdpiVpnService.nSplit, GdpiVpnService.nSkip
            )
        } else {
            (if (cfg.onlyTargets) "Sadece hedef listesi" else "Her site") + "   ·   " +
                (if (cfg.fake == "ttl") "düşük TTL" else "sadece bölme")
        }
    }

    private fun tick() {
        if (power.isAttachedToWindow) refresh()
        ui.postDelayed({ tick() }, 1000)
    }

    private fun toggle() {
        if (GdpiVpnService.running) {
            GdpiVpnService.stop(this)
            ui.postDelayed({ refresh() }, 300)
            return
        }
        val ask = VpnService.prepare(this)
        if (ask != null) {
            wantOn = true
            startActivityForResult(ask, 1)
        } else {
            GdpiVpnService.start(this)
            ui.postDelayed({ refresh() }, 500)
        }
    }

    @Deprecated("Activity API")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == 1 && wantOn) {
            wantOn = false
            if (resultCode == RESULT_OK) GdpiVpnService.start(this)
            else Toast.makeText(this, "VPN izni verilmedi.", Toast.LENGTH_SHORT).show()
            ui.postDelayed({ refresh() }, 500)
        }
    }

    // ---------------------------------------------------------------- siteler
    private fun sites() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(0, 0, 0, dp(16)) }
        scroll.addView(col)

        val targets = listBox(cfg.targets)
        col.addView(card().apply {
            addView(head("Hedef siteler", C_BLUE))
            addView(note("\"Sadece hedef listesi\" modunda yalnızca bunlar bölünür."))
            addView(targets)
        }, cardParams())

        val bypass = listBox(cfg.bypass)
        col.addView(card().apply {
            addView(head("Asla dokunma", C_GREEN))
            addView(note("Banka, devlet, X / Grok… hiç ellenmez. Bankalar ve gov.tr listeden silinse bile korunur."))
            addView(bypass)
        }, cardParams())

        col.addView(saveButton {
            cfg.targets = Dpi.cleanList(targets.text.toString())
            cfg.bypass = Dpi.cleanList(bypass.text.toString())
        }, cardParams())
        body.addView(scroll, LinearLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }

    private fun head(s: String, color: Int) = TextView(this).apply {
        text = s; textSize = 16f; setTextColor(color); typeface = Typeface.DEFAULT_BOLD
    }

    private fun note(s: String) = TextView(this).apply {
        text = s; textSize = 13f; setTextColor(C_MUTE); setPadding(0, dp(4), 0, dp(8))
    }

    private fun listBox(items: List<String>) = EditText(this).apply {
        setText(items.joinToString("\n"))
        setTextColor(C_TEXT)
        textSize = 14f
        inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        minLines = 4
        gravity = Gravity.TOP
        background = GradientDrawable().apply { setColor(C_CARD2); cornerRadius = dp(10).toFloat() }
        setPadding(dp(12), dp(10), dp(12), dp(10))
    }

    private fun saveButton(save: () -> Unit) = Button(this).apply {
        text = "Kaydet"
        setTextColor(Color.WHITE)
        isAllCaps = false
        background = GradientDrawable().apply { setColor(C_BLUE); cornerRadius = dp(12).toFloat() }
        setOnClickListener {
            save()
            val msg = if (GdpiVpnService.running) "Kaydedildi. Değişiklik için GDPI'ı kapatıp aç." else "Kaydedildi."
            Toast.makeText(this@MainActivity, msg, Toast.LENGTH_SHORT).show()
        }
    }

    // ---------------------------------------------------------------- ayarlar
    private fun settings() {
        val scroll = ScrollView(this)
        val col = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(0, 0, 0, dp(16)) }
        scroll.addView(col)

        val mode = RadioGroup(this)
        val all = radio("Her site (dokunma listesi hariç)")
        val list = radio("Sadece hedef listesi")
        mode.addView(all); mode.addView(list)
        (if (cfg.onlyTargets) list else all).isChecked = true
        col.addView(card().apply { addView(head("Mod", C_TEXT)); addView(mode) }, cardParams())

        val fake = RadioGroup(this)
        val split = radio("Sadece böl")
        val ttl = radio("Düşük TTL (ilk parça sunucuya varmadan düşer)")
        fake.addView(split); fake.addView(ttl)
        (if (cfg.fake == "ttl") ttl else split).isChecked = true
        val ttlVal = EditText(this).apply {
            setText(cfg.ttl.toString()); setTextColor(C_TEXT); inputType = InputType.TYPE_CLASS_NUMBER
        }
        col.addView(card().apply {
            addView(head("Yöntem", C_TEXT))
            addView(note("Root olmadan bozuk sağlama toplamlı sahte paket gönderilemiyor. Sadece bölme çoğu yerde yetiyor, olmazsa düşük TTL'yi dene."))
            addView(fake)
            addView(note("TTL değeri (1-32, genelde 3-6 arası iyi)"))
            addView(ttlVal)
        }, cardParams())

        val quic = check("QUIC'i engelle (tarayıcı normal bağlantıya döner)", cfg.blockQuic)
        val dns = check("DNS'i Yandex'e yönlendir (77.88.8.8, port 1253)", cfg.dnsRedirect)
        col.addView(card().apply { addView(head("Diğer", C_TEXT)); addView(quic); addView(dns) }, cardParams())

        val apps = listBox(cfg.excludedApps)
        col.addView(card().apply {
            addView(head("VPN'e girmeyecek uygulamalar", C_GREEN))
            addView(note("Bu uygulamaların trafiği GDPI'dan hiç geçmez. Banka uygulamaları baştan ekli. Paket adı yaz ya da listeden seç."))
            addView(apps)
            addView(Button(this@MainActivity).apply {
                text = "Listeden seç"
                isAllCaps = false
                setOnClickListener { pickApp(apps) }
            })
        }, cardParams())

        col.addView(saveButton {
            cfg.onlyTargets = list.isChecked
            cfg.fake = if (ttl.isChecked) "ttl" else "split"
            cfg.ttl = ttlVal.text.toString().toIntOrNull() ?: 4
            cfg.blockQuic = quic.isChecked
            cfg.dnsRedirect = dns.isChecked
            cfg.excludedApps = apps.text.toString().split('\n', ',', ' ').map { it.trim() }.filter { it.isNotEmpty() }.distinct()
        }, cardParams())

        col.addView(note("GDPI ${packageManager.getPackageInfo(packageName, 0).versionName}  ·  github.com/TechnoDiance23/gdpi").apply {
            gravity = Gravity.CENTER
        }, cardParams())
        body.addView(scroll, LinearLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }

    private fun radio(s: String) = RadioButton(this).apply { text = s; setTextColor(C_TEXT); id = View.generateViewId() }
    private fun check(s: String, v: Boolean) = CheckBox(this).apply { text = s; setTextColor(C_TEXT); isChecked = v }

    private fun pickApp(box: EditText) {
        val pm = packageManager
        val launch = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        val apps = pm.queryIntentActivities(launch, 0)
            .map { it.activityInfo.packageName to it.loadLabel(pm).toString() }
            .filter { it.first != packageName }
            .distinctBy { it.first }
            .sortedBy { it.second.lowercase() }
        AlertDialog.Builder(this)
            .setTitle("Uygulama seç")
            .setItems(apps.map { "${it.second}\n${it.first}" }.toTypedArray()) { _, i ->
                val cur = box.text.toString().trimEnd()
                if (apps[i].first !in cur.split('\n')) box.setText(if (cur.isEmpty()) apps[i].first else "$cur\n${apps[i].first}")
            }
            .show()
    }

    companion object {
        val C_HEADER = Color.parseColor("#0b2a6b")
        val C_BG = Color.parseColor("#0f141b")
        val C_SIDE = Color.parseColor("#0a0e13")
        val C_CARD = Color.parseColor("#161d27")
        val C_CARD2 = Color.parseColor("#1c2532")
        val C_TEXT = Color.parseColor("#e6edf6")
        val C_MUTE = Color.parseColor("#8a98ab")
        val C_BLUE = Color.parseColor("#2f80ff")
        val C_GREEN = Color.parseColor("#22c55e")
    }
}
