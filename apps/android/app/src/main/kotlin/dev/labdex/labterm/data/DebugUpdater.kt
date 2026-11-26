package dev.labdex.labterm.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import dev.labdex.labterm.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/** A newer build on the debug channel, as described by its latest.json. */
data class DebugUpdate(val build: Int, val commit: String, val apkUrl: String, val sha256: String)

/**
 * In-app updates for debug-channel builds (fork CI publishes each build to a rolling
 * prerelease). Off unless the build was made with `termoso.debugUpdateUrl`; release
 * builds update through their store. Android itself refuses an APK signed with another key.
 */
object DebugUpdater {
    val enabled: Boolean get() = BuildConfig.DEBUG && BuildConfig.DEBUG_UPDATE_URL.startsWith("https://")

    /** The newer build, or null when this one is current or the channel is off. */
    suspend fun check(): DebugUpdate? = withContext(Dispatchers.IO) {
        if (!enabled) return@withContext null
        val base = URL(BuildConfig.DEBUG_UPDATE_URL)
        val json = JSONObject(fetch(base).use { it.readBytes() }.decodeToString())
        val update = DebugUpdate(
            build = json.getInt("build"),
            commit = json.optString("commit"),
            apkUrl = URL(base, json.getString("apk")).toString(),
            sha256 = json.getString("sha256").lowercase(),
        )
        update.takeIf { it.build > BuildConfig.DEBUG_BUILD && it.apkUrl.startsWith("https://") }
    }

    /** Download and verify the APK, then hand it to the system installer. */
    suspend fun install(context: Context, update: DebugUpdate) {
        val apk = withContext(Dispatchers.IO) {
            val dir = File(context.cacheDir, "updates").apply { mkdirs() }
            val file = File(dir, "termoso-debug.apk")
            val digest = MessageDigest.getInstance("SHA-256")
            fetch(URL(update.apkUrl)).use { input ->
                file.outputStream().use { out ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        digest.update(buf, 0, n)
                        out.write(buf, 0, n)
                    }
                }
            }
            val got = digest.digest().joinToString("") { "%02x".format(it) }
            if (got != update.sha256) {
                file.delete()
                error("the downloaded update does not match its checksum")
            }
            file
        }
        if (!context.packageManager.canRequestPackageInstalls()) {
            // First time: the user allows this app to install updates, then taps Update again.
            context.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            return
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", apk)
        context.startActivity(
            Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }

    private fun fetch(url: URL) = (url.openConnection() as HttpURLConnection).run {
        connectTimeout = 15_000
        readTimeout = 60_000
        instanceFollowRedirects = true
        if (responseCode !in 200..299) error("update server answered $responseCode")
        inputStream
    }
}
