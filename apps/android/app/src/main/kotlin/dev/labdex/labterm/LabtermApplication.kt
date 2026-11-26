package dev.labdex.labterm

import android.app.Application
import dev.labdex.labterm.data.AppContainer
import dev.labdex.labterm.core.initLogging

class LabtermApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        initLogging(verbose = BuildConfig.DEBUG)
        Strings.init(this)
        container = AppContainer(this)
    }
}
