package io.github.technodiance23.gdpi

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.view.View
import kotlin.math.min
import kotlin.math.sin

/** Masaüstündeki ortadaki büyük düğme: kapalıyken gri, açıkken yeşil ve nabız gibi parlıyor. */
class PowerButton(ctx: Context) : View(ctx) {
    var on = false
        set(v) { field = v; invalidate() }
    private var phase = 0f
    private val p = Paint(Paint.ANTI_ALIAS_FLAG)

    init {
        isClickable = true
        contentDescription = "Aç / kapat"
    }

    override fun onMeasure(w: Int, h: Int) {
        val s = (220 * resources.displayMetrics.density).toInt()
        setMeasuredDimension(resolveSize(s, w), resolveSize(s, h))
    }

    override fun onDraw(c: Canvas) {
        val cx = width / 2f
        val cy = height / 2f
        val u = min(width, height) / 220f
        p.style = Paint.Style.FILL
        if (on) {
            val pulse = (sin(phase) + 1) / 2
            for ((i, col) in listOf("#0d2a1f", "#10372a", "#134634").withIndex()) {
                p.color = Color.parseColor(col)
                c.drawCircle(cx, cy, (104 - i * 10 + pulse * 4) * u, p)
            }
        } else {
            for ((i, col) in listOf("#121922", "#151d28").withIndex()) {
                p.color = Color.parseColor(col)
                c.drawCircle(cx, cy, (100 - i * 10) * u, p)
            }
        }
        val ring = Color.parseColor(if (on) "#22c55e" else "#334155")
        val core = Color.parseColor(if (on) "#0f2a1f" else "#111823")
        val icon = Color.parseColor(if (on) "#22c55e" else "#94a3b8")
        p.color = core
        c.drawCircle(cx, cy, 70 * u, p)
        p.style = Paint.Style.STROKE
        p.strokeWidth = 4 * u
        p.color = ring
        c.drawCircle(cx, cy, 70 * u, p)
        p.strokeWidth = 7 * u
        p.strokeCap = Paint.Cap.ROUND
        p.color = icon
        c.drawArc(RectF(cx - 30 * u, cy - 30 * u, cx + 30 * u, cy + 30 * u), -60f, 300f, false, p)
        c.drawLine(cx, cy - 40 * u, cx, cy - 6 * u, p)
        if (on) {
            phase = (phase + 0.08f) % (2 * Math.PI.toFloat())
            postInvalidateDelayed(40)
        }
    }
}
