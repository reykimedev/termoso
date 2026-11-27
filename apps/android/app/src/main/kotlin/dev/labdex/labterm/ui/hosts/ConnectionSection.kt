package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.PickerRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SwitchRow
import dev.labdex.labterm.core.HostChainDraft
import dev.labdex.labterm.core.HostChainItem
import dev.labdex.labterm.core.HostDraft
import dev.labdex.labterm.core.GatewayCard
import dev.labdex.labterm.core.ProxyDraft
import dev.labdex.labterm.core.ProxyItem
import dev.labdex.labterm.core.TransportItem
import dev.labdex.labterm.core.terminalThemes
import androidx.compose.runtime.produceState

/** Picker sentinel: "create a new one" entry. */
private const val NEW = "__new__"

/** Charsets offered per host, like Termius' "Select charset"; `null` is UTF-8. */
private val CHARSETS = listOf(
    "ISO-8859-1", "ISO-8859-2", "ISO-8859-5", "ISO-8859-15",
    "Windows-1250", "Windows-1251", "Windows-1252",
    "KOI8-R", "KOI8-U", "GBK", "GB18030", "Big5", "Shift_JIS", "EUC-JP", "EUC-KR",
)

/**
 * Termius-style "Connection" block of the host editor: jump chain, proxy,
 * colour scheme, charset and the Backspace mode — the SSH options that live in
 * the vault model but had no mobile UI.
 */
