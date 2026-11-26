package dev.labdex.labterm.ui.keychain

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Badge
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.TextButton
import androidx.compose.material3.Text
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
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.EmptyState
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.core.OidcProviderItem
import kotlinx.coroutines.launch

/** The selected vault's OpenID providers on a page of their own (team-wide in a team vault). */
@Composable
fun OidcProvidersScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var providers by remember { mutableStateOf<List<OidcProviderItem>?>(null) }
    var editing by remember { mutableStateOf<OidcProviderItem?>(null) }
    LaunchedEffect(vaultId, revision) {
        val v = vaultId ?: return@LaunchedEffect
        runCatching { shell.repo.read { oidcProviders(v) } }
            .onSuccess { providers = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    fun newProvider(): OidcProviderItem? = vaultId?.let { v ->
        OidcProviderItem(
            id = null,
            vaultId = v,
            label = "",
            issuer = "",
            clientId = "",
            clientSecret = null,
            scopes = emptyList(),
            isDefault = providers.isNullOrEmpty(),
            reuseSecs = null,
            expirationPolicy = "24h",
        )
    }

    SubScreen(
        stringResource(R.string.openid_providers),
        onBack,
        floating = {
            FloatingActionButton(onClick = { editing = newProvider() }) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.add))
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
                        ChevronRow(
                            title = p.label.ifBlank { p.issuer },
                            subtitle = p.issuer + if (p.isDefault) " · " + str(R.string.openid_default) else "",
                            leading = { IconTile(Icons.Filled.Badge) },
                            modifier = Modifier.clickable { editing = p },
                        )
                    }
                }
            }
        }
    }

    editing?.let { p ->
        val id = p.id
        ProviderEditDialog(
            p,
            onDismiss = { editing = null },
            onDelete = if (id == null) null else ({
                scope.launch {
                    runCatching { shell.repo.write { deleteOidcProvider(id) } }
                        .onSuccess { editing = null }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            }),
        ) { saved ->
            scope.launch {
                runCatching { shell.repo.write { saveOidcProvider(saved) } }
                    .onSuccess { editing = null }
                    .onFailure { shell.notify(it.userMessage()) }
            }
        }
    }
}
