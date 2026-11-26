package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.core.IdentityItem
import dev.labdex.labterm.core.KeyItem
import dev.labdex.labterm.core.OverlayDraft
import dev.labdex.labterm.core.OverlayItem
import dev.labdex.labterm.core.VaultKind
import kotlinx.coroutines.launch

/** [OverlayDraft.scope] values, as the store spells them. */
const val OVERLAY_SCOPE_HOST = "host"
const val OVERLAY_SCOPE_GROUP = "group"
const val OVERLAY_SCOPE_TAG = "tag"
const val OVERLAY_SCOPE_VAULT = "vault"

private data class OverlayLoad(
    val ownVaultId: String,
    val current: OverlayItem?,
    val identities: List<IdentityItem>,
    val openIdKeys: List<KeyItem>,
)

/**
 * "My own credentials" for a team host, group, tag or whole team vault. The
 * link and the credentials stay in the member's personal (or local) vault;
 * the team never sees them. Renders nothing outside team vaults.
 */
@Composable
fun MyCredentialsCard(
    shell: ShellViewModel,
    teamVaultId: String,
    scope: String,
    targetId: String?,
) {
    val coroutines = rememberCoroutineScope()
    var load by remember(teamVaultId, scope, targetId) { mutableStateOf<OverlayLoad?>(null) }
    var identityId by remember(teamVaultId, scope, targetId) { mutableStateOf<String?>(null) }
    var openIdKeyId by remember(teamVaultId, scope, targetId) { mutableStateOf<String?>(null) }
    var agentForwarding by remember(teamVaultId, scope, targetId) { mutableStateOf(false) }
    var forwardX11 by remember(teamVaultId, scope, targetId) { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var reload by remember { mutableStateOf(0) }
    val linkedMessage = stringResource(R.string.overlay_linked)
    val removedMessage = stringResource(R.string.overlay_removed)

    LaunchedEffect(teamVaultId, scope, targetId, reload) {
        runCatching {
            shell.repo.read {
                val vaults = vaults().filter { !it.locked }
                val team = vaults.firstOrNull { it.id == teamVaultId }
                if (team?.kind != VaultKind.TEAM) return@read null
                val current = overlayFor(teamVaultId, scope, targetId)
                val own = current?.vaultId
                    ?: vaults.firstOrNull { it.kind == VaultKind.PERSONAL }?.id
                    ?: vaults.firstOrNull { it.kind == VaultKind.LOCAL }?.id
                    ?: return@read null
                OverlayLoad(
                    ownVaultId = own,
                    current = current,
                    identities = identities(own),
                    openIdKeys = keys(own).filter { it.keyType == "opkssh" },
                )
            }
        }.onSuccess { l ->
            load = l
            identityId = l?.current?.identityId
            openIdKeyId = l?.current?.oidcKeyId
            agentForwarding = l?.current?.agentForwarding ?: false
            forwardX11 = l?.current?.forwardX11 ?: false
        }.onFailure { shell.notify(it.userMessage()) }
    }

    val l = load ?: return
    val current = l.current
    // A group, tag or whole vault is filled by the team: keys only there.
    val keysOnly = scope != OVERLAY_SCOPE_HOST
    val choices = l.identities.filter { !keysOnly || it.sshKeyId != null || it.hasCertificate || it.sshId }

    fun run(block: suspend () -> Unit, done: String) {
        busy = true
        coroutines.launch {
            runCatching { block() }
                .onSuccess { shell.notify(done); reload++ }
                .onFailure { shell.notify(it.userMessage()) }
            busy = false
        }
    }

    SectionCard {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.overlay_title), style = MaterialTheme.typography.titleSmall)
            Text(
                stringResource(
                    when (scope) {
                        OVERLAY_SCOPE_HOST -> R.string.overlay_hint_host
                        OVERLAY_SCOPE_GROUP -> R.string.overlay_hint_group
                        OVERLAY_SCOPE_TAG -> R.string.overlay_hint_tag
                        else -> R.string.overlay_hint_vault
                    },
                ),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (current?.paused == true) {
                Text(
                    stringResource(R.string.overlay_paused, current.pinnedAddress ?: "?"),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                )
                OutlinedButton(
                    enabled = !busy,
                    onClick = { run({ shell.repo.write { confirmOverlay(current.id) } }, linkedMessage) },
                ) { Text(stringResource(R.string.overlay_use_again)) }
            }
            Picker(
                label = stringResource(R.string.overlay_credentials),
                selected = choices.firstOrNull { it.id == identityId }?.let { i ->
                    if (i.username.isBlank()) i.label else "${i.label} (${i.username})"
                },
                options = choices.map { it.id to if (it.username.isBlank()) it.label else "${it.label} (${it.username})" },
                onPick = { identityId = it },
            )
            Text(
                stringResource(if (keysOnly) R.string.overlay_keys_only else R.string.overlay_private),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (l.openIdKeys.isNotEmpty()) {
                Picker(
                    label = stringResource(R.string.overlay_openid_key),
                    selected = l.openIdKeys.firstOrNull { it.id == openIdKeyId }?.label,
                    options = l.openIdKeys.map { it.id to it.label },
                    onPick = { openIdKeyId = it },
                )
            }
            ToggleRow(stringResource(R.string.overlay_agent_forwarding), agentForwarding) { agentForwarding = it }
            ToggleRow(stringResource(R.string.overlay_x11_forwarding), forwardX11) { forwardX11 = it }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                if (current != null) {
                    TextButton(
                        enabled = !busy,
                        onClick = { run({ shell.repo.write { deleteOverlay(current.id) } }, removedMessage) },
                    ) { Text(stringResource(R.string.overlay_use_team)) }
                }
                Button(
                    enabled = !busy && (identityId != null || openIdKeyId != null),
                    onClick = {
                        val draft = OverlayDraft(
                            id = current?.id,
                            vaultId = l.ownVaultId,
                            targetVaultId = teamVaultId,
                            scope = scope,
                            targetId = targetId,
                            identityId = identityId,
                            oidcKeyId = openIdKeyId,
                            agentForwarding = agentForwarding,
                            forwardX11 = forwardX11,
                        )
                        run({ shell.repo.write { saveOverlay(draft) } }, linkedMessage)
                    },
                ) { Text(stringResource(if (current == null) R.string.overlay_use_mine else R.string.save)) }
            }
        }
    }
}

@Composable
private fun Picker(
    label: String,
    selected: String?,
    options: List<Pair<String, String>>,
    onPick: (String?) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Column {
        Text(label, style = MaterialTheme.typography.labelMedium)
        Box {
            OutlinedButton(onClick = { open = true }, modifier = Modifier.fillMaxWidth()) {
                Text(selected ?: stringResource(R.string.none))
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.none)) },
                    onClick = { onPick(null); open = false },
                )
                options.forEach { (id, name) ->
                    DropdownMenuItem(text = { Text(name) }, onClick = { onPick(id); open = false })
                }
            }
        }
    }
}

@Composable
private fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange)
    }
}
