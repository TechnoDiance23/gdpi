package io.github.technodiance23.gdpi

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.VpnService
import android.os.Process
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.File

/**
 * Root gerektirmeyen motor: VpnService bir tun arayüzü açar, hev-socks5-tunnel bu arayüzdeki
 * trafiği yerel SOCKS5 sunucumuza (Socks) taşır, Socks da gerçek bağlantıları korumalı
 * (protect) soketlerle açıp ilk yükü bölerek yazar.
 */
class GdpiVpnService : VpnService() {
    private var tun: ParcelFileDescriptor? = null
    private var socks: Socks? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopEngine()
            stopSelf()
            return START_NOT_STICKY
        }
        startForegroundCompat()
        if (tun == null) {
            try {
                startEngine()
            } catch (e: Exception) {
                Log.e("GDPI", "başlatılamadı", e)
                lastError = e.message ?: e.toString()
                stopEngine()
                stopSelf()
            }
        }
        return START_STICKY
    }

    private fun startEngine() {
        if (!TProxyService.available) throw IllegalStateException("Yerel kütüphane eksik (hev-socks5-tunnel).")
        val cfg = Config(this)
        val s = Socks(
            protect = { protect(it) },
            protectUdp = { protect(it) },
            ownPeer = { proto, from, to ->
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) true
                else try {
                    val uid = getSystemService(ConnectivityManager::class.java).getConnectionOwnerUid(proto, from, to)
                    // UDP'de bağlantısız soketler için Android çoğu zaman sahibi bulamaz (-1).
                    // Öyle durumda sadece başka bir uygulamaya ait olduğu kesinse reddediyoruz,
                    // yoksa DNS paketleri düşüyor ve hiçbir site açılmıyor.
                    uid == Process.myUid() || (proto == 17 && uid == Process.INVALID_UID)
                } catch (_: Exception) { proto == 17 }
            },
            cfg = Socks.Snapshot(
                onlyTargets = cfg.onlyTargets, ttlMode = cfg.fake == "ttl", fakeMode = cfg.fake == "fake", ttl = cfg.ttl,
                blockQuic = cfg.blockQuic, dnsRedirect = cfg.dnsRedirect,
                targets = cfg.targets, bypass = cfg.bypass,
            ),
        )
        s.start()
        socks = s

        val b = Builder()
            .setSession("GDPI")
            .setMtu(MTU)
            .addAddress("10.10.10.10", 32)
            .addRoute("0.0.0.0", 0)
            .addDnsServer("1.1.1.1")
        try {
            b.addAddress("fd00:6764:7069::1", 128).addRoute("::", 0)
        } catch (_: Exception) {
        }
        // GDPI'ın kendi soketleri tünele girmesin (protect'e ek güvence)
        b.addDisallowedApplication(packageName)
        for (pkg in cfg.excludedApps) {
            try {
                b.addDisallowedApplication(pkg)
            } catch (_: PackageManager.NameNotFoundException) {
            }
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) b.setMetered(false)
        b.setConfigureIntent(
            PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        )
        val fd = b.establish() ?: throw IllegalStateException("VPN izni yok.")
        tun = fd

        val conf = File(noBackupFilesDir, "tunnel.yml")
        conf.writeText(
            """
            tunnel:
              mtu: $MTU
              ipv4: 198.18.0.1
              ipv6: 'fc00::1'
            socks5:
              port: ${s.port}
              address: 127.0.0.1
              udp: 'udp'
              username: '${s.user}'
              password: '${s.pass}'
            misc:
              task-stack-size: 81920
              log-level: warn
            """.trimIndent()
        )
        if (!TProxyService.TProxyStartService(conf.absolutePath, fd.fd)) {
            throw IllegalStateException("Tünel başlatılamadı.")
        }
        running = true
        startedAt = System.currentTimeMillis()
        lastError = null
        instance = this
    }

    private fun stopEngine() {
        if (TProxyService.available) {
            try { TProxyService.TProxyStopService() } catch (_: Throwable) {}
        }
        socks?.stop(); socks = null
        try { tun?.close() } catch (_: Exception) {}
        tun = null
        running = false
        instance = null
        stopForeground(STOP_FOREGROUND_REMOVE)
    }

    override fun onRevoke() {               // başka bir VPN açıldı ya da izin geri alındı
        stopEngine()
        stopSelf()
    }

    override fun onDestroy() {
        stopEngine()
        super.onDestroy()
    }

    private fun startForegroundCompat() {
        val nm = getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL, "GDPI çalışıyor", NotificationManager.IMPORTANCE_LOW)
            )
        }
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(
            this, 1, Intent(this, GdpiVpnService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        @Suppress("DEPRECATION")
        val nb = if (Build.VERSION.SDK_INT >= 26) Notification.Builder(this, CHANNEL) else Notification.Builder(this)
        val n = nb.setSmallIcon(R.drawable.ic_notif)
            .setContentTitle("GDPI açık")
            .setContentText("Engeller aşılıyor. Bankalara ve devlet sitelerine dokunulmuyor.")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Kapat", stop).build())
            .build()
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(NOTIF_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIF_ID, n)
        }
    }

    companion object {
        const val ACTION_STOP = "io.github.technodiance23.gdpi.STOP"
        private const val CHANNEL = "gdpi"
        private const val NOTIF_ID = 1
        private const val MTU = 8500

        @Volatile var running = false
        @Volatile var startedAt = 0L
        @Volatile var lastError: String? = null
        @Volatile private var instance: GdpiVpnService? = null

        val nSplit: Int get() = instance?.socks?.nSplit?.get() ?: 0
        val nSkip: Int get() = instance?.socks?.nSkip?.get() ?: 0

        fun start(ctx: Context) {
            val i = Intent(ctx, GdpiVpnService::class.java)
            if (Build.VERSION.SDK_INT >= 26) ctx.startForegroundService(i) else ctx.startService(i)
        }

        fun stop(ctx: Context) {
            ctx.startService(Intent(ctx, GdpiVpnService::class.java).setAction(ACTION_STOP))
        }
    }
}
