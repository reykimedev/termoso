package dev.labdex.labterm.data

import android.os.Process
import dev.labdex.labterm.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Recent log lines of this process for the in-app viewer of debug builds. An app may read
 * its own logcat without a permission, and Rust `tracing` already goes there (tag `termoso`),
 * so this covers Kotlin, Rust and crash traces alike. Lines are redacted before they reach
 * the screen, the clipboard or a share target.
 */
object DebugLog {
    val enabled: Boolean get() = BuildConfig.DEBUG

    suspend fun read(maxLines: Int = 3000): List<String> = withContext(Dispatchers.IO) {
        if (!enabled) return@withContext emptyList()
        val proc = ProcessBuilder(
            "logcat", "-d", "-v", "time", "--pid=${Process.myPid()}", "-t", maxLines.toString(),
        ).redirectErrorStream(true).start()
        try {
            proc.inputStream.bufferedReader().useLines { lines -> lines.map(::redact).toList() }
        } finally {
            proc.destroy()
        }
    }

    // Values that must never leave the device, even from a debug build.
    private val rules: List<Pair<Regex, String>> = listOf(
        Regex("-----BEGIN [A-Z ]*PRIVATE KEY-----.*") to "<private key>",
        Regex("eyJ[A-Za-z0-9_-]{5,}\\.[A-Za-z0-9_-]{5,}\\.[A-Za-z0-9_-]*") to "<jwt>",
        Regex("(?i)\\b(bearer|basic)\\s+[A-Za-z0-9._~+/=-]+") to "$1 <redacted>",
        // OAuth parameters in URLs (redirects, authorize and token requests).
        Regex("(?i)([?&#](?:code|state|code_verifier|code_challenge|nonce|login_hint)=)[^&\\s\"']+") to "$1<redacted>",
        // Secret-looking fields in key=value, key: value and JSON form.
        Regex(
            "(?i)\\b(code_verifier|access_token|id_token|refresh_token|token|client_secret|secret|password|" +
                "passphrase|setup_key|setupkey|private_key|privatekey|psk|otp)" +
                "(\"?\\s*[=:]\\s*\"?)[^\\s&\"',;}]+",
        ) to "$1$2<redacted>",
        // WireGuard keys and other 32-byte base64 values.
        Regex("\\b[A-Za-z0-9+/]{43}=") to "<key>",
    )

    fun redact(line: String): String = rules.fold(line) { acc, (re, with) -> re.replace(acc, with) }
}
