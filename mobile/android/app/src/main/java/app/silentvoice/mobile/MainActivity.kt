package app.silentvoice.mobile

import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.silentvoice.mobile.ui.MainScreen
import app.silentvoice.mobile.ui.Onboarding
import app.silentvoice.mobile.ui.SilentVoiceTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        selfTest(intent)
        setContent {
            SilentVoiceTheme {
                var onboarded by remember { mutableStateOf(Prefs.onboarded) }
                if (onboarded) MainScreen()
                else Onboarding(onDone = { Prefs.onboarded = true; onboarded = true })
            }
        }
    }

    // The self-test intent can arrive while the app is already open.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        selfTest(intent)
    }

    /**
     * Checks a build from a PC without speaking (see mobile/android/README.md):
     * logs what the app believes about the bubble, then runs the speech self-test.
     */
    private fun selfTest(intent: Intent?) {
        if (intent?.getBooleanExtra("selftest", false) != true) return
        // Read after the cold-start grace in Keepalive.bubble().
        window.decorView.postDelayed({
            Log.i(
                "SVKeepalive",
                "bubble=${Keepalive.bubble(this)} battery=${Keepalive.batteryFree(this)} " +
                    "xiaomi=${Keepalive.isXiaomi} autostart=${Keepalive.autostart(this)}",
            )
        }, 5000)
        Dictation.selfTest(this)
    }
}
