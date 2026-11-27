package dev.labdex.labterm.ui.netbird

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportDetails
import dev.labdex.labterm.core.TransportHost
import dev.labdex.labterm.core.TransportPeer
import dev.labdex.labterm.core.TransportState
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.hosts.vpnProviderName
import dev.labdex.labterm.ui.shell.ShellViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * One VPN network on this device: its state, this device's address in it, the peers it
 * sees and the hosts that use it, with connect, disconnect, sign-in and sign-out.
 * Refreshes every few seconds while shown.
 */
@Composable
fun VpnNetworkScreen(
    shell: ShellViewModel,
    networkId: String,
    onBack: () -> Unit,
    onOpenHost: (String) -> Unit,
) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var details by remember { mutableStateOf<TransportDetails?>(null) }
    var failed by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var signingIn by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf(false) }
    var testing by remember { mutableStateOf<TransportHost?>(null) }
    // Bumped after an action, so the next read happens right away.
    var poke by remember { mutableIntStateOf(0) }

    LaunchedEffect(networkId, revision, poke) {
        while (true) {
            runCatching { shell.repo.read { transportDetails(networkId) } }
                .onSuccess { details = it; failed = null }
                .onFailure { failed = it.userMessage() }
            delay(3_000)
        }
    }

    fun act(job: suspend () -> Unit) {
        if (busy) return
        busy = true
        scope.launch {
            runCatching { job() }.onFailure { shell.notify(it.userMessage()) }
            busy = false
            poke++
        }
    }

    val d = details
    SubScreen(
        d?.network?.label ?: stringResource(R.string.netbird_networks),
        onBack,
        actions = {
            if (d != null) {
                IconButton(onClick = { editing = true }) {
                    Icon(Icons.Filled.Settings, contentDescription = stringResource(R.string.netbird_network_settings))
                }
            }
        },
    ) { padding ->
        if (d == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                val f = failed
                if (f != null) Text(f, color = MaterialTheme.colorScheme.error) else CircularProgressIndicator()
            }
            return@SubScreen
        }
        val n = d.network
        val state = d.state
        val joined = state is TransportState.Connected
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        StateDot(state)
                        Text(state.label(), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(start = 8.dp))
                        if (busy) CircularProgressIndicator(Modifier.padding(start = 12.dp).size(16.dp))
                    }
                    Text(
                        vpnProviderName(n.provider) + " · " + serverLabel(n),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (state is TransportState.Error) {
                        Text(state.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                    }
                    d.liveError?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (joined) {
                            Button(enabled = !busy, onClick = { act { shell.repo.read { transportDisconnect(networkId) } } }) {
                                Text(stringResource(R.string.vpn_disconnect))
                            }
                        } else if (state !is TransportState.Unavailable) {
                            Button(
                                enabled = !busy && state !is TransportState.Connecting,
                                onClick = {
                                    if (!n.hasSetupKey && !n.signedIn) {
                                        signingIn = true
                                    } else {
                                        act { shell.repo.read { transportConnect(networkId) } }
                                    }
                                },
                            ) { Text(stringResource(R.string.vpn_connect)) }
                        }
                        if (!n.hasSetupKey && state !is TransportState.Unavailable) {
                            OutlinedButton(enabled = !busy, onClick = { signingIn = true }) {
                                Text(stringResource(if (n.signedIn) R.string.netbird_sign_in_again else R.string.netbird_sign_in))
                            }
                        }
                        if (n.signedIn) {
                            OutlinedButton(enabled = !busy, onClick = {
                                act {
                                    shell.repo.read { transportSignOut(networkId) }
                                    shell.notify(str(R.string.vpn_signed_out, n.label))
                                }
                            }) { Text(stringResource(R.string.vpn_sign_out)) }
                        }
                    }
                }
            }

            SectionLabel(stringResource(R.string.vpn_this_device))
            SectionCard {
                InfoRow(stringResource(R.string.vpn_address), d.ip, mono = true)
                RowDivider()
                InfoRow(stringResource(R.string.vpn_dns_name), d.fqdn, mono = true)
                RowDivider()
                InfoRow(
                    stringResource(R.string.vpn_account),
                    n.account ?: if (n.hasSetupKey) str(R.string.netbird_joins_with_key) else if (n.signedIn) str(R.string.vpn_whoever_signed_in) else "",
                )
                if (joined) {
                    RowDivider()
                    InfoRow(stringResource(R.string.vpn_management_server), str(if (d.managementConnected) R.string.netbird_connected else R.string.vpn_not_connected))
                    RowDivider()
                    InfoRow(stringResource(R.string.vpn_signal_server), str(if (d.signalConnected) R.string.netbird_connected else R.string.vpn_not_connected))
                } else {
                    Text(
                        stringResource(R.string.vpn_live_when_connected),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }

            SectionLabel(stringResource(R.string.vpn_hosts_using))
            SectionCard {
                if (d.hosts.isEmpty()) {
                    Text(
                        stringResource(R.string.vpn_no_hosts_using),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                }
                d.hosts.forEachIndexed { i, h ->
                    if (i > 0) RowDivider()
                    ListRow(
                        title = h.label.ifEmpty { h.address },
                        subtitle = h.address,
                        leading = { IconTile(Icons.Filled.Dns) },
                        modifier = Modifier.clickable { onOpenHost(h.id) },
                    ) {
                        if (state !is TransportState.Unavailable) {
                            TextButton(onClick = { testing = h }) { Text(stringResource(R.string.vpn_test)) }
                        }
                    }
                }
            }

            if (joined) {
                val routes = d.peers.flatMap { p -> p.routes.map { it to p.fqdn.ifEmpty { p.ip } } }
                SectionLabel(stringResource(R.string.vpn_routes, routes.size))
                SectionCard {
                    if (routes.isEmpty()) {
                        Text(
                            stringResource(R.string.vpn_no_routes),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(16.dp),
                        )
                    }
                    routes.forEachIndexed { i, (network, via) ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = if (network == "0.0.0.0/0" || network == "::/0") {
                                stringResource(R.string.vpn_exit_node, network)
                            } else network,
                            subtitle = stringResource(R.string.vpn_route_through, via),
                            leading = { IconTile(Icons.Filled.Public) },
                        )
                    }
                }

                if (d.dns.isNotEmpty()) {
                    SectionLabel(stringResource(R.string.vpn_dns))
                    SectionCard {
                        d.dns.forEachIndexed { i, g ->
                            if (i > 0) RowDivider()
                            ListRow(
                                title = if (g.domains.isEmpty()) stringResource(R.string.vpn_dns_all_names) else g.domains.joinToString(", "),
                                subtitle = g.servers.joinToString(", ") + " · " +
                                    stringResource(if (g.tunnel) R.string.vpn_dns_tunnel else R.string.vpn_dns_netbird),
                            )
                        }
                    }
                }

                SectionLabel(stringResource(R.string.vpn_peers, d.peers.size))
                SectionCard {
                    if (d.peers.isEmpty()) {
                        Text(
                            stringResource(R.string.vpn_no_peers),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(16.dp),
                        )
                    }
                    d.peers.forEachIndexed { i, p ->
                        if (i > 0) RowDivider()
                        PeerRow(p)
                    }
                }
            }
        }
    }

    testing?.let { h -> HostTestDialog(h, shell.repo) { testing = null } }

    if (signingIn) {
        d?.network?.let { n ->
            NetbirdSignInDialog(n, shell.repo, shell::notify) { ok ->
                signingIn = false
                if (ok) act { shell.repo.read { transportConnect(networkId) } } else poke++
            }
        }
    }

    if (editing) {
        d?.network?.let { n ->
            NetworkEditorPage(
                n,
                onDismiss = { editing = false },
                onSignIn = if (n.hasSetupKey) null else ({ editing = false; signingIn = true }),
                onDelete = {
                    scope.launch {
                        runCatching { shell.repo.write { deleteTransport(networkId) } }
                            .onSuccess { editing = false; onBack() }
                            .onFailure { shell.notify(it.userMessage()) }
                    }
                },
            ) { saved ->
                scope.launch {
                    runCatching { shell.repo.write { saveTransport(saved) } }
                        .onSuccess { editing = false; poke++ }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            }
        }
    }
}

