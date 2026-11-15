package app.silentvoice.mobile

import android.os.Bundle
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
        setContent {
            SilentVoiceTheme {
                var onboarded by remember { mutableStateOf(Prefs.onboarded) }
                if (onboarded) MainScreen()
                else Onboarding(onDone = { Prefs.onboarded = true; onboarded = true })
            }
        }
    }
}