@Composable
fun ConnectionSection(state: HostEditorState, draft: HostDraft, vm: HostEditorViewModel, networks: List<TransportItem> = emptyList()) {
    var proxyEditor by remember { mutableStateOf<ProxyDraft?>(null) }
    var chainEditor by remember { mutableStateOf<HostChainDraft?>(null) }
    val themes by produceState(initialValue = terminalThemes()) { value = vm.allThemes() }

    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.connection), style = MaterialTheme.typography.labelLarge)

        val chain = state.chains.firstOrNull { it.id == draft.hostChainId }
        fun chainName(c: HostChainItem) = c.label.ifBlank { c.hostLabels.joinToString(" → ") }
        val chainOptions = inheritOptions(
            state.inherited?.hostChainId?.let { id -> state.chains.firstOrNull { it.id == id }?.let(::chainName) ?: "\u2026" },
            draft.hostChainId,
            stringResource(R.string.none),
        ) + state.chains.map { it.id to chainName(it) } + (NEW to stringResource(R.string.new_chain))
        if (draft.ssh) PickerRow(
            label = stringResource(R.string.host_chaining),
            value = chainOptions.labelOf(draft.hostChainId),
            options = chainOptions,
            selected = draft.hostChainId,
            onPick = { id ->
                if (id == NEW) chainEditor = HostChainDraft(null, draft.vaultId, "", emptyList())
                else vm.update { it.copy(hostChainId = id) }
            },
            empty = null,
        )
        if (draft.ssh && chain != null) {
            EditLink { chainEditor = HostChainDraft(chain.id, chain.vaultId, chain.label, chain.hostIds) }
        }

        if (draft.ssh) {
            VpnNetworkPicker(
                networks = networks,
                selected = draft.transportId,
                inheritedId = state.inherited?.transportId,
                onPick = { id -> vm.update { it.copy(transportId = id) } },
            )
        }

        val proxy = state.proxies.firstOrNull { it.id == draft.proxyId }
        fun proxyName(p: ProxyItem) = "${p.kind.uppercase()} ${p.host}:${p.port}"
        val proxyOptions = inheritOptions(
            state.inherited?.proxyId?.let { id -> state.proxies.firstOrNull { it.id == id }?.let(::proxyName) ?: "\u2026" },
            draft.proxyId,
            stringResource(R.string.none),
        ) + state.proxies.map { it.id to proxyName(it) } + (NEW to stringResource(R.string.new_proxy))
        PickerRow(
            label = stringResource(R.string.proxy),
            value = proxyOptions.labelOf(draft.proxyId),
            options = proxyOptions,
            selected = draft.proxyId,
            onPick = { id ->
                if (id == NEW) proxyEditor = ProxyDraft(null, draft.vaultId, "socks5", "", 1080u, null, null, null)
                else vm.update { it.copy(proxyId = id) }
            },
            empty = null,
        )
        if (proxy != null) {
            EditLink { proxyEditor = ProxyDraft(proxy.id, proxy.vaultId, proxy.kind, proxy.host, proxy.port, proxy.identityId, proxy.username, proxy.password) }
        }

        if (draft.ssh && state.gateways.isNotEmpty()) {
            val online = stringResource(R.string.gateway_online)
            val offline = stringResource(R.string.gateway_offline)
            fun gwLabel(g: GatewayCard) = "${g.name} · ${if (g.online) online else offline}"
            val gw = state.gateways.firstOrNull { it.id == draft.gatewayId }
            PickerRow(
                label = stringResource(R.string.gateway),
                value = gw?.let { gwLabel(it) } ?: stringResource(R.string.none),
                options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + state.gateways.map { it.id to gwLabel(it) },
                selected = draft.gatewayId,
                onPick = { id -> vm.update { it.copy(gatewayId = id) } },
                empty = null,
            )
        }
        if (draft.ssh) {
            val sent = stringResource(R.string.magic_packet_sent)
            val mac = draft.macAddress.orEmpty()
            FormField(
                mac,
                { v -> vm.update { it.copy(macAddress = v.ifBlank { null }) } },
                stringResource(R.string.mac_address),
                placeholder = "aa:bb:cc:dd:ee:ff",
                trailing = if (mac.isBlank()) null else {
                    { TextButton(onClick = { vm.wake(mac, sent) }) { Text(stringResource(R.string.wake)) } }
                },
            )
            state.notice?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (draft.ssh) PickerRow(
            label = stringResource(R.string.color_scheme),
            value = themes.firstOrNull { it.id == canonicalThemeId(draft.colorScheme) || it.name == draft.colorScheme }?.name
                ?: draft.colorScheme ?: stringResource(R.string.app_default),
            options = listOf<Pair<String?, String>>(null to stringResource(R.string.app_default)) + themes.map { it.id to it.name },
            selected = canonicalThemeId(draft.colorScheme),
            onPick = { id -> vm.update { it.copy(colorScheme = id) } },
            empty = null,
        )
        if (draft.ssh) PickerRow(
            label = stringResource(R.string.charset),
            value = draft.charset ?: "UTF-8",
            options = listOf<Pair<String?, String>>(null to "UTF-8") + CHARSETS.map { it to it },
            selected = draft.charset,
            onPick = { id -> vm.update { it.copy(charset = id) } },
            empty = null,
        )
    }
    if (draft.ssh) RowDivider()
    if (draft.ssh) SwitchRow(
        title = stringResource(R.string.backspace_sends_ctrl_h),
        subtitle = stringResource(R.string.for_hosts_whose_shell_expects_h_instead_of_del),
        checked = draft.backspace == "ctrl-h",
        onCheckedChange = { v -> vm.update { it.copy(backspace = if (v) "ctrl-h" else null) } },
    )

    proxyEditor?.let { initial ->
        ProxyEditorPage(
            initial = initial,
            identities = state.identities,
            onSave = { d -> vm.saveProxy(d) { id -> vm.update { it.copy(proxyId = id) } }; proxyEditor = null },
            onDelete = initial.id?.let { id -> { vm.deleteProxy(id); vm.update { it.copy(proxyId = null) }; proxyEditor = null } },
            onDismiss = { proxyEditor = null },
        )
    }
    chainEditor?.let { initial ->
        ChainEditorPage(
            initial = initial,
            hosts = state.hosts.filter { it.id != state.draft?.id },
            onSave = { d -> vm.saveChain(d) { id -> vm.update { it.copy(hostChainId = id) } }; chainEditor = null },
            onDelete = initial.id?.let { id -> { vm.deleteChain(id); vm.update { it.copy(hostChainId = null) }; chainEditor = null } },
            onDismiss = { chainEditor = null },
        )
    }
}

@Composable
private fun EditLink(onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
        TextButton(onClick = onClick) { Text(stringResource(R.string.edit)) }
    }
}

/** Scheme ids saved before the rename open the Labterm pair. */
private fun canonicalThemeId(id: String?): String? = when (id) {
    "termoso-dark" -> "labterm-dark" // rename:keep
    "termoso-light" -> "labterm-light" // rename:keep
    else -> id
}
