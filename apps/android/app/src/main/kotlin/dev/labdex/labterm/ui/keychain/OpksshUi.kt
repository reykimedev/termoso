package dev.labdex.labterm.ui.keychain

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
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
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.labdex.labterm.R
import dev.labdex.labterm.data.KeyExportBridge
import dev.labdex.labterm.data.KeyExportState
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.hosts.FullScreenEditor
import dev.labdex.labterm.ui.components.SecretField
import dev.labdex.labterm.ui.components.SwitchRow
import dev.labdex.labterm.ui.terminal.PromptDialog
import dev.labdex.labterm.core.HostItem
import dev.labdex.labterm.core.KeyExport
import dev.labdex.labterm.core.KeyItem
import dev.labdex.labterm.core.OidcKeyChoice
import dev.labdex.labterm.core.OidcProviderItem
import dev.labdex.labterm.ui.hosts.NONE_ID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** opkssh keys hold no key material; Rust names their type "opkssh". */
fun KeyItem.isOpkssh(): Boolean = keyType == "opkssh"

private fun words(text: String): List<String> =
    text.split(',', ' ', '\n').map { it.trim() }.filter { it.isNotEmpty() }

/** One option per provider plus "vault default"; null id = the default. */
@Composable
private fun ProviderChoice(providers: List<OidcProviderItem>, selected: String?, onSelect: (String?) -> Unit) {
    Column {
        val default = providers.firstOrNull { it.isDefault } ?: providers.singleOrNull()
        ChoiceRow(
            stringResource(R.string.openid_vault_default) + (default?.let { " (${it.label})" } ?: ""),
            selected == null,
        ) { onSelect(null) }
        providers.forEach { p -> ChoiceRow(p.label, selected == p.id) { onSelect(p.id) } }
    }
}

@Composable
private fun ChoiceRow(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected = selected, onClick = onClick)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
private fun rememberProviders(repo: VaultRepository, vaultId: String, reload: Int = 0): List<OidcProviderItem>? {
    val list by produceState<List<OidcProviderItem>?>(null, vaultId, reload) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { oidcProviders(vaultId) } }.getOrDefault(emptyList()) }
    }
    return list
}

