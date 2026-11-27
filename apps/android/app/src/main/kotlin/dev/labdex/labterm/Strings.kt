package dev.labdex.labterm

import android.app.Application
import android.content.Context
import androidx.annotation.PluralsRes
import androidx.annotation.StringRes

/**
 * String resources for code that runs outside composition: view models, managers, services and
 * error mapping. Composables use `stringResource` directly.
 */
object Strings {
    private lateinit var app: Application

    fun init(application: Application) {
        app = application
    }

    val context: Context
        get() = app
}

fun str(@StringRes id: Int, vararg args: Any?): String = Strings.context.getString(id, *args)

fun plural(@PluralsRes id: Int, quantity: Int, vararg args: Any?): String =
    Strings.context.resources.getQuantityString(id, quantity, *args)
