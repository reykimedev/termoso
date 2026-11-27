package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.labdex.labterm.R
import dev.labdex.labterm.core.HostChainDraft
import dev.labdex.labterm.core.HostItem
import dev.labdex.labterm.core.IdentityItem
import dev.labdex.labterm.core.ProxyDraft
import dev.labdex.labterm.ui.components.DeleteCard
import dev.labdex.labterm.ui.components.EditScreen
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.PickerRow
import dev.labdex.labterm.ui.components.SectionCard

val PROXY_KINDS = listOf("socks5" to "SOCKS5", "socks4" to "SOCKS4", "http" to "HTTP")

/** A full-screen edit page over the current screen, in the shared [EditScreen] layout. */
@Composable
fun FullScreenEditor(
    title: String,
    canSave: Boolean,
    onSave: () -> Unit,
    onDismiss: () -> Unit,
    deleteLabel: String? = null,
    onDelete: (() -> Unit)? = null,
    content: @Composable () -> Unit,
) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        EditScreen(title = title, canSave = canSave, onSave = onSave, onClose = onDismiss) { padding ->
            Column(
                Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                content()
                if (onDelete != null) DeleteCard(deleteLabel ?: stringResource(R.string.delete), onDelete)
            }
        }
    }
}

/** Full-screen editor of a SOCKS/HTTP proxy. */
@Composable
fun ProxyEditorPage(
    initial: ProxyDraft,
    identities: List<IdentityItem>,
    onSave: (ProxyDraft) -> Unit,
    onDelete: (() -> Unit)?,
    onDismiss: () -> Unit,
) {
    var kind by remember { mutableStateOf(initial.kind) }
    var host by remember { mutableStateOf(initial.host) }
    var port by remember { mutableStateOf(initial.port.toString()) }
    var identityId by remember { mutableStateOf(initial.identityId) }
    var username by remember { mutableStateOf(initial.username ?: "") }
    var password by remember { mutableStateOf(initial.password ?: "") }
    val valid = host.isNotBlank() && (port.toIntOrNull() ?: 0) in 1..65535
    FullScreenEditor(
        title = if (initial.id == null) stringResource(R.string.new_proxy_title) else stringResource(R.string.edit_proxy),
        canSave = valid,
        onSave = {
            onSave(
                ProxyDraft(
                    initial.id, initial.vaultId, kind, host.trim(), port.toInt().toUShort(), identityId,
                    username.trim().ifEmpty { null }.takeIf { identityId == null },
                    password.ifEmpty { null }.takeIf { identityId == null && kind != "socks4" },
                ),
            )
        },
        onDismiss = onDismiss,
        onDelete = onDelete,
    ) {
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                PickerRow(
                    label = stringResource(R.string.proxy_type),
                    value = PROXY_KINDS.firstOrNull { it.first == kind }?.second ?: kind,
                    options = PROXY_KINDS.map { it.first to it.second },
                    selected = kind,
                    onPick = { it?.let { k -> kind = k } },
                    empty = null,
                )
                FormField(host, { host = it }, stringResource(R.string.hostname_or_ip_address), keyboard = KeyboardType.Uri)
                FormField(port, { port = it.filter(Char::isDigit).take(5) }, stringResource(R.string.port), keyboard = KeyboardType.Number)
            }
        }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                val identity = identities.firstOrNull { it.id == identityId }
                PickerRow(
                    label = stringResource(R.string.identity),
                    value = identity?.label ?: stringResource(R.string.none),
                    options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + identities.map { it.id to it.label },
                    selected = identityId,
                    onPick = { identityId = it },
                    empty = null,
                )
                if (identityId == null) {
                    FormField(username, { username = it }, stringResource(R.string.username))
                    if (kind != "socks4") {
                        FormField(password, { password = it }, stringResource(R.string.password), keyboard = KeyboardType.Password, visual = PasswordVisualTransformation())
                    }
                    Text(
                        stringResource(R.string.proxy_credentials_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

/** Full-screen editor of a jump host chain. [hosts] are the candidates (never the host being edited). */
@Composable
fun ChainEditorPage(
    initial: HostChainDraft,
    hosts: List<HostItem>,
    onSave: (HostChainDraft) -> Unit,
    onDelete: (() -> Unit)?,
    onDismiss: () -> Unit,
) {
    var label by remember { mutableStateOf(initial.label) }
    var hostIds by remember { mutableStateOf(initial.hostIds) }
    val candidates = hosts.filter { it.id !in hostIds }
    FullScreenEditor(
        title = if (initial.id == null) stringResource(R.string.new_chain_title) else stringResource(R.string.edit_chain),
        canSave = hostIds.isNotEmpty(),
        onSave = { onSave(HostChainDraft(initial.id, initial.vaultId, label, hostIds)) },
        onDismiss = onDismiss,
        onDelete = onDelete,
    ) {
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(label, { label = it }, stringResource(R.string.name))
            }
        }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.jump_hosts_in_order), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                hostIds.forEachIndexed { i, id ->
                    val h = hosts.firstOrNull { it.id == id }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("${i + 1}. ${h?.label?.ifBlank { h.address } ?: id}", modifier = Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                        IconButton(enabled = i > 0, onClick = { hostIds = hostIds.toMutableList().also { l -> l.add(i - 1, l.removeAt(i)) } }) {
                            Icon(Icons.Filled.ArrowUpward, contentDescription = stringResource(R.string.move_up))
                        }
                        IconButton(onClick = { hostIds = hostIds - id }) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.remove_2)) }
                    }
                }
                PickerRow(
                    label = stringResource(R.string.add_host),
                    value = "",
                    options = candidates.map { it.id to it.label.ifBlank { it.address } },
                    selected = null,
                    onPick = { id -> id?.let { hostIds = hostIds + it } },
                    empty = stringResource(R.string.no_hosts_yet),
                )
            }
        }
    }
}
