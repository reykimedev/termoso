package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.AccountTree
import androidx.compose.material.icons.filled.Lan
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.HostChainDraft
import dev.labdex.labterm.core.HostChainItem
import dev.labdex.labterm.core.HostItem
import dev.labdex.labterm.core.IdentityItem
import dev.labdex.labterm.core.ProxyDraft
import dev.labdex.labterm.core.ProxyItem
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.EmptyState
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.components.itemClicks
import dev.labdex.labterm.ui.shell.ShellViewModel
import kotlinx.coroutines.launch

private fun ProxyItem.title() = "${PROXY_KINDS.firstOrNull { it.first == kind }?.second ?: kind} · $host:$port"

private fun ProxyItem.draft() = ProxyDraft(id, vaultId, kind, host, port, identityId, username, password)

private fun HostChainItem.draft() = HostChainDraft(id, vaultId, label, hostIds)

private fun HostChainItem.title() = label.ifBlank { hostLabels.joinToString(" → ") }

/**
 * The proxies of the selected vault: ⋯ or long-press for Edit, Duplicate and
 * Delete, tap to edit on the full-screen page, + for a new one.
 */
@Composable
fun ProxiesScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<ProxyItem>>(emptyList()) }
    var identities by remember { mutableStateOf<List<IdentityItem>>(emptyList()) }
    var menu by remember { mutableStateOf<ProxyItem?>(null) }
    var editing by remember { mutableStateOf<ProxyDraft?>(null) }
    var confirmDelete by remember { mutableStateOf<ProxyItem?>(null) }
    LaunchedEffect(vaultId, revision) {
        runCatching { shell.repo.read { proxies(vaultId) to identities(vaultId) } }
            .onSuccess { (p, i) -> items = p; identities = i }
            .onFailure { shell.notify(it.userMessage()) }
    }
    fun write(block: suspend () -> Unit) {
        scope.launch { runCatching { block() }.onFailure { shell.notify(it.userMessage()) } }
    }

    RouteList(
        title = stringResource(R.string.proxies),
        onBack = onBack,
        icon = Icons.Filled.Lan,
        empty = items.isEmpty(),
        emptyTitle = stringResource(R.string.no_proxies_yet),
        emptyHint = stringResource(R.string.proxies_hint),
        newLabel = stringResource(R.string.new_proxy_title),
        onNew = vaultId?.let { v -> { editing = ProxyDraft(null, v, "socks5", "", 1080.toUShort(), null, null, null) } },
        rows = items.map { p ->
            RouteRow(p.title(), p.identityLabel ?: p.username, onClick = { editing = p.draft() }, onMenu = { menu = p })
        },
    )

    menu?.let { p ->
        ItemActionsSheet(
            title = p.title(),
            subtitle = p.identityLabel ?: p.username,
            leading = { IconTile(Icons.Filled.Lan) },
            sections = listOf(
                SheetSection(
                    stringResource(R.string.proxy),
                    listOf(
                        SheetAction(Icons.Filled.Edit, stringResource(R.string.edit)) { editing = p.draft() },
                        SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.duplicate)) {
                            write { shell.repo.write { saveProxy(p.draft().copy(id = null)) } }
                        },
                    ),
                ),
                SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true) { confirmDelete = p })),
            ),
            onClose = { if (menu == p) menu = null },
        )
    }
    editing?.let { d ->
        ProxyEditorPage(
            initial = d,
            identities = identities,
            onSave = { out -> editing = null; write { shell.repo.write { saveProxy(out) } } },
            onDelete = d.id?.let { id -> { editing = null; confirmDelete = items.firstOrNull { it.id == id } } },
            onDismiss = { editing = null },
        )
    }
    confirmDelete?.let { p ->
        ConfirmDialog(
            title = stringResource(R.string.delete_proxy_title, p.title()),
            text = stringResource(R.string.delete_proxy_text),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = null; write { shell.repo.write { deleteProxy(p.id) } } },
            onDismiss = { confirmDelete = null },
        )
    }
}