/** Create an OpenID (opkssh) key in [vaultId]: a label and a provider, nothing secret. */
@Composable
fun NewOpksshKeyDialog(
    repo: VaultRepository,
    vaultId: String,
    onNotify: (String) -> Unit,
    onCreated: (String) -> Unit,
    onManageProviders: () -> Unit,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val providers = rememberProviders(repo, vaultId)
    var label by remember { mutableStateOf("") }
    var provider by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(stringResource(R.string.openid_key)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.openid_key_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                FormField(label, { label = it }, stringResource(R.string.label))
                when {
                    providers == null -> CircularProgressIndicator(Modifier.size(20.dp))
                    providers.isEmpty() -> Text(stringResource(R.string.openid_no_providers), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    else -> ProviderChoice(providers, provider) { provider = it }
                }
                TextButton(onClick = onManageProviders) { Text(stringResource(R.string.openid_providers)) }
            }
        },
        confirmButton = {
            TextButton(enabled = !busy && label.isNotBlank(), onClick = {
                busy = true
                scope.launch {
                    runCatching { repo.write { createOpksshKey(vaultId, label.trim(), OidcKeyChoice(provider, null, null, emptyList(), null)) } }
                        .onSuccess { onCreated(it.id) }
                        .onFailure { busy = false; onNotify(it.userMessage()) }
                }
            }) { Text(stringResource(R.string.create)) }
        },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Provider and per-key overrides of an opkssh key. */
@Composable
fun OidcKeyChoiceDialog(
    repo: VaultRepository,
    key: KeyItem,
    current: OidcKeyChoice?,
    onDismiss: () -> Unit,
    onSave: (OidcKeyChoice) -> Unit,
) {
    val providers = rememberProviders(repo, key.vaultId)
    var provider by remember { mutableStateOf(current?.providerId) }
    var issuer by remember { mutableStateOf(current?.issuer.orEmpty()) }
    var clientId by remember { mutableStateOf(current?.clientId.orEmpty()) }
    var principals by remember { mutableStateOf(current?.principals.orEmpty().joinToString(", ")) }
    var account by remember { mutableStateOf(current?.account.orEmpty()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.openid_provider)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (providers == null) CircularProgressIndicator(Modifier.size(20.dp)) else ProviderChoice(providers, provider) { provider = it }
                Text(stringResource(R.string.openid_overrides_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                FormField(issuer, { issuer = it }, stringResource(R.string.openid_issuer))
                FormField(clientId, { clientId = it }, stringResource(R.string.openid_client_id))
                FormField(principals, { principals = it }, stringResource(R.string.openid_principals))
                FormField(account, { account = it }, stringResource(R.string.openid_account), placeholder = "alice-admin")
                Text(stringResource(R.string.openid_account_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
        confirmButton = {
            TextButton(onClick = {
                onSave(
                    OidcKeyChoice(
                        provider,
                        issuer.trim().ifEmpty { null },
                        clientId.trim().ifEmpty { null },
                        words(principals),
                        account.trim().ifEmpty { null },
                    ),
                )
            }) { Text(stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/**
 * The OpenID providers of [vaultId] (team-wide in a team vault) as a full-screen page over the
 * current screen, for flows that start from a key or the "new OpenID key" dialog.
 */
@Composable
fun OidcProvidersDialog(repo: VaultRepository, vaultId: String, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        OidcProvidersPage(repo, vaultId, onNotify, onBack = onDismiss)
    }
}

private val POLICIES = listOf("24h", "48h", "1week", "oidc", "oidc-refreshed", "never")

/** Full-screen editor of an OpenID provider, in the shared edit page layout. */
@Composable
internal fun ProviderEditorPage(
    current: OidcProviderItem,
    onDismiss: () -> Unit,
    onDelete: (() -> Unit)?,
    onSave: (OidcProviderItem) -> Unit,
) {
    var label by remember { mutableStateOf(current.label) }
    var issuer by remember { mutableStateOf(current.issuer) }
    var clientId by remember { mutableStateOf(current.clientId) }
    var secret by remember { mutableStateOf(current.clientSecret.orEmpty()) }
    var scopes by remember { mutableStateOf(current.scopes.joinToString(" ")) }
    var policy by remember { mutableStateOf(current.expirationPolicy ?: "24h") }
    var reuseHours by remember { mutableStateOf(current.reuseSecs?.let { (it / 3600u).toString() }.orEmpty()) }
    var isDefault by remember { mutableStateOf(current.isDefault) }
    val reuse = reuseHours.trim().takeIf { it.isNotEmpty() }?.toULongOrNull()
    val valid = issuer.isNotBlank() && clientId.isNotBlank() && (reuseHours.isBlank() || reuse != null)
    FullScreenEditor(
        title = if (current.id == null) stringResource(R.string.openid_new_provider) else stringResource(R.string.openid_edit_provider),
        canSave = valid,
        onSave = {
            onSave(
                current.copy(
                    label = label.trim(),
                    issuer = issuer.trim(),
                    clientId = clientId.trim(),
                    clientSecret = secret.trim().ifEmpty { null },
                    scopes = words(scopes),
                    isDefault = isDefault,
                    reuseSecs = reuse?.let { it * 3600u },
                    expirationPolicy = policy,
                ),
            )
        },
        onDismiss = onDismiss,
        onDelete = onDelete,
    ) {
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(label, { label = it }, stringResource(R.string.label))
                FormField(issuer, { issuer = it }, stringResource(R.string.openid_issuer), placeholder = "https://auth.example.com/")
                FormField(clientId, { clientId = it }, stringResource(R.string.openid_client_id))
                SecretField(secret, { secret = it }, stringResource(R.string.openid_client_secret))
                FormField(scopes, { scopes = it }, stringResource(R.string.openid_scopes))
            }
        }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(stringResource(R.string.openid_expiration_policy), style = MaterialTheme.typography.labelMedium)
                POLICIES.forEach { p -> ChoiceRow(p, policy == p) { policy = p } }
                Spacer(Modifier.size(8.dp))
                FormField(reuseHours, { reuseHours = it }, stringResource(R.string.openid_reuse_hours))
            }
        }
        SectionCard {
            SwitchRow(title = stringResource(R.string.openid_default_for_vault), checked = isDefault, onCheckedChange = { isDefault = it })
        }
    }
}

/** Pick a host, then install opkssh there and allow the signed-in identity, like "Export key to host". */
@Composable
fun SetupOpksshOnHostFlow(repo: VaultRepository, key: KeyItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    var host by remember { mutableStateOf<HostItem?>(null) }
    val hosts by produceState<List<HostItem>?>(null, key.vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { hosts(key.vaultId) } }.getOrDefault(emptyList()).filter { it.protocol == "ssh" } }
    }
    val picked = host
    if (picked == null) {
        PickerDialog(
            title = stringResource(R.string.setup_opkssh_on_host),
            hint = stringResource(R.string.setup_opkssh_on_host_hint),
            empty = stringResource(R.string.no_hosts_in_this_vault),
            items = hosts,
            label = { it.label },
            onPick = { host = it },
            onDismiss = onDismiss,
        )
    } else {
        ConfirmOpksshSetup(repo, key, picked, onNotify, onDismiss)
    }
}

/** From a host's menu: pick one of the vault's OpenID keys, then set opkssh up on [host]. */
@Composable
fun SetupOpksshForHostFlow(repo: VaultRepository, host: HostItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    var key by remember { mutableStateOf<KeyItem?>(null) }
    val keys by produceState<List<KeyItem>?>(null, host.vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { keys(host.vaultId) } }.getOrDefault(emptyList()).filter { it.isOpkssh() } }
    }
    val picked = key
    if (picked == null) {
        PickerDialog(
            title = stringResource(R.string.setup_opkssh_on_host),
            hint = stringResource(R.string.setup_opkssh_on_host_hint),
            empty = stringResource(R.string.openid_no_keys),
            items = keys,
            label = { it.label },
            onPick = { key = it },
            onDismiss = onDismiss,
        )
    } else {
        ConfirmOpksshSetup(repo, picked, host, onNotify, onDismiss)
    }
}

@Composable
private fun ConfirmOpksshSetup(repo: VaultRepository, key: KeyItem, host: HostItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    var started by remember { mutableStateOf(false) }
    var principal by remember { mutableStateOf("") }
    var identity by remember { mutableStateOf("") }
    if (started) {
        OpksshSetupDialog(
            repo,
            key.id,
            host.id,
            key.vaultId,
            principal.trim().ifEmpty { null },
            identity.trim().ifEmpty { null },
            onDone = onNotify,
            onDismiss = onDismiss,
        )
        return
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.setup_opkssh_on_host)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.setup_opkssh_confirm, host.label), style = MaterialTheme.typography.bodySmall)
                FormField(principal, { principal = it }, stringResource(R.string.openid_login_user))
                FormField(identity, { identity = it }, stringResource(R.string.openid_identity))
            }
        },
        confirmButton = { TextButton(onClick = { started = true }) { Text(stringResource(R.string.openid_set_up)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** The OpenID key [host] tries before its credentials (its own setting; groups' are inherited). */
@Composable
fun HostOidcKeyDialog(repo: VaultRepository, host: HostItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    val keys by produceState<List<KeyItem>?>(null, host.vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { keys(host.vaultId) } }.getOrDefault(emptyList()).filter { it.isOpkssh() } }
    }
    val current by produceState<String?>(null, host.id) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { hostOidcKey(host.id) } }.getOrNull() }
    }
    var choice by remember(current) { mutableStateOf(current) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.openid_key_for_host)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.openid_key_for_host_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                val list = keys
                if (list == null) {
                    CircularProgressIndicator(Modifier.size(20.dp))
                } else {
                    ChoiceRow(stringResource(R.string.openid_none_inherit), choice == null) { choice = null }
                    ChoiceRow(stringResource(R.string.openid_none_explicit), choice == NONE_ID) { choice = NONE_ID }
                    list.forEach { k -> ChoiceRow(k.label, choice == k.id) { choice = k.id } }
                    if (list.isEmpty()) Text(stringResource(R.string.openid_no_keys), color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val picked = choice
                scope.launch {
                    runCatching { repo.write { setHostOidcKey(host.id, picked) } }
                        .onSuccess { onNotify(str(R.string.openid_key_for_host_saved)); onDismiss() }
                        .onFailure { onNotify(it.userMessage()) }
                }
            }) { Text(stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun OpksshSetupDialog(
    repo: VaultRepository,
    keyId: String,
    hostId: String,
    vaultId: String,
    principal: String?,
    identity: String?,
    onDone: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val bridge = remember { KeyExportBridge() }
    var rust by remember { mutableStateOf<KeyExport?>(null) }
    LaunchedEffect(keyId, hostId) {
        rust = withContext(Dispatchers.IO) {
            runCatching { repo.read { startOpksshSetup(keyId, hostId, vaultId, principal, identity, bridge) } }
                .onFailure { bridge.onFailed(it.message ?: it.toString()) }
                .getOrNull()
        }
    }
    DisposableEffect(Unit) { onDispose { rust?.cancel() } }

    val state by bridge.state.collectAsState()
    val prompt by bridge.prompt.collectAsState()
    val ready = stringResource(R.string.opkssh_set_up_on)
    LaunchedEffect(state) {
        val s = state
        if (s is KeyExportState.Done) {
            onDone(ready + " " + s.result.target)
            onDismiss()
        }
    }
    prompt?.let { p ->
        PromptDialog(p) { answer ->
            bridge.promptAnswered(p.id)
            rust?.answer(p.id, answer)
        }
    }
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(R.string.setup_opkssh_on_host)) },
        text = {
            when (val s = state) {
                is KeyExportState.Running -> Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(12.dp))
                    Text(s.detail.ifEmpty { stringResource(R.string.connecting) })
                }
                is KeyExportState.Failed -> Text(s.message, color = MaterialTheme.colorScheme.error)
                is KeyExportState.Done -> Text(s.result.target)
            }
        },
        confirmButton = {
            if (state is KeyExportState.Failed) TextButton(onClick = onDismiss) { Text(stringResource(R.string.close)) }
        },
        dismissButton = {
            if (state is KeyExportState.Running) TextButton(onClick = { rust?.cancel(); onDismiss() }) { Text(stringResource(R.string.cancel)) }
        },
    )
}
