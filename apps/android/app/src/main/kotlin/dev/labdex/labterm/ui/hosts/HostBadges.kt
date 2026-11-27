package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Hub
import androidx.compose.material.icons.filled.Badge
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.GppMaybe
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Password
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PersonOff
import androidx.compose.material.icons.filled.Router
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.VerifiedUser
import androidx.compose.material.icons.filled.VpnLock
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportState
import dev.labdex.labterm.ui.netbird.VpnStates
import dev.labdex.labterm.ui.netbird.color
import dev.labdex.labterm.ui.netbird.label
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.core.HostItem
import dev.labdex.labterm.core.HostVpnItem
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.ui.shell.ShellViewModel
import androidx.compose.runtime.produceState
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.theme.StatusWarn

/** Display name of a network type (`Transport::provider`). */
fun vpnProviderName(provider: String): String = when (provider.lowercase()) {
    "netbird" -> "NetBird"
    "tailscale" -> "Tailscale"
    else -> provider.replaceFirstChar { it.uppercase() }
}

/**
 * Live state per network id for the hosts shown, polled every few seconds
 * while the list is on screen (the same model as the VPN networks page).
 */
@Composable
fun rememberVpnStates(repo: VaultRepository, hosts: List<HostItem>): Map<String, TransportState> {
    val ids = remember(hosts) { hosts.mapNotNull { it.vpn?.id }.toSet() }
    val flow = remember(ids) { VpnStates.observe(repo, ids) }
    val states by flow.collectAsStateWithLifecycle(emptyMap())
    return states
}

/**
 * Pill with the network's name and a dot in the tunnel state's colour. An
 * inherited network (from a group or tag) gets an outline instead of a fill.
 */
@Composable
fun VpnBadge(vpn: HostVpnItem, state: TransportState, onClick: (() -> Unit)? = null) {
    val color = state.color()
    val description = stringResource(
        if (vpn.inherited) R.string.vpn_badge_inherited_description else R.string.vpn_badge_description,
        vpnProviderName(vpn.provider),
        vpn.label,
        state.label(),
    )
    val shape = RoundedCornerShape(999.dp)
    Row(
        Modifier
            .padding(start = 6.dp)
            .clip(shape)
            .then(
                if (vpn.inherited) {
                    Modifier.border(1.dp, color.copy(alpha = 0.6f), shape)
                } else {
                    Modifier.background(color.copy(alpha = 0.16f))
                },
            )
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 7.dp, vertical = 3.dp)
            .semantics { contentDescription = description },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Box(Modifier.size(6.dp).clip(CircleShape).background(color))
        Icon(Icons.Filled.VpnLock, contentDescription = null, tint = color, modifier = Modifier.size(12.dp))
        Text(vpn.label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurface, maxLines = 1)
    }
}

/** What host rows need to draw their badges, worked out once per list. */
class HostRowBadges(
    private val vpnStates: Map<String, TransportState>,
    private val teamVault: Boolean,
    private val forwardsByHost: Map<String, Int>,
) {
    fun vpn(v: HostVpnItem): TransportState = vpnStates[v.id] ?: TransportState.Off
    fun facts(h: HostItem): List<HostFact> = hostFacts(h, teamVault, forwardsByHost[h.id] ?: 0)

    companion object {
        val NONE = HostRowBadges(emptyMap(), false, emptyMap())
    }
}

/** Running port forwards per host id in the vault. */
@Composable
fun rememberActiveForwards(shell: ShellViewModel, vaultId: String?): Map<String, Int> {
    val tunnels by shell.forwards.tunnels.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val hostByRule by produceState<Map<String, String>>(emptyMap(), vaultId, revision) {
        value = runCatching { shell.repo.read { pfRules(vaultId) } }.getOrDefault(emptyList()).associate { it.id to it.hostId }
    }
    return remember(tunnels, hostByRule) {
        tunnels.keys.mapNotNull { hostByRule[it] }.groupingBy { it }.eachCount()
    }
}

/** One small connection fact: an icon on host rows, icon + text in the quick menu. */
data class HostFact(val icon: ImageVector, val label: String, val warn: Boolean = false)

/**
 * The connection facts of a host besides its VPN network, in a fixed order:
 * how it logs in, a personal overlay, a host key still to confirm, jump
 * host, gateway, proxy, Mosh, team vault and active port forwards.
 */
fun hostFacts(h: HostItem, teamVault: Boolean, forwards: Int): List<HostFact> = buildList {
    val b = h.badges
    when (b.auth) {
        "opkssh" -> add(HostFact(Icons.Filled.Badge, str(R.string.badge_auth_opkssh)))
        "ssh_id" -> add(HostFact(Icons.Filled.Fingerprint, str(R.string.badge_auth_ssh_id)))
        "certificate" -> add(HostFact(Icons.Filled.VerifiedUser, str(R.string.badge_auth_certificate)))
        "key" -> add(HostFact(Icons.Filled.Key, str(R.string.badge_auth_key)))
        "password" -> add(HostFact(Icons.Filled.Password, str(R.string.badge_auth_password)))
    }
    if (b.overlay) add(HostFact(Icons.Filled.Person, str(R.string.badge_overlay)))
    if (b.overlayPaused) add(HostFact(Icons.Filled.PersonOff, str(R.string.badge_overlay_paused), warn = true))
    if (b.hostKeyTrusted == false) add(HostFact(Icons.Filled.GppMaybe, str(R.string.badge_host_key_unknown), warn = true))
    if (b.jump) add(HostFact(Icons.Filled.Hub, str(R.string.badge_jump)))
    if (b.gateway) add(HostFact(Icons.Filled.Router, str(R.string.badge_gateway)))
    if (b.proxy) add(HostFact(Icons.Filled.Shield, str(R.string.badge_proxy)))
    if (h.useMosh) add(HostFact(Icons.Filled.Bolt, "Mosh"))
    if (teamVault) add(HostFact(Icons.Filled.Groups, str(R.string.badge_team_vault)))
    if (forwards > 0) add(HostFact(Icons.Filled.SwapHoriz, str(R.string.badge_forwards, forwards)))
}

/** Icons only, for the end of a host row; each has its label for TalkBack. */
@Composable
fun HostFactIcons(facts: List<HostFact>) {
    Row(horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
        facts.forEach { f ->
            Icon(
                f.icon,
                contentDescription = f.label,
                tint = if (f.warn) StatusWarn else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(14.dp),
            )
        }
    }
}

/** Icon + text chips, for the host's quick menu where there is room. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HostFactChips(vpn: HostVpnItem?, state: TransportState?, facts: List<HostFact>, onVpn: (() -> Unit)?) {
    if (vpn == null && facts.isEmpty()) return
    FlowRow(
        Modifier.padding(top = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (vpn != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                VpnBadge(vpn, state ?: TransportState.Off, onVpn)
                Text(
                    " " + (state ?: TransportState.Off).label() +
                        if (vpn.inherited) " · " + stringResource(R.string.badge_vpn_inherited) else "",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        facts.forEach { f ->
            val tint = if (f.warn) StatusWarn else MaterialTheme.colorScheme.onSurfaceVariant
            Row(
                Modifier
                    .clip(RoundedCornerShape(999.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .padding(horizontal = 8.dp, vertical = 3.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Icon(f.icon, contentDescription = null, tint = tint, modifier = Modifier.size(14.dp))
                Text(f.label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurface)
            }
        }
    }
}