/** The jump host chains of the selected vault, same pattern as [ProxiesScreen]. */
@Composable
fun JumpChainsScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<HostChainItem>>(emptyList()) }
    var hosts by remember { mutableStateOf<List<HostItem>>(emptyList()) }
    var menu by remember { mutableStateOf<HostChainItem?>(null) }
    var editing by remember { mutableStateOf<HostChainDraft?>(null) }
    var confirmDelete by remember { mutableStateOf<HostChainItem?>(null) }
    LaunchedEffect(vaultId, revision) {
        runCatching { shell.repo.read { hostChains(vaultId) to hosts(vaultId) } }
            .onSuccess { (c, h) -> items = c; hosts = h.filter { it.protocol.equals("ssh", true) } }
            .onFailure { shell.notify(it.userMessage()) }
    }
    fun write(block: suspend () -> Unit) {
        scope.launch { runCatching { block() }.onFailure { shell.notify(it.userMessage()) } }
    }

    RouteList(
        title = stringResource(R.string.jump_chains),
        onBack = onBack,
        icon = Icons.Filled.AccountTree,
        empty = items.isEmpty(),
        emptyTitle = stringResource(R.string.no_chains_yet),
        emptyHint = stringResource(R.string.jump_chains_hint),
        newLabel = stringResource(R.string.new_chain_title),
        onNew = vaultId?.let { v -> { editing = HostChainDraft(null, v, "", emptyList()) } },
        rows = items.map { c ->
            RouteRow(c.title(), c.hostLabels.joinToString(" → "), onClick = { editing = c.draft() }, onMenu = { menu = c })
        },
    )

    menu?.let { c ->
        ItemActionsSheet(
            title = c.title(),
            subtitle = c.hostLabels.joinToString(" → "),
            leading = { IconTile(Icons.Filled.AccountTree) },
            sections = listOf(
                SheetSection(
                    stringResource(R.string.host_chaining),
                    listOf(
                        SheetAction(Icons.Filled.Edit, stringResource(R.string.edit)) { editing = c.draft() },
                        SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.duplicate)) {
                            write { shell.repo.write { saveHostChain(c.draft().copy(id = null)) } }
                        },
                    ),
                ),
                SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true) { confirmDelete = c })),
            ),
            onClose = { if (menu == c) menu = null },
        )
    }
    editing?.let { d ->
        ChainEditorPage(
            initial = d,
            hosts = hosts,
            onSave = { out -> editing = null; write { shell.repo.write { saveHostChain(out) } } },
            onDelete = d.id?.let { id -> { editing = null; confirmDelete = items.firstOrNull { it.id == id } } },
            onDismiss = { editing = null },
        )
    }
    confirmDelete?.let { c ->
        ConfirmDialog(
            title = stringResource(R.string.delete_chain_title, c.title()),
            text = stringResource(R.string.delete_chain_text),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = null; write { shell.repo.write { deleteHostChain(c.id) } } },
            onDismiss = { confirmDelete = null },
        )
    }
}

private class RouteRow(val title: String, val subtitle: String?, val onClick: () -> Unit, val onMenu: () -> Unit)

@Composable
private fun RouteList(
    title: String,
    onBack: () -> Unit,
    icon: ImageVector,
    empty: Boolean,
    emptyTitle: String,
    emptyHint: String,
    newLabel: String,
    onNew: (() -> Unit)?,
    rows: List<RouteRow>,
) {
    SubScreen(
        title,
        onBack,
        floating = {
            if (onNew != null) FloatingActionButton(onClick = onNew) { Icon(Icons.Filled.Add, contentDescription = newLabel) }
        },
    ) { padding ->
        if (empty) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(title = emptyTitle, hint = emptyHint, icon = icon)
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    rows.forEachIndexed { i, r ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = r.title,
                            subtitle = r.subtitle,
                            leading = { IconTile(icon) },
                            modifier = Modifier.itemClicks(onClick = r.onClick, onMenu = r.onMenu),
                        ) {
                            ItemMoreButton(r.onMenu)
                        }
                    }
                }
            }
        }
    }
}
