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
 * Stored in a reference field (VPN network, proxy, jump hosts, OpenID key) to
 * say "none" on purpose, so a host or group does not take what its group or
 * tags set; `null` still inherits. Same id as `EXPLICIT_NONE` in labterm-proto.
 */
const val NONE_ID = "00000000-0000-0000-0000-000000000000"

/**
 * First options of a picker whose `null` inherits: "Inherited: X" plus a
 * separate [noneLabel] entry ([NONE_ID]) when something is inherited (or
 * "none" was picked before), otherwise one [noneLabel] entry.
 */
@Composable
fun inheritOptions(inherited: String?, selected: String?, noneLabel: String): List<Pair<String?, String>> {
    if (inherited == null && selected != NONE_ID) return listOf<Pair<String?, String>>(null to noneLabel)
    val inherit = inherited?.let { stringResource(R.string.inherited_value, it) } ?: stringResource(R.string.inherited_nothing)
    return listOf<Pair<String?, String>>(null to inherit, NONE_ID to noneLabel)
}

/** The label of [selected] among [options], or the first (inherit) entry's. */
fun List<Pair<String?, String>>.labelOf(selected: String?): String =
    firstOrNull { it.first == selected }?.second ?: first().second

/**
 * The "VPN network" field of the full-screen editors. `null` inherits what
 * the parent group or a tag sets ([inheritedId]); [NONE_ID] connects
 * directly whatever they set.
 */
@Composable
fun VpnNetworkPicker(
    networks: List<TransportItem>,
    selected: String?,
    inheritedId: String?,
    onPick: (String?) -> Unit,
) {
    fun name(n: TransportItem) = "${vpnProviderName(n.provider)}: ${n.label}"
    val inherited = inheritedId?.let { id -> networks.firstOrNull { it.id == id }?.let(::name) ?: "\u2026" }
    val options = inheritOptions(inherited, selected, stringResource(R.string.tunnel_direct)) +
        networks.mapNotNull { n -> n.id?.let { it to name(n) } }
    PickerRow(
        label = stringResource(R.string.vpn_network),
        value = options.labelOf(selected),
        options = options,
        selected = selected,
        onPick = onPick,
        empty = null,
    )
}
