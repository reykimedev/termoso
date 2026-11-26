package dev.labdex.labterm.ui.settings

import android.text.format.Formatter
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.SystemUpdate
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.BuildConfig
import dev.labdex.labterm.R
import dev.labdex.labterm.data.DebugUpdate
import dev.labdex.labterm.data.DebugUpdater
import dev.labdex.labterm.data.DownloadProgress
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.RowDivider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** "Check for updates" on debug-channel builds; nothing on other builds. */
@Composable
fun DebugUpdateRow(onNotify: (String) -> Unit) {
    if (!DebugUpdater.enabled) return
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var update by remember { mutableStateOf<DebugUpdate?>(null) }
    var busy by remember { mutableStateOf(false) }
    var checked by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<DownloadProgress?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        runCatching { DebugUpdater.check() }
            .onSuccess { update = it }
            .onFailure { failure = it.userMessage() }
        checked = true
    }
    val current = update
    val upToDate = stringResource(R.string.debug_update_current, BuildConfig.DEBUG_BUILD)
    val p = progress
    fun size(bytes: Long) = Formatter.formatShortFileSize(context, bytes)
    RowDivider()
    ChevronRow(
        title = if (current != null) {
            stringResource(R.string.debug_update_available, current.build) +
                (current.size?.let { " · " + size(it) } ?: "")
        } else {
            stringResource(R.string.debug_check_for_updates)
        },
        subtitle = when {
            busy && p != null && p.total != null -> stringResource(
                R.string.debug_update_progress,
                size(p.bytes), size(p.total), size(p.bytesPerSecond), (p.bytes * 100 / p.total).toInt(),
            )
            busy && p != null -> stringResource(R.string.debug_update_progress_unknown, size(p.bytes), size(p.bytesPerSecond))
            busy -> stringResource(R.string.debug_update_downloading)
            failure != null -> failure
            current != null -> stringResource(R.string.debug_update_tap_to_install, current.commit.take(7))
            checked -> upToDate
            else -> null
        },
        leading = { IconTile(Icons.Filled.SystemUpdate) },
        modifier = Modifier.clickable(enabled = !busy) {
            busy = true
            failure = null
            progress = null
            scope.launch {
                runCatching {
                    val u = current ?: DebugUpdater.check()
                    update = u
                    if (u == null) {
                        onNotify(upToDate)
                    } else {
                        DebugUpdater.install(context, u) { next ->
                            // Called on the download thread.
                            scope.launch(Dispatchers.Main) { progress = next }
                        }
                    }
                }.onFailure {
                    failure = it.userMessage()
                    onNotify(it.userMessage())
                }
                withContext(Dispatchers.Main) {
                    busy = false
                    progress = null
                    checked = true
                }
            }
        },
    )
    if (busy && p != null) {
        val total = p.total
        if (total != null && total > 0) {
            LinearProgressIndicator(
                progress = { (p.bytes.toFloat() / total).coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
            )
        } else {
            LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp))
        }
    }
}