@Composable
private fun StateDot(state: TransportState) {
    Box(Modifier.size(10.dp).clip(CircleShape).background(state.color()))
}

@Composable
private fun InfoRow(label: String, value: String, mono: Boolean = false) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(120.dp),
        )
        Text(
            value.ifEmpty { "—" },
            style = MaterialTheme.typography.bodyMedium,
            fontFamily = if (mono) FontFamily.Monospace else null,
        )
    }
}

@Composable
private fun PeerRow(p: TransportPeer) {
    val facts = listOfNotNull(
        p.ip.ifEmpty { null },
        str(if (!p.connected) R.string.vpn_peer_idle else if (p.relayed) R.string.vpn_peer_relayed else R.string.vpn_peer_direct),
        if (p.connected && p.latencyMs > 0u) "${p.latencyMs} ms" else null,
    ).joinToString(" · ")
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.size(8.dp).clip(RoundedCornerShape(4.dp))
                .background(if (p.connected) TransportState.Connected.color() else MaterialTheme.colorScheme.outlineVariant),
        )
        Column(Modifier.padding(start = 12.dp)) {
            Text(p.fqdn.ifEmpty { p.ip }, style = MaterialTheme.typography.bodyMedium, fontFamily = FontFamily.Monospace)
            Text(facts, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
