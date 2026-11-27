package dev.labdex.labterm

import android.app.Activity
import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.browser.customtabs.CustomTabsIntent
import dev.labdex.labterm.data.AppContainer
import dev.labdex.labterm.core.UrlOpener
import dev.labdex.labterm.core.initLoggingWithFiles
import dev.labdex.labterm.data.AppLog
import dev.labdex.labterm.data.DebugLog
import dev.labdex.labterm.core.setUrlOpener
import java.lang.ref.WeakReference

class LabtermApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        // Debug builds also keep one log file per launch for the in-app viewer.
        initLoggingWithFiles(
            verbose = BuildConfig.DEBUG,
            logDir = if (DebugLog.enabled) DebugLog.dir(this).path else null,
        )
        if (DebugLog.enabled) {
            AppLog.i(
                "Labterm",
                "start: ${BuildConfig.APPLICATION_ID} ${BuildConfig.VERSION_NAME} build ${BuildConfig.DEBUG_BUILD}, " +
                    "Android ${android.os.Build.VERSION.RELEASE} (API ${android.os.Build.VERSION.SDK_INT}), " +
                    "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}",
            )
            // A crash lands in the file before the process dies, so the next launch can show it.
            val previous = Thread.getDefaultUncaughtExceptionHandler()
            Thread.setDefaultUncaughtExceptionHandler { thread, error ->
                AppLog.e("Labterm", "crash on thread ${thread.name}", error)
                previous?.uncaughtException(thread, error)
            }
        }
        Strings.init(this)
        container = AppContainer(this)
        setUrlOpener(BrowserOpener(this))
    }
}

/**
 * Opens OpenID sign-in pages in an in-app browser tab (Custom Tabs) on top of the app, so
 * bringing the app back after the redirect closes the tab. Called from Rust during connect.
 */
private class BrowserOpener(private val app: Application) : UrlOpener, Application.ActivityLifecycleCallbacks {
    @Volatile private var resumed: WeakReference<Activity>? = null

    init {
        app.registerActivityLifecycleCallbacks(this)
    }

    override fun openUrl(url: String): Boolean {
        val uri = Uri.parse(url)
        val activity = resumed?.get()
        return try {
            if (activity != null) {
                // Launched from the activity, the tab joins the app's task.
                activity.runOnUiThread { CustomTabsIntent.Builder().build().launchUrl(activity, uri) }
            } else {
                app.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            }
            true
        } catch (_: ActivityNotFoundException) {
            false
        }
    }

    override fun closeBrowser() {
        // MainActivity is singleTask: starting it clears the tab above it in the task.
        runCatching {
            app.startActivity(
                Intent(app, MainActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
            )
        }
    }

    override fun onActivityResumed(activity: Activity) {
        if (activity is MainActivity) resumed = WeakReference(activity)
    }

    override fun onActivityDestroyed(activity: Activity) {
        if (resumed?.get() === activity) resumed = null
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}
    override fun onActivityStarted(activity: Activity) {}
    override fun onActivityPaused(activity: Activity) {}
    override fun onActivityStopped(activity: Activity) {}
    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
}
