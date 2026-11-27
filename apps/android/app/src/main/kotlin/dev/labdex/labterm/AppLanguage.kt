package dev.labdex.labterm

import android.app.LocaleManager
import android.content.Context
import android.os.Build
import android.os.LocaleList

/**
 * Labterm ships in English only. Earlier builds had a language picker; this clears what it left
 * behind so a former Russian choice doesn't keep formatting dates and numbers in Russian.
 */
object AppLanguage {
    private const val LEGACY_PREFS = "app_language"

    fun clearLegacyChoice(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val manager = context.getSystemService(LocaleManager::class.java)
            if (!manager.applicationLocales.isEmpty) manager.applicationLocales = LocaleList.getEmptyLocaleList()
        }
        context.deleteSharedPreferences(LEGACY_PREFS)
    }
}
