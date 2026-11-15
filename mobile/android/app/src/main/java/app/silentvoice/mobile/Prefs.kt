package app.silentvoice.mobile

import android.app.Application
import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Prefs.init(this)
        History.init(this)
        ModelStore.removeUnused(this)
    }
}

/** Settings. Plain values live in SharedPreferences; the API key is encrypted. */
object Prefs {
    private lateinit var sp: SharedPreferences

    fun init(ctx: Context) {
        sp = ctx.getSharedPreferences("settings", Context.MODE_PRIVATE)
    }

    private fun bool(key: String, default: Boolean) = sp.getBoolean(key, default)
    private fun put(key: String, v: Boolean) = sp.edit().putBoolean(key, v).apply()
    private fun str(key: String, default: String) = sp.getString(key, default) ?: default
    private fun put(key: String, v: String) = sp.edit().putString(key, v).apply()

    var onboarded: Boolean
        get() = bool("onboarded", false)
        set(v) = put("onboarded", v)

    // Clean-up: same steps as desktop, on by default.
    var fillers: Boolean
        get() = bool("fillers", true)
        set(v) = put("fillers", v)
    var repeats: Boolean
        get() = bool("repeats", true)
        set(v) = put("repeats", v)
    var numbers: Boolean
        get() = bool("numbers", true)
        set(v) = put("numbers", v)

    // Writing modes (cloud AI). Off until the user adds a provider.
    var aiOn: Boolean
        get() = bool("ai_on", false)
        set(v) = put("ai_on", v)
    var aiMode: String
        get() = str("ai_mode", "polish")
        set(v) = put("ai_mode", v)
    var aiBaseUrl: String
        get() = str("ai_base_url", "")
        set(v) = put("ai_base_url", v)
    var aiModel: String
        get() = str("ai_model", "")
        set(v) = put("ai_model", v)

    /**
     * The provider key, encrypted with a key that never leaves the phone's
     * hardware-backed Keystore. Desktop keeps keys in plain text; a phone is
     * lost more easily, so this does not.
     */
    var apiKey: String
        get() = runCatching {
            val raw = Base64.decode(str("ai_key", ""), Base64.NO_WRAP)
            if (raw.size <= 12) return@runCatching ""
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, raw, 0, 12))
            String(cipher.doFinal(raw, 12, raw.size - 12))
        }.getOrDefault("")
        set(v) {
            if (v.isEmpty()) {
                sp.edit().remove("ai_key").apply()
                return
            }
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
            put("ai_key", Base64.encodeToString(cipher.iv + cipher.doFinal(v.toByteArray()), Base64.NO_WRAP))
        }

    private fun secretKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey("sv_api_key", null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(
                KeyGenParameterSpec.Builder("sv_api_key", KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .build()
            )
            generateKey()
        }
    }
}
