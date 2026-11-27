package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.labdex.labterm.R
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.PickerRow
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.IconCatalog
import dev.labdex.labterm.ui.components.IconPickerSheet
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SwitchRow
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.core.GroupItem
import dev.labdex.labterm.core.InheritedInfo
import dev.labdex.labterm.core.KeyItem
import dev.labdex.labterm.core.SshDefaultsDraft
import kotlinx.coroutines.launch

/** Whose SSH defaults the editor edits. */
sealed interface DefaultsTarget {
    data class Group(val id: String) : DefaultsTarget
    data class Tag(val id: String) : DefaultsTarget
}

/** What the editor loaded: name (and priority for tags), defaults, keys, parent's values. */
private data class Loaded(
    val label: String,
    val priority: Int?,
    val vaultId: String,
    val defaults: SshDefaultsDraft,
    val keys: List<KeyItem>,
    val inherited: InheritedInfo?,
    /** Groups only: the parent, and the groups it may move under. */
    val parentId: String? = null,
    val icon: String? = null,
    val parents: List<GroupItem> = emptyList(),
    val allGroups: List<GroupItem> = emptyList(),
)

/**
 * Username, credentials and connection settings a group or tag hands down to
 * its hosts. A host's own settings win, then its groups (nearest first), then
 * its tags (higher priority first, then by name). An empty username falls
 * back to the nearest place that sets one; key and password come as one set.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DefaultsEditorDialog(shell: ShellViewModel, target: DefaultsTarget, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    val savedMessage = stringResource(R.string.settings_saved)
    var loaded by remember(target) { mutableStateOf<Loaded?>(null) }
    var label by remember(target) { mutableStateOf("") }
    var priority by remember(target) { mutableStateOf("0") }
    var d by remember(target) { mutableStateOf<SshDefaultsDraft?>(null) }
    var parentId by remember(target) { mutableStateOf<String?>(null) }
    var parentPicker by remember { mutableStateOf(false) }
    var icon by remember(target) { mutableStateOf<String?>(null) }
    var iconPicker by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }

    LaunchedEffect(target) {
        runCatching {
            shell.repo.read {
                when (target) {
                    is DefaultsTarget.Group -> {
                        val g = groupDraft(target.id)
                        val all = groups(g.vaultId)
                        Loaded(
                            g.label, null, g.vaultId, g.defaults, keys(g.vaultId), g.parentId?.let { inherited(it) },
                            parentId = g.parentId,
                            icon = g.icon,
                            parents = parentCandidates(all, target.id),
                            allGroups = all,
                        )
                    }
                    is DefaultsTarget.Tag -> {
                        val t = tagDraft(target.id)
                        Loaded(t.label, t.priority, t.vaultId, t.defaults, keys(t.vaultId), null)
                    }
                }
            }
        }.onSuccess {
            loaded = it
            label = it.label
            priority = it.priority?.toString() ?: "0"
            d = it.defaults
            parentId = it.parentId
            icon = it.icon
        }.onFailure { shell.notify(it.userMessage()); onDismiss() }
    }

    fun save() {
        val draft = d ?: return
        if (loaded == null || label.isBlank()) return
        saving = true
        scope.launch {
            runCatching {
                shell.repo.write {
                    when (target) {
                        is DefaultsTarget.Group -> {
                            val g = groupDraft(target.id)
                            saveGroupDraft(g.copy(label = label.trim(), parentId = parentId, icon = icon, defaults = draft))
                        }
                        is DefaultsTarget.Tag -> {
                            val t = tagDraft(target.id)
                            saveTagDraft(
                                t.copy(
                                    label = label.trim(),
                                    priority = (priority.toIntOrNull() ?: 0).coerceIn(-1000, 1000),
                                    defaults = draft,
                                ),
                            )
                        }
                    }
                }
            }.onSuccess {
                shell.notify(savedMessage)
                onDismiss()
            }.onFailure { shell.notify(it.userMessage()) }
            saving = false
        }
    }

    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = {
                        Text(
                            when (target) {
                                is DefaultsTarget.Group -> stringResource(R.string.edit_group)
                                is DefaultsTarget.Tag -> stringResource(R.string.edit_tag)
                            },
                        )
                    },
                    navigationIcon = {
                        IconButton(onClick = onDismiss) {
                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close))
                        }
                    },
                    actions = {
                        TextButton(onClick = ::save, enabled = !saving && d != null && label.isNotBlank()) {
                            Text(stringResource(R.string.save))
                        }
                    },
                )
            },
        ) { padding ->
            val draft = d
            val l = loaded
            if (draft == null || l == null) {
                Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
                return@Scaffold
            }
            val inh = l.inherited
            val networks = rememberVpnNetworks(shell.repo, l.vaultId)
            Column(
                Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                SectionCard {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        FormField(label, { label = it }, stringResource(R.string.name))
                        if (target is DefaultsTarget.Tag) {
                            FormField(
                                priority,
                                { v -> priority = v.filter { it.isDigit() || it == '-' }.take(5) },
                                stringResource(R.string.priority),
                                keyboard = KeyboardType.Number,
                            )
                        }
                        Text(
                            when (target) {
                                is DefaultsTarget.Group -> stringResource(R.string.group_settings_hint)
                                is DefaultsTarget.Tag -> stringResource(R.string.tag_settings_hint)
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    if (target is DefaultsTarget.Group) {
                        val paths = remember(l.allGroups) { groupPaths(l.allGroups) }
                        RowDivider()
                        ChevronRow(
                            title = stringResource(R.string.parent_group),
                            badge = parentId?.let { paths[it] } ?: stringResource(R.string.none),
                            modifier = Modifier.clickable { parentPicker = true },
                        )
                        RowDivider()
                        ChevronRow(
                            title = stringResource(R.string.icon),
                            leading = { GroupIconTile(icon) },
                            badge = icon?.let { IconCatalog.picked(it)?.title ?: stringResource(R.string.icon_custom) } ?: stringResource(R.string.icon_folder),
                            modifier = Modifier.clickable { iconPicker = true },
                        )
                    }
                }
                SectionCard {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        if (draft.identityId != null) {
                            Text(
                                stringResource(R.string.defaults_use_shared_identity),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        } else {
                            FormField(
                                draft.username,
                                { v -> d = draft.copy(username = v) },
                                stringResource(R.string.username),
                                placeholder = inh?.username ?: stringResource(R.string.asked_on_connect),
                            )
                            val stored = draft.password == null && draft.hasPassword
                            val keepOrClear: (@Composable () -> Unit)? = if (draft.hasPassword) {
                                {
                                    TextButton(onClick = { d = draft.copy(password = if (stored) "" else null) }) {
                                        Text(if (stored) stringResource(R.string.clear) else stringResource(R.string.keep_saved))
                                    }
                                }
                            } else {
                                null
                            }
                            FormField(
                                value = draft.password ?: "",
                                onChange = { v -> d = draft.copy(password = v) },
                                label = if (stored) stringResource(R.string.password_saved) else stringResource(R.string.password),
                                placeholder = when {
                                    stored -> "••••••••"
                                    inh?.hasPassword == true -> stringResource(R.string.inherited_from_group)
                                    else -> null
                                },
                                keyboard = KeyboardType.Password,
                                visual = PasswordVisualTransformation(),
                                trailing = keepOrClear,
                            )
                            val key = l.keys.firstOrNull { it.id == draft.sshKeyId }
                            PickerRow(
                                label = stringResource(R.string.ssh_key),
                                value = key?.let { "${it.label} · ${it.keyType}" }
                                    ?: inh?.sshKeyLabel?.let { stringResource(R.string.inherited_value, it) }
                                    ?: stringResource(R.string.none),
                                options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) +
                                    l.keys.map { it.id to "${it.label} · ${it.keyType}" },
                                selected = draft.sshKeyId,
                                onPick = { id -> d = draft.copy(sshKeyId = id) },
                                empty = null,
                            )
                        }
                    }
                }
                SectionCard {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(stringResource(R.string.connection), style = MaterialTheme.typography.labelLarge)
                        VpnNetworkPicker(
                            networks = networks,
                            selected = draft.transportId,
                            inheritedId = inh?.transportId,
                            onPick = { id -> d = draft.copy(transportId = id) },
                        )
                        FormField(
                            draft.port?.toString() ?: "",
                            { v -> d = draft.copy(port = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { p -> p in 1u..65535u }?.toUShort()) },
                            stringResource(R.string.port),
                            placeholder = inh?.port?.toString() ?: "22",
                            keyboard = KeyboardType.Number,
                        )
                        FormField(
                            draft.keepAliveInterval?.toString() ?: "",
                            { v -> d = draft.copy(keepAliveInterval = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { it > 0u }) },
                            stringResource(R.string.keep_alive_interval_seconds),
                            keyboard = KeyboardType.Number,
                        )
                        FormField(
                            draft.timeout?.toString() ?: "",
                            { v -> d = draft.copy(timeout = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { it > 0u }) },
                            stringResource(R.string.connection_timeout_seconds),
                            keyboard = KeyboardType.Number,
                        )
                    }
                    SwitchRow(
                        title = stringResource(R.string.agent_forwarding),
                        checked = draft.agentForwarding,
                        onCheckedChange = { v -> d = draft.copy(agentForwarding = v) },
                    )
                }
                MyCredentialsCard(
                    shell,
                    l.vaultId,
                    if (target is DefaultsTarget.Group) OVERLAY_SCOPE_GROUP else OVERLAY_SCOPE_TAG,
                    when (target) {
                        is DefaultsTarget.Group -> target.id
                        is DefaultsTarget.Tag -> target.id
                    },
                )
            }
        }
        val l = loaded
        if (iconPicker) {
            IconPickerSheet(
                selected = icon,
                automaticLabel = stringResource(R.string.icon_folder),
                onPick = { icon = it; iconPicker = false },
                onDismiss = { iconPicker = false },
            )
        }
        if (parentPicker && l != null) {
            ParentPicker(l.parents, parentId, onPick = { parentId = it; parentPicker = false }, onDismiss = { parentPicker = false })
        }
    }
}

@Composable
private fun ParentPicker(groups: List<GroupItem>, current: String?, onPick: (String?) -> Unit, onDismiss: () -> Unit) {
    GroupPickerDialog(
        title = stringResource(R.string.parent_group),
        groups = groups,
        current = current,
        onPick = onPick,
        onDismiss = onDismiss,
    )
}
