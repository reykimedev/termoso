package dev.labdex.labterm.ui.keychain

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Badge
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.OidcProviderItem
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
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
import dev.labdex.labterm.ui.hosts.ConfirmDialog
import dev.labdex.labterm.ui.shell.ShellViewModel
import kotlinx.coroutines.launch

/** The selected vault's OpenID providers on a page of their own (team-wide in a team vault). */
@Composable
fun OidcProvidersScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val v = vaultId
    if (v == null) {
        SubScreen(stringResource(R.string.openid_providers), onBack) { }
        return
    }
    OidcProvidersPage(shell.repo, v, shell::notify, onBack)
}

private fun OidcProviderItem.title() = label.ifBlank { issuer }

private fun OidcProviderItem.subtitle() = issuer + if (isDefault) " · " + str(R.string.openid_default) else ""

/**
 * The OpenID providers of [vaultId]: ⋯ or long-press for Edit, Duplicate and Delete, tap to edit
 * on the full-screen page, + for a new one. Same pattern as proxies and jump chains.
 */
@Composable
fun OidcProvidersPage(repo: VaultRepository, vaultId: String, onNotify: (String) -> Unit, onBack: () -> Unit) {
    val revision by repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var providers by remember { mutableStateOf<List<OidcProviderItem>?>(null) }
    var menu by remember { mutableStateOf<OidcProviderItem?>(null) }
    var editing by remember { mutableStateOf<OidcProviderItem?>(null) }
    var confirmDelete by remember { mutableStateOf<OidcProviderItem?>(null) }
    LaunchedEffect(vaultId, revision) {
        runCatching { repo.read { oidcProviders(vaultId) } }
            .onSuccess { providers = it }
            .onFailure { onNotify(it.userMessage()) }
    }
    fun write(block: suspend () -> Unit) {
        scope.launch { runCatching { block() }.onFailure { onNotify(it.userMessage()) } }
    }

    fun newProvider() = OidcProviderItem(
        id = null,
        vaultId = vaultId,
        label = "",
        issuer = "",
        clientId = "",
        clientSecret = null,
        scopes = emptyList(),
        isDefault = providers.isNullOrEmpty(),
        reuseSecs = null,
        expirationPolicy = "24h",
    )

    SubScreen(
        stringResource(R.string.openid_providers),
        onBack,
        floating = {
            FloatingActionButton(onClick = { editing = newProvider() }) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.openid_new_provider))
            }
        },
    ) { padding ->
        val list = providers
        when {
            list == null -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            list.isEmpty() -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.openid_no_providers),
                    hint = stringResource(R.string.openid_providers_hint),
                    icon = Icons.Filled.Badge,
                    action = { TextButton(onClick = { editing = newProvider() }) { Text(stringResource(R.string.openid_new_provider)) } },
                )
            }
            else -> Column(
                Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
            ) {
                SectionCard {
                    list.forEachIndexed { i, p ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = p.title(),
                            subtitle = p.subtitle(),
                            leading = { IconTile(Icons.Filled.Badge) },
                            modifier = Modifier.itemClicks(onClick = { editing = p }, onMenu = { menu = p }),
                        ) {
                            ItemMoreButton({ menu = p })
                        }
                    }
                }
            }
        }
    }

    menu?.let { p ->
        ItemActionsSheet(
            title = p.title(),
            subtitle = p.subtitle(),
            leading = { IconTile(Icons.Filled.Badge) },
            sections = listOf(
                SheetSection(
                    stringResource(R.string.openid_provider),
                    listOf(
                        SheetAction(Icons.Filled.Edit, stringResource(R.string.edit)) { editing = p },
                        SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.duplicate)) {
                            write { repo.write { saveOidcProvider(p.copy(id = null, isDefault = false)) } }
                        },
                    ),
                ),
                SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true) { confirmDelete = p })),
            ),
            onClose = { if (menu == p) menu = null },
        )
    }
    editing?.let { p ->
        val id = p.id
        ProviderEditorPage(
            p,
            onDismiss = { editing = null },
            onDelete = if (id == null) null else ({ editing = null; confirmDelete = p }),
        ) { saved ->
            editing = null
            write { repo.write { saveOidcProvider(saved) } }
        }
    }
    confirmDelete?.let { p ->
        val id = p.id ?: return@let
        ConfirmDialog(
            title = stringResource(R.string.openid_delete_provider_title, p.title()),
            text = stringResource(R.string.openid_delete_provider_text),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = null; write { repo.write { deleteOidcProvider(id) } } },
            onDismiss = { confirmDelete = null },
        )
    }
}
