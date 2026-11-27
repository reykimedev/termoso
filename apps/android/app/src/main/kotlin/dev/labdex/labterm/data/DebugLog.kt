package dev.labdex.labterm.data

import android.content.Context
import android.os.Process
import android.util.Log
import dev.labdex.labterm.BuildConfig
import dev.labdex.labterm.core.DebugLogFile
import dev.labdex.labterm.core.debugLogFiles
import dev.labdex.labterm.core.debugLogRead
import dev.labdex.labterm.core.debugLogRedact
import dev.labdex.labterm.core.logLine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Debug logs for the in-app viewer of debug builds. Two kinds of source:
 *
 * - **Files**: one per launch, written by the Rust core (Rust `tracing` plus the app's own
 *   [AppLog] lines), the last ten kept. They survive a crash or a restart, so the run that
 *   went wrong can still be read.
 * - **Logcat**: this process's recent logcat (an app may read its own without a permission),
 *   which adds system messages and other libraries.
 *
 * Every line is redacted before it reaches the screen, the clipboard or a share target; the
 * files are redacted when they are written, too.
 */
object DebugLog {
    val enabled: Boolean get() = BuildConfig.DEBUG

    /** Source id of the live logcat buffer; every other id is a file name. */
    const val LOGCAT = "logcat"

    fun dir(context: Context): File = File(context.filesDir, "debug-logs")

    /** Log files, newest first (empty on release builds). */
    suspend fun files(): List<DebugLogFile> = withContext(Dispatchers.IO) {
        if (!enabled) return@withContext emptyList()
        runCatching { debugLogFiles() }.getOrDefault(emptyList())
    }

    suspend fun read(source: String, maxLines: Int = 3000): List<String> = withContext(Dispatchers.IO) {
        if (!enabled) return@withContext emptyList()
        if (source == LOGCAT) readLogcat(maxLines) else debugLogRead(source).takeLast(maxLines)
    }

    private fun readLogcat(maxLines: Int): List<String> {
        val proc = ProcessBuilder(
            "logcat", "-d", "-v", "time", "--pid=${Process.myPid()}", "-t", maxLines.toString(),
        ).redirectErrorStream(true).start()
        return try {
            proc.inputStream.bufferedReader().useLines { lines -> lines.map(::redact).toList() }
        } finally {
            proc.destroy()
        }
    }

    // The core's rules, so logcat and the files mask the same things.
    fun redact(line: String): String = debugLogRedact(line)
}

/**
 * Logging for the app's own code: logcat as usual, and the debug log file of this launch, so
 * one file tells the whole story (Kotlin and Rust, with timings). Never pass secrets; lines
 * are redacted anyway.
 */
object AppLog {
    fun d(tag: String, message: String) = line("debug", tag, message) { Log.d(tag, message) }
    fun i(tag: String, message: String) = line("info", tag, message) { Log.i(tag, message) }
    fun w(tag: String, message: String, error: Throwable? = null) =
        line("warn", tag, withError(message, error)) { Log.w(tag, message, error) }
    fun e(tag: String, message: String, error: Throwable? = null) =
        line("error", tag, withError(message, error)) { Log.e(tag, message, error) }

    private inline fun line(level: String, tag: String, message: String, logcat: () -> Unit) {
        logcat()
        // The native library may be missing in JVM unit tests.
        runCatching { logLine(level, tag, message) }
    }

    private fun withError(message: String, error: Throwable?): String =
        if (error == null) message else "$message: ${Log.getStackTraceString(error).trimEnd()}"
}
