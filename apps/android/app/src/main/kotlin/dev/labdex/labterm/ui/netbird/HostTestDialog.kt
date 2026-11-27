package dev.labdex.labterm.ui.netbird

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Error
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportHost
import dev.labdex.labterm.core.TransportHostTest
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str

/**
 * The connection test for one host: join its VPN network, look the name up, dial
 * through the network and read the SSH banner. No login, nothing sent to the host.
 */
@Composable
fun HostTestDialog(host: TransportHost, repo: VaultRepository, onDismiss: () -> Unit) {
    var result by remember { mutableStateOf<Result<TransportHostTest>?>(null) }
    var run by remember { mutableIntStateOf(0) }
    LaunchedEffect(host.id, run) {
        result = null
        result = runCatching { repo.read { transportTestHost(host.id) } }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.vpn_test_title, host.label.ifEmpty { host.address })) },
        text = {
            val r = result
            when {
                r == null -> Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                    CircularProgressIndicator()
                }
                r.isFailure -> Text(r.exceptionOrNull()!!.userMessage(), color = MaterialTheme.colorScheme.error)
                else -> Steps(r.getOrThrow(), host.address)
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.close)) } },
        dismissButton = {
            TextButton(enabled = result != null, onClick = { run++ }) { Text(stringResource(R.string.vpn_test_again)) }
        },
    )
}

@Composable
private fun Steps(t: TransportHostTest, address: String) {
    val failed = t.networkError != null
    val resolved = !failed && t.remote.isNotEmpty()
    val via = when (t.via) {
        "address" -> str(R.string.vpn_test_via_address)
        "nameserver" -> str(R.string.vpn_test_via_nameserver, t.server)
        "tailscale" -> str(R.string.vpn_test_via_tailscale)
        else -> str(R.string.vpn_test_via_netbird)
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Step(
            ok = !failed,
            title = str(R.string.vpn_test_network, t.network),
            detail = t.networkError ?: if (t.startMs > 0u) {
                str(R.string.vpn_test_joined_in, t.startMs.toLong())
            } else {
                str(R.string.vpn_test_already_connected)
            },
        )
        if (!failed) {
            Step(
                ok = resolved,
                title = str(R.string.vpn_test_lookup),
                detail = if (resolved) {
                    "${t.name.ifEmpty { address }} → ${t.remote.substringBeforeLast(':')} ($via)"
                } else {
                    t.error.orEmpty()
                },
            )
        }
        if (resolved) {
            val p = t.peer
            val path = if (p == null) {
                str(R.string.vpn_test_no_path)
            } else {
                listOfNotNull(
                    t.route?.let { str(R.string.vpn_test_route, it, p.fqdn.ifEmpty { p.ip }) } ?: p.fqdn.ifEmpty { p.ip },
                    str(if (p.relayed) R.string.vpn_peer_relayed else R.string.vpn_peer_direct),
                    if (p.latencyMs > 0u) "${p.latencyMs} ms" else null,
                ).joinToString(" · ")
            }
            Step(ok = p != null, title = str(R.string.vpn_test_path), detail = path)
            Step(
                ok = t.banner.isNotEmpty(),
                title = str(R.string.vpn_test_ssh),
                detail = if (t.banner.isNotEmpty()) {
                    t.banner + " · " + str(R.string.vpn_test_connected_in, t.connectMs.toLong())
                } else {
                    t.error.orEmpty()
                },
            )
        }
    }
}

@Composable
private fun Step(ok: Boolean, title: String, detail: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Top) {
        Icon(
            if (ok) Icons.Filled.CheckCircle else Icons.Filled.Error,
            contentDescription = null,
            tint = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            modifier = Modifier.size(20.dp),
        )
        Column {
            Text(title, style = MaterialTheme.typography.bodyMedium)
            Text(
                detail.ifEmpty { "—" },
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
