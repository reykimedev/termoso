package dev.labdex.labterm.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.SystemClock
import android.provider.Settings
import androidx.core.content.FileProvider
import dev.labdex.labterm.BuildConfig
import dev.labdex.labterm.R
import dev.labdex.labterm.str
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/** A newer build on the debug channel, as described by its latest.json. */
data class DebugUpdate(
    val build: Int,
    val commit: String,
    val apkUrl: String,
    val sha256: String,
    /** Bytes, when latest.json says (builds published after the field was added). */
    val size: Long? = null,
)

/** Download state for the progress row. */
data class DownloadProgress(val bytes: Long, val total: Long?, val bytesPerSecond: Long)

/** A failed update step, worded for the user; the details go to the debug log. */
class UpdateException(message: String, cause: Throwable? = null) : Exception(message, cause)

/**
 * In-app updates for debug-channel builds (fork CI publishes each build to a rolling
 * prerelease). Off unless the build was made with `labterm.debugUpdateUrl`; release
 * builds update through their store. Android itself refuses an APK signed with another key.
 */
object DebugUpdater {
    val enabled: Boolean get() = BuildConfig.DEBUG && BuildConfig.DEBUG_UPDATE_URL.startsWith("https://")

    /** The newer build, or null when this one is current or the channel is off. */
    suspend fun check(): DebugUpdate? = withContext(Dispatchers.IO) {
        if (!enabled) return@withContext null
        val base = URL(BuildConfig.DEBUG_UPDATE_URL)
        val started = SystemClock.elapsedRealtime()
        AppLog.i(TAG, "check: $base (installed build ${BuildConfig.DEBUG_BUILD})")
        val body = try {
            fetch(base).use { (_, input) -> input.readBytes() }.decodeToString()
        } catch (e: IOException) {
            AppLog.w(TAG, "check failed", e)
            throw UpdateException(str(R.string.debug_update_error_network), e)
        }
        val update = try {
            val json = JSONObject(body)
            DebugUpdate(
                build = json.getInt("build"),
                commit = json.optString("commit"),
                apkUrl = URL(base, json.getString("apk")).toString(),
                sha256 = json.getString("sha256").lowercase(),
                size = json.optLong("size", -1).takeIf { it > 0 },
            )
        } catch (e: Exception) {
            AppLog.w(TAG, "check: latest.json is not readable: ${body.take(200)}", e)
            throw UpdateException(str(R.string.debug_update_error_feed), e)
        }
        AppLog.i(
            TAG,
            "check: channel has build ${update.build} (${update.commit.take(7)}), size ${update.size ?: "unknown"}, " +
                "${SystemClock.elapsedRealtime() - started} ms",
        )
        update.takeIf { it.build > BuildConfig.DEBUG_BUILD && it.apkUrl.startsWith("https://") }
    }

    /** Download and verify the APK, then hand it to the system installer. */
    suspend fun install(
        context: Context,
        update: DebugUpdate,
        onProgress: (DownloadProgress) -> Unit = {},
    ) {
        val apk = withContext(Dispatchers.IO) {
            val dir = File(context.cacheDir, "updates").apply { mkdirs() }
            val file = File(dir, "labterm-debug.apk")
            val digest = MessageDigest.getInstance("SHA-256")
            val started = SystemClock.elapsedRealtime()
            var bytes = 0L
            AppLog.i(TAG, "download: build ${update.build} from ${update.apkUrl}")
            try {
                fetch(URL(update.apkUrl)).use { (conn, input) ->
                    val total = conn.contentLengthLong.takeIf { it > 0 } ?: update.size
                    var lastReport = 0L
                    file.outputStream().use { out ->
                        val buf = ByteArray(64 * 1024)
                        while (true) {
                            val n = input.read(buf)
                            if (n < 0) break
                            digest.update(buf, 0, n)
                            out.write(buf, 0, n)
                            bytes += n
                            val now = SystemClock.elapsedRealtime()
                            if (now - lastReport >= 200) {
                                lastReport = now
                                onProgress(DownloadProgress(bytes, total, speed(bytes, now - started)))
                            }
                        }
                    }
                    if (total != null && bytes != total) {
                        throw IOException("got $bytes of $total bytes")
                    }
                    onProgress(DownloadProgress(bytes, total, speed(bytes, SystemClock.elapsedRealtime() - started)))
                }
            } catch (e: IOException) {
                file.delete()
                AppLog.w(TAG, "download failed after $bytes bytes", e)
                throw UpdateException(str(R.string.debug_update_error_download), e)
            }
            val ms = SystemClock.elapsedRealtime() - started
            AppLog.i(TAG, "download: $bytes bytes in $ms ms (${speed(bytes, ms) / 1024} KiB/s)")
            val got = digest.digest().joinToString("") { "%02x".format(it) }
            if (got != update.sha256) {
                file.delete()
                AppLog.w(TAG, "verify: SHA-256 mismatch, expected ${update.sha256}, got $got")
                throw UpdateException(str(R.string.debug_update_error_checksum))
            }
            AppLog.i(TAG, "verify: SHA-256 matches ($got)")
            file
        }
        if (!context.packageManager.canRequestPackageInstalls()) {
            // First time: the user allows this app to install updates, then taps Update again.
            AppLog.i(TAG, "install: asking for permission to install updates")
            context.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            return
        }
        AppLog.i(TAG, "install: handing build ${update.build} to the system installer")
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", apk)
        context.startActivity(
            Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }

    private fun speed(bytes: Long, ms: Long): Long = if (ms <= 0) 0 else bytes * 1000 / ms

    private class Fetched(val conn: HttpURLConnection, val input: java.io.InputStream) : java.io.Closeable {
        operator fun component1() = conn
        operator fun component2() = input
        override fun close() {
            input.close()
            conn.disconnect()
        }
    }

    private fun fetch(url: URL): Fetched {
        val conn = url.openConnection() as HttpURLConnection
        conn.connectTimeout = 15_000
        conn.readTimeout = 60_000
        conn.instanceFollowRedirects = true
        // latest.json is rewritten in place; never answer from a cache.
        conn.useCaches = false
        conn.setRequestProperty("Cache-Control", "no-cache")
        val code = conn.responseCode
        if (code !in 200..299) {
            conn.disconnect()
            AppLog.w(TAG, "GET $url answered HTTP $code")
            throw UpdateException(str(R.string.debug_update_error_http, code))
        }
        return Fetched(conn, conn.inputStream)
    }

    private const val TAG = "LabtermUpdate"
}
