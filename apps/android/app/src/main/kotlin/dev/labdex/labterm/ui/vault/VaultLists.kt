package dev.labdex.labterm.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.EmptyState
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.itemClicks
import dev.labdex.labterm.ui.terminal.copyToClipboard
import dev.labdex.labterm.str
import androidx.compose.ui.platform.LocalContext
import androidx.compose.material.icons.filled.ContentCopy
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.connections.historySubtitle
import dev.labdex.labterm.ui.hosts.ConfirmDialog
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.core.HistoryItem
import dev.labdex.labterm.core.IdentityItem
import dev.labdex.labterm.core.KeyItem
import dev.labdex.labterm.core.KnownHostItem
import kotlinx.coroutines.launch

/** Trusted server keys; swipe-free removal via the trailing trash icon. */
@Composable
fun KnownHostsScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<KnownHostItem>>(emptyList()) }
    var confirm by remember { mutableStateOf<KnownHostItem?>(null) }
    var menu by remember { mutableStateOf<KnownHostItem?>(null) }
    val context = LocalContext.current
    LaunchedEffect(revision) {
        runCatching { shell.repo.read { knownHosts() } }
            .onSuccess { items = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen(stringResource(R.string.known_hosts), onBack) { padding ->
        if (items.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.no_known_hosts),
                    hint = stringResource(R.string.server_keys_you_accept_when_connecting_are_remembered),
                    icon = Icons.Filled.Fingerprint,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    items.forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = k.hostname,
                            subtitle = "${k.keyType} · ${k.fingerprint}",
                            leading = { IconTile(Icons.Filled.Fingerprint) },
                            modifier = Modifier.itemClicks(onClick = { menu = k }, onMenu = { menu = k }),
                        ) {
                            ItemMoreButton({ menu = k })
                        }
                    }
                }
            }
        }
    }
    menu?.let { k ->
        ItemActionsSheet(
            title = k.hostname,
            subtitle = "${k.keyType} · ${k.fingerprint}",
            leading = { IconTile(Icons.Filled.Fingerprint) },
            sections = listOf(
                SheetSection(
                    stringResource(R.string.known_hosts),
                    listOf(
                        SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.copy_fingerprint)) {
                            copyToClipboard(context, k.fingerprint)
                            shell.notify(str(R.string.copied, k.fingerprint))
                        },
                    ),
                ),
                SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.forget), danger = true) { confirm = k })),
            ),
            onClose = { menu = null },
        )
    }
    confirm?.let { k ->
        ConfirmDialog(
            title = stringResource(R.string.forget_2, k.hostname),
            text = stringResource(R.string.you_will_be_asked_to_confirm_its_key),
            confirm = stringResource(R.string.forget),
            onConfirm = {
                confirm = null
                scope.launch {
                    runCatching { shell.repo.write { forgetKnownHost(k.id) } }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { confirm = null },
        )
    }
}

/** Past connections of the selected vault, newest first. */
@Composable
fun HistoryScreen(shell: ShellViewModel, onBack: () -> Unit, onOpenHost: (String) -> Unit) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vault = vaults.firstOrNull { it.id == selectedId }
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<HistoryItem>>(emptyList()) }
    var confirmClear by remember { mutableStateOf(false) }
    LaunchedEffect(revision, vault) {
        val v = vault ?: return@LaunchedEffect
        runCatching { shell.repo.read { history(200u) } }
            .onSuccess { items = it.filter { h -> h.belongsTo(v) } }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen(
        if (vault != null) stringResource(R.string.history_2, vault.name) else stringResource(R.string.history),
        onBack,
        actions = {
            if (items.isNotEmpty()) {
                IconButton(onClick = { confirmClear = true }) { Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.clear_history)) }
            }
        },
    ) { padding ->
        if (items.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.no_connections_yet),
                    hint = stringResource(R.string.sessions_you_open_show_up_here_with_their),
                    icon = Icons.Filled.History,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    items.forEachIndexed { i, h ->
                        if (i > 0) RowDivider()
                        val hostId = h.hostId
                        ListRow(
                            title = h.label.ifBlank { h.target },
                            subtitle = historySubtitle(h),
                            leading = { IconTile(Icons.Filled.History) },
                            modifier = if (hostId != null) Modifier.clickable { onOpenHost(hostId) } else Modifier,
                            titleColor = if (h.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
        }
    }
    if (confirmClear) {
        ConfirmDialog(
            title = stringResource(R.string.clear_history_2),
            text = stringResource(R.string.removes_all_entries_of_this_vault_from_this, items.size),
            confirm = stringResource(R.string.clear),
            onConfirm = {
                confirmClear = false
                val id = vault?.id ?: return@ConfirmDialog
                scope.launch {
                    runCatching { shell.repo.write { clearVaultHistory(id) } }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { confirmClear = false },
        )
    }
}
