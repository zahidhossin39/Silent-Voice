package app.silentvoice.mobile

import android.annotation.SuppressLint
import android.app.AppOpsManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.os.Process
import android.os.SystemClock
import android.provider.Settings

/**
 * Whether the bubble can actually run, and what keeps it running.
 *
 * Why this exists: Xiaomi (MIUI / HyperOS) kills the whole app when it is
 * swiped away from Recents ("SwipeUpClean" in the phone's exit log). Android
 * then marks the accessibility service as crashed and does not start it again
 * unless the app may auto-start. The Accessibility switch still reads "on", so
 * the switch alone cannot tell "on" from "on but dead".
 */
object Keepalive {
    enum class Bubble {
        /** Switched off in Accessibility. */
        Off,
        /** Switched on and running: the bubble will appear. */
        On,
        /** Switched on, but the phone stopped it. Needs off-and-on again. */
        Stopped,
    }

    fun bubble(ctx: Context): Bubble {
        val switched = Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES)
            .orEmpty().split(':')
            .any { ComponentName.unflattenFromString(it)?.packageName == ctx.packageName }
        return when {
            !switched -> Bubble.Off
            // The service runs in this process, so this is exact: after the
            // phone kills the app, a fresh process starts with it unset.
            BubbleService.running -> Bubble.On
            // On a cold start Android binds the service about a second after
            // the app opens; don't call it stopped during that window.
            SystemClock.elapsedRealtime() - Process.getStartElapsedRealtime() < 2500 -> Bubble.On
            else -> Bubble.Stopped
        }
    }

    /** Xiaomi, Redmi and POCO phones, which need their own Autostart permission. */
    val isXiaomi: Boolean
        get() = Build.MANUFACTURER.equals("Xiaomi", true) ||
            Build.BRAND.lowercase().let { it == "xiaomi" || it == "redmi" || it == "poco" }

    /** Android battery saver leaves the app alone. */
    fun batteryFree(ctx: Context): Boolean =
        ctx.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(ctx.packageName)

    /**
     * Xiaomi's Autostart permission (app-op 10008). Null when it can't be read;
     * the method is hidden, so a later system update may block the read.
     */
    @SuppressLint("DiscouragedPrivateApi")
    fun autostart(ctx: Context): Boolean? {
        if (!isXiaomi) return null
        return try {
            val ops = ctx.getSystemService(AppOpsManager::class.java)
            val m = AppOpsManager::class.java.getMethod(
                "checkOpNoThrow", Int::class.javaPrimitiveType, Int::class.javaPrimitiveType, String::class.java,
            )
            (m.invoke(ops, 10008, Process.myUid(), ctx.packageName) as Int) == AppOpsManager.MODE_ALLOWED
        } catch (e: Throwable) {
            null
        }
    }

    /**
     * True when nothing more can be done to keep the bubble alive. On Xiaomi an
     * unreadable Autostart counts as not done, so the reminder stays.
     */
    fun protected(ctx: Context): Boolean =
        batteryFree(ctx) && (!isXiaomi || autostart(ctx) == true)

    @SuppressLint("BatteryLife") // Sideloaded dictation app; the user asks for it.
    fun askBattery(ctx: Context) {
        val direct = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${ctx.packageName}"))
        if (!start(ctx, direct)) start(ctx, Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
    }

    /** Opens Xiaomi's Autostart list; falls back to App info. */
    fun openAutostart(ctx: Context) {
        val miui = Intent().setComponent(
            ComponentName("com.miui.securitycenter", "com.miui.permcenter.autostart.AutoStartManagementActivity"),
        )
        if (!start(ctx, miui)) {
            start(ctx, Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}")))
        }
    }

    private fun start(ctx: Context, intent: Intent): Boolean = try {
        ctx.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (e: Exception) {
        false
    }
}
