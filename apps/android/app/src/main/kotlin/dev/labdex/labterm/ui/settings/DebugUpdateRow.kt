package dev.labdex.labterm.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.SystemUpdate
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
import dev.labdex.labterm.BuildConfig
import dev.labdex.labterm.R
import dev.labdex.labterm.data.DebugUpdate
import dev.labdex.labterm.data.DebugUpdater
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.RowDivider
import kotlinx.coroutines.launch

/** "Check for updates" on debug-channel builds; nothing on other builds. */
@Composable
fun DebugUpdateRow(onNotify: (String) -> Unit) {
    if (!DebugUpdater.enabled) return
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var update by remember { mutableStateOf<DebugUpdate?>(null) }
    var busy by remember { mutableStateOf(false) }
    var checked by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        update = runCatching { DebugUpdater.check() }.getOrNull()
        checked = true
    }
    val current = update
    val upToDate = stringResource(R.string.debug_update_current, BuildConfig.DEBUG_BUILD)
    RowDivider()
    ChevronRow(
        title = if (current != null) stringResource(R.string.debug_update_available, current.build) else stringResource(R.string.debug_check_for_updates),
        subtitle = when {
            busy -> stringResource(R.string.debug_update_downloading)
            current != null -> stringResource(R.string.debug_update_tap_to_install, current.commit.take(7))
            checked -> stringResource(R.string.debug_update_current, BuildConfig.DEBUG_BUILD)
            else -> null
        },
        leading = { IconTile(Icons.Filled.SystemUpdate) },
        modifier = Modifier.clickable(enabled = !busy) {
            busy = true
            scope.launch {
                runCatching {
                    val u = current ?: DebugUpdater.check()
                    update = u
                    if (u == null) onNotify(upToDate)
                    else DebugUpdater.install(context, u)
                }.onFailure { onNotify(it.userMessage()) }
                busy = false
                checked = true
            }
        },
    )
}
