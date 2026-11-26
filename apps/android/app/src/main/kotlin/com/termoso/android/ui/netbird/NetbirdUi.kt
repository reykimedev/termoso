package com.termoso.android.ui.netbird

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Hub
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.TransportItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val NETBIRD = "netbird"

/**
 * The selected vault's NetBird networks (team-wide in a team vault). Hosts set to use
 * one are reached through a NetBird client inside the app, started on demand: no VPN.
 */
@Composable
fun NetbirdNetworksScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var networks by remember { mutableStateOf<List<TransportItem>?>(null) }
    var editing by remember { mutableStateOf<TransportItem?>(null) }
    LaunchedEffect(vaultId, revision) {
        val v = vaultId ?: return@LaunchedEffect
        runCatching { shell.repo.read { transports(v) } }
            .onSuccess { networks = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    fun newNetwork(): TransportItem? = vaultId?.let { v ->
        TransportItem(
            id = null,
            vaultId = v,
            label = "",
            provider = NETBIRD,
            managementUrl = null,
            setupKey = null,
            hasSetupKey = false,
        )
    }

    SubScreen(
        stringResource(R.string.netbird_networks),
        onBack,
        floating = {
            FloatingActionButton(onClick = { editing = newNetwork() }) {
                Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.add))
            }
        },
    ) { padding ->
        val list = networks
        when {
            list == null -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            list.isEmpty() -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.netbird_no_networks),
                    hint = stringResource(R.string.netbird_networks_hint),
                    icon = Icons.Filled.Hub,
                    action = { TextButton(onClick = { editing = newNetwork() }) { Text(stringResource(R.string.netbird_new_network)) } },
                )
            }
            else -> Column(
                Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
            ) {
                SectionCard {
                    list.forEachIndexed { i, n ->
                        if (i > 0) RowDivider()
                        ChevronRow(
                            title = n.label,
                            subtitle = n.managementUrl ?: str(R.string.netbird_cloud),
                            leading = { IconTile(Icons.Filled.Hub) },
                            modifier = Modifier.clickable { editing = n },
                        )
                    }
                }
                Text(
                    stringResource(R.string.netbird_networks_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp, start = 4.dp, end = 4.dp),
                )
            }
        }
    }

    editing?.let { n ->
        val id = n.id
        NetworkEditDialog(
            n,
            onDismiss = { editing = null },
            onDelete = if (id == null) null else ({
                scope.launch {
                    runCatching { shell.repo.write { deleteTransport(id) } }
                        .onSuccess { editing = null }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            }),
        ) { saved ->
            scope.launch {
                runCatching { shell.repo.write { saveTransport(saved) } }
                    .onSuccess { editing = null }
                    .onFailure { shell.notify(it.userMessage()) }
            }
        }
    }
}

@Composable
private fun NetworkEditDialog(
    current: TransportItem,
    onDismiss: () -> Unit,
    onDelete: (() -> Unit)?,
    onSave: (TransportItem) -> Unit,
) {
    var label by remember { mutableStateOf(current.label) }
    var url by remember { mutableStateOf(current.managementUrl.orEmpty()) }
    var setupKey by remember { mutableStateOf("") }
    // A stored key is never shown again: leaving the field empty keeps it.
    val valid = label.isNotBlank() && (current.hasSetupKey || setupKey.isNotBlank())
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (current.id == null) stringResource(R.string.netbird_new_network) else current.label) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(label, { label = it }, stringResource(R.string.label))
                FormField(
                    url,
                    { url = it },
                    stringResource(R.string.netbird_management_url),
                    placeholder = "https://netbird.example.com",
                )
                SecretField(
                    setupKey,
                    { setupKey = it },
                    stringResource(R.string.netbird_setup_key),
                    placeholder = if (current.hasSetupKey) str(R.string.netbird_setup_key_stored) else null,
                )
                Text(
                    stringResource(R.string.netbird_setup_key_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (onDelete != null) {
                    TextButton(onClick = onDelete) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = valid, onClick = {
                onSave(
                    current.copy(
                        label = label.trim(),
                        provider = NETBIRD,
                        managementUrl = url.trim().ifEmpty { null },
                        setupKey = setupKey.trim().ifEmpty { null },
                    ),
                )
            }) { Text(stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun ChoiceRow(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected = selected, onClick = onClick)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

/**
 * Pick how a host or group is reached: directly (or, for a host, as its group says) or
 * through one of the vault's NetBird networks.
 */
@Composable
fun TunnelDialog(
    repo: VaultRepository,
    vaultId: String,
    title: String,
    forGroup: Boolean,
    load: suspend () -> String?,
    save: suspend (String?) -> Unit,
    onNotify: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val networks by produceState<List<TransportItem>?>(null, vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { transports(vaultId) } }.getOrDefault(emptyList()) }
    }
    var loaded by remember { mutableStateOf(false) }
    var choice by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        choice = runCatching { load() }.getOrNull()
        loaded = true
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(if (forGroup) R.string.tunnel_group_hint else R.string.tunnel_host_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val list = networks
                if (list == null || !loaded) {
                    CircularProgressIndicator(Modifier.size(20.dp))
                } else {
                    ChoiceRow(
                        stringResource(if (forGroup) R.string.tunnel_direct else R.string.tunnel_direct_or_group),
                        choice == null,
                    ) { choice = null }
                    list.forEach { n -> ChoiceRow(str(R.string.tunnel_via_netbird, n.label), choice == n.id) { choice = n.id } }
                    if (list.isEmpty()) {
                        Text(stringResource(R.string.netbird_no_networks_add_in_vault), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = loaded, onClick = {
                val picked = choice
                scope.launch {
                    runCatching { save(picked) }
                        .onSuccess { onNotify(str(R.string.tunnel_saved)); onDismiss() }
                        .onFailure { onNotify(it.userMessage()) }
                }
            }) { Text(stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
