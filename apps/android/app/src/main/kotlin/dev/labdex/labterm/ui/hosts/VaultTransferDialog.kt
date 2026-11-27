package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransferPlanCard
import dev.labdex.labterm.core.VaultAccess
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.core.VaultKind
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.vault.vaultLabel
import kotlinx.coroutines.launch

/**
 * "Move to vault…" / "Copy to vault…" for hosts and whole groups: pick the vault,
 * choose whether logins go along, check the preview (what is copied, reused,
 * unlinked or refused), then run it. Same rules as desktop (labterm_client::vaultmove).
 */
@Composable
fun VaultTransferDialog(
    repo: VaultRepository,
    /** Vault the items come from. */
    source: VaultInfo?,
    vaults: List<VaultInfo>,
    hostIds: List<String>,
    groupIds: List<String>,
    move: Boolean,
    onNotify: (String) -> Unit,
    onDone: () -> Unit,
    onDismiss: () -> Unit,
) {
    val targets = vaults.filter { it.id != source?.id && !it.locked && it.access != VaultAccess.VIEW }
    var target by remember { mutableStateOf(targets.singleOrNull()) }
    var picked by remember { mutableStateOf(targets.size == 1) }
    var shared by remember { mutableStateOf<Boolean?>(null) }
    // Logins go along by default, except from a personal vault into a team.
    val withCredentials = shared ?: !(target?.kind == VaultKind.TEAM && source?.kind != VaultKind.TEAM)
    var plan by remember { mutableStateOf<TransferPlanCard?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(picked, target?.id, withCredentials) {
        plan = null
        error = null
        val t = target ?: return@LaunchedEffect
        if (!picked) return@LaunchedEffect
        runCatching {
            repo.read { vaultTransferPlan(hostIds, groupIds, t.id, null, move, withCredentials) }
        }.onSuccess { plan = it }.onFailure { error = it.userMessage() }
    }

    val title = if (move) stringResource(R.string.transfer_move_title) else stringResource(R.string.transfer_copy_title)
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(if (picked && target != null) "$title · ${vaultLabel(target!!)}" else title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                if (!picked) {
                    if (targets.isEmpty()) Text(stringResource(R.string.transfer_no_other_vaults))
                    targets.forEach { v ->
                        Choice(vaultLabel(v), target?.id == v.id) { target = v }
                    }
                } else {
                    Choice(stringResource(R.string.transfer_logins_go_along), withCredentials) { shared = true }
                    Choice(stringResource(R.string.transfer_leave_logins_out), !withCredentials) { shared = false }
                    Spacer(Modifier.height(12.dp))
                    val p = plan
                    when {
                        error != null -> Text(error!!, color = MaterialTheme.colorScheme.error)
                        p == null -> CircularProgressIndicator(Modifier.size(20.dp))
                        p.blocked.isNotEmpty() -> p.blocked.forEach { b ->
                            Text(
                                (if (b.label.isNotEmpty()) "“${b.label}”: " else "") + blockerText(b.reason),
                                color = MaterialTheme.colorScheme.error,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                        else -> {
                            Text(
                                stringResource(R.string.transfer_summary, p.groups.toInt(), p.hosts.toInt()) +
                                    if (move) " · " + stringResource(R.string.transfer_originals_removed) else "",
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            p.dependencies.groupBy { it.action }.forEach { (action, items) ->
                                Spacer(Modifier.height(8.dp))
                                Text(
                                    actionText(action),
                                    style = MaterialTheme.typography.labelMedium,
                                    fontWeight = FontWeight.Medium,
                                    color = if (action == "breaks" || action == "unlink") {
                                        MaterialTheme.colorScheme.error
                                    } else {
                                        MaterialTheme.colorScheme.onSurfaceVariant
                                    },
                                )
                                Text(
                                    items.joinToString(", ") { kindText(it.kind) + if (it.label.isNotEmpty()) " “${it.label}”" else "" },
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            if (!picked) {
                TextButton(onClick = { picked = true }, enabled = target != null) { Text(stringResource(R.string.transfer_next)) }
            } else {
                val p = plan
                TextButton(
                    enabled = !busy && p != null && p.blocked.isEmpty(),
                    onClick = {
                        val t = target ?: return@TextButton
                        busy = true
                        scope.launch {
                            runCatching {
                                if (move) repo.perms.require(source?.id, "entities.move")
                                repo.write { vaultTransfer(hostIds, groupIds, t.id, null, move, withCredentials) }
                            }.onSuccess {
                                onNotify(
                                    if (move) str(R.string.transfer_moved_to, vaultLabel(t))
                                    else str(R.string.transfer_copied_to, vaultLabel(t)),
                                )
                                onDone()
                            }.onFailure { error = it.userMessage(); busy = false }
                        }
                    },
                ) { Text(if (move) stringResource(R.string.move) else stringResource(R.string.copy)) }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !busy) { Text(stringResource(R.string.cancel)) }
        },
    )
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = onClick)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

private fun blockerText(reason: String): String = when (reason) {
    "same_vault" -> str(R.string.transfer_blocked_same_vault)
    "mixed_vaults" -> str(R.string.transfer_blocked_mixed_vaults)
    "target_locked" -> str(R.string.transfer_blocked_target_locked)
    "target_read_only" -> str(R.string.transfer_blocked_target_read_only)
    "target_group" -> str(R.string.transfer_blocked_target_group)
    "source_read_only" -> str(R.string.transfer_blocked_source_read_only)
    "connect_only" -> str(R.string.transfer_blocked_connect_only)
    "hidden" -> str(R.string.transfer_blocked_hidden)
    "restricted" -> str(R.string.transfer_blocked_restricted)
    else -> str(R.string.transfer_blocked_nothing)
}

private fun actionText(action: String): String = when (action) {
    "copy" -> str(R.string.transfer_action_copy)
    "reuse" -> str(R.string.transfer_action_reuse)
    "move_along" -> str(R.string.transfer_action_move_along)
    "leave_out" -> str(R.string.transfer_action_leave_out)
    "unlink" -> str(R.string.transfer_action_unlink)
    "rewrite" -> str(R.string.transfer_action_rewrite)
    "remove" -> str(R.string.transfer_action_remove)
    else -> str(R.string.transfer_action_breaks)
}

private fun kindText(kind: String): String = when (kind) {
    "identity" -> str(R.string.transfer_kind_identity)
    "ssh_key" -> str(R.string.transfer_kind_key)
    "snippet" -> str(R.string.transfer_kind_snippet)
    "proxy" -> str(R.string.transfer_kind_proxy)
    "host_chain" -> str(R.string.transfer_kind_jump_hosts)
    "port_knocking" -> str(R.string.transfer_kind_port_knocking)
    "transport" -> str(R.string.transfer_kind_vpn)
    "color_scheme" -> str(R.string.transfer_kind_color_scheme)
    "tag" -> str(R.string.transfer_kind_tag)
    "pf_rule" -> str(R.string.transfer_kind_port_forwarding)
    "host_overlay" -> str(R.string.transfer_kind_overlay)
    else -> kind
}
