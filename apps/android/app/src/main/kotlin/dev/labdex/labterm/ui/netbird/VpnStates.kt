package dev.labdex.labterm.ui.netbird

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportState
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.str
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flow

/**
 * Each VPN network's state on this device: one model for the networks list, the
 * details page and the host badges (labterm-client `vpn::state`).
 */
object VpnStates {
    /** The states of [ids], read again every [everyMs] while collected. */
    fun observe(repo: VaultRepository, ids: Collection<String>, everyMs: Long = 3_000): Flow<Map<String, TransportState>> = flow {
        val wanted = ids.toSet()
        while (true) {
            if (wanted.isNotEmpty()) {
                emit(
                    repo.read {
                        wanted.associateWith { id -> runCatching { transportState(id) }.getOrDefault(TransportState.Off) }
                    },
                )
            } else {
                emit(emptyMap())
            }
            delay(everyMs)
        }
    }.distinctUntilChanged()
}

/** Short label of a state ("Connected", "Sign-in needed", ...). */
fun TransportState.label(): String = when (this) {
    is TransportState.Off -> str(R.string.vpn_status_off)
    is TransportState.Connecting -> str(R.string.netbird_connecting)
    is TransportState.Connected -> str(R.string.netbird_connected)
    is TransportState.NeedsSignIn -> str(R.string.vpn_status_sign_in)
    is TransportState.Error -> str(R.string.vpn_status_error)
    is TransportState.Unavailable -> str(R.string.vpn_status_unavailable)
}

/** Colour of a state: green connected, amber connecting or sign-in, red error, grey off. */
@Composable
fun TransportState.color(): Color = when (this) {
    is TransportState.Connected -> Color(0xFF2E7D32)
    is TransportState.Connecting, is TransportState.NeedsSignIn -> Color(0xFFF9A825)
    is TransportState.Error -> MaterialTheme.colorScheme.error
    else -> MaterialTheme.colorScheme.outline
}
