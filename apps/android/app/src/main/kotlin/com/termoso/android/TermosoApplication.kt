package com.termoso.android

import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import com.termoso.android.data.AppContainer
import com.termoso.core.UrlOpener
import com.termoso.core.initLogging
import com.termoso.core.setUrlOpener

class TermosoApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        initLogging(verbose = BuildConfig.DEBUG)
        Strings.init(this)
        container = AppContainer(this)
        setUrlOpener(BrowserOpener(this))
    }
}

/** Opens OpenID sign-in pages in the default browser; called from Rust during connect. */
private class BrowserOpener(private val app: Application) : UrlOpener {
    override fun openUrl(url: String): Boolean = try {
        app.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    } catch (_: ActivityNotFoundException) {
        false
    }
}
