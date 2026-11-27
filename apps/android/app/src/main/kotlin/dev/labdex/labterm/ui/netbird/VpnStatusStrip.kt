package dev.labdex.labterm.ui.netbird

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.VpnLock
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportActive
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.hosts.vpnProviderName
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flow

/** The VPN networks in use on this device (labterm-client `vpn::active`), for the status strip. */
object VpnActivity {
    /** The networks in use, read again every [everyMs] while collected. Cheap: in memory. */
    fun observe(repo: VaultRepository, everyMs: Long = 2_000): Flow<List<TransportActive>> = flow {
        while (true) {
            emit(runCatching { repo.read { transportActive() } }.getOrDefault(emptyList()))
            delay(everyMs)
        }
    }.distinctUntilChanged()
}

/** "1:05" for a countdown. */
internal fun formatCountdown(secs: ULong): String {
    val s = secs.toLong()
    return "%d:%02d".format(s / 60, s % 60)
}

/**
 * The status row above the navigation bar: [leading] (other status chips), then one chip per
 * VPN network in use. Draws nothing while there is nothing to show.
 */
@Composable
fun VpnStatusStrip(
    repo: VaultRepository,
    onOpen: (String) -> Unit,
    modifier: Modifier = Modifier,
    leading: (@Composable RowScope.() -> Unit)? = null,
) {
    val flow = remember(repo) { VpnActivity.observe(repo) }
    val networks by flow.collectAsStateWithLifecycle(emptyList())
    if (networks.isEmpty() && leading == null) return
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = modifier.fillMaxWidth()) {
        Row(
            Modifier
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            leading?.invoke(this)
            networks.forEach { VpnStatusChip(it, onClick = { onOpen(it.id) }) }
        }
    }
}

/** One network in the strip: icon, short label, state dot and the countdown to its idle stop. */
@Composable
fun VpnStatusChip(network: TransportActive, onClick: () -> Unit) {
    val color = network.state.color()
    val label = network.label.ifEmpty { vpnProviderName(network.provider) }
    val left = network.stopsInSecs
    val description = buildString {
        append(vpnProviderName(network.provider)).append(' ').append(label).append(": ").append(network.state.label())
        if (left != null) append(", ").append(str(R.string.vpn_strip_stops_in, formatCountdown(left)))
    }
    val shape = RoundedCornerShape(999.dp)
    Row(
        Modifier
            .height(24.dp)
            .clip(shape)
            .background(color.copy(alpha = 0.14f))
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp)
            .semantics { contentDescription = description },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Icon(Icons.Filled.VpnLock, contentDescription = null, tint = color, modifier = Modifier.size(12.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 120.dp),
        )
        Box(Modifier.size(6.dp).clip(CircleShape).background(color))
        if (left != null) {
            Text(formatCountdown(left), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * Shows a notice with "Reconnect" each time a network stopped after being unused. Starts at
 * the newest stop, so stops from before the app opened are not shown.
 */
@Composable
fun VpnIdleStopNotices(repo: VaultRepository, snackbar: SnackbarHostState, everyMs: Long = 2_000) {
    LaunchedEffect(repo) {
        var seq = runCatching { repo.read { transportIdleStopSeq() } }.getOrDefault(0uL)
        while (true) {
            delay(everyMs)
            val stops = runCatching { repo.read { transportIdleStopsSince(seq) } }.getOrDefault(emptyList())
            for (stop in stops) {
                seq = maxOf(seq, stop.seq)
                val name = "${vpnProviderName(stop.provider)} \"${stop.label.ifEmpty { vpnProviderName(stop.provider) }}\""
                val result = snackbar.showSnackbar(
                    str(R.string.vpn_idle_stopped, name),
                    actionLabel = str(R.string.reconnect),
                    duration = SnackbarDuration.Long,
                )
                if (result == SnackbarResult.ActionPerformed) {
                    runCatching { repo.read { transportConnect(stop.id) } }
                }
            }
        }
    }
}
