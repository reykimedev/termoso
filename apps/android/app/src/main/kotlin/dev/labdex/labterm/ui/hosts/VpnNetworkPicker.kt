package dev.labdex.labterm.ui.hosts

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.res.stringResource
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportItem
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.ui.components.PickerRow

/** VPN networks of a vault, for the pickers in the host, group and tag editors. */
@Composable
fun rememberVpnNetworks(repo: VaultRepository, vaultId: String): List<TransportItem> {
    val list by produceState(emptyList<TransportItem>(), vaultId) {
        value = runCatching { repo.read { transports(vaultId) } }.getOrDefault(emptyList())
    }
    return list
}

/**
 * The "VPN network" field of the full-screen editors. `None` means direct
 * for a group's root, or whatever the parent group or a tag sets
 * ([inheritedId], shown as the placeholder value).
 */
@Composable
fun VpnNetworkPicker(
    networks: List<TransportItem>,
    selected: String?,
    inheritedId: String?,
    onPick: (String?) -> Unit,
) {
    fun name(n: TransportItem) = "${vpnProviderName(n.provider)}: ${n.label}"
    val inherited = networks.firstOrNull { it.id == inheritedId }
    val none = inherited?.let { stringResource(R.string.inherited_value, name(it)) } ?: stringResource(R.string.tunnel_direct)
    PickerRow(
        label = stringResource(R.string.vpn_network),
        value = networks.firstOrNull { it.id == selected }?.let(::name) ?: none,
        options = listOf<Pair<String?, String>>(null to none) + networks.mapNotNull { n -> n.id?.let { it to name(n) } },
        selected = selected,
        onPick = onPick,
        empty = null,
    )
}
