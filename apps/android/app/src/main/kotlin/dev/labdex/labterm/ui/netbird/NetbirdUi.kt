package dev.labdex.labterm.ui.netbird

import android.content.ActivityNotFoundException
import android.content.Context
import android.net.Uri
import android.util.Log
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Login
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Hub
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.LinkOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TransportItem
import dev.labdex.labterm.core.TransportState
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.service.VpnSignIns
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.EmptyState
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.PickerRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SecretField
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.hosts.FullScreenEditor
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.components.SwitchRow
import dev.labdex.labterm.ui.components.itemClicks
import dev.labdex.labterm.ui.hosts.vpnProviderName
import dev.labdex.labterm.ui.shell.ShellViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val NETBIRD = "netbird"
private const val TAILSCALE = "tailscale"

/** The network's server, or its provider's cloud when it names none. */
internal fun serverLabel(n: TransportItem): String =
    n.managementUrl ?: if (n.provider == TAILSCALE) vpnProviderName(TAILSCALE) else str(R.string.netbird_cloud)

/**
 * The selected vault's NetBird networks. Hosts set to use one are reached through a
 * NetBird client inside the app, started on demand: no VPN. A team vault shares only
 * the network settings; each member signs in to NetBird as themselves.
 */
@Composable
fun NetbirdNetworksScreen(shell: ShellViewModel, onBack: () -> Unit, onOpenNetwork: (String) -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var networks by remember { mutableStateOf<List<TransportItem>?>(null) }
    var editing by remember { mutableStateOf<TransportItem?>(null) }
    var signInOnly by remember { mutableStateOf(false) }
    var signingIn by remember { mutableStateOf<TransportItem?>(null) }
    // The quick menu (long-press or ⋯), like hosts and groups.
    var menu by remember { mutableStateOf<TransportItem?>(null) }
    val ids = networks.orEmpty().mapNotNull { it.id }
    val states by remember(ids) { VpnStates.observe(shell.repo, ids) }.collectAsStateWithLifecycle(emptyMap())

    fun connect(n: TransportItem) {
        val id = n.id ?: return
        scope.launch {
            runCatching { shell.repo.read { transportConnect(id) } }
                .onSuccess { shell.notify(str(R.string.netbird_connected_to, n.label)) }
                .onFailure { shell.notify(it.userMessage()) }
        }
    }

    LaunchedEffect(vaultId, revision) {
        val v = vaultId ?: return@LaunchedEffect
        runCatching { shell.repo.read { transports(v) to transportSignInOnly(v) } }
            .onSuccess { (list, team) -> networks = list; signInOnly = team }
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
            team = signInOnly,
            signedIn = false,
            account = null,
            directFirst = false,
            idleSecs = null,
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
                    hint = stringResource(if (signInOnly) R.string.netbird_networks_hint_team else R.string.netbird_networks_hint),
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
                        val state = n.id?.let { states[it] }
                        ListRow(
                            title = n.label,
                            subtitle = serverLabel(n) + " · " + (
                                state?.label() ?: str(if (n.signedIn || n.hasSetupKey) R.string.vpn_status_off else R.string.netbird_not_signed_in)
                            ),
                            leading = {
                                Box {
                                    IconTile(Icons.Filled.Hub)
                                    if (state != null) {
                                        Box(
                                            Modifier.align(Alignment.BottomEnd).size(10.dp).clip(CircleShape)
                                                .background(state.color()),
                                        )
                                    }
                                }
                            },
                            // A tap opens the network's details: state, peers, hosts, join and sign-in.
                            modifier = Modifier.itemClicks(onClick = { n.id?.let(onOpenNetwork) }, onMenu = { menu = n }),
                        ) {
                            if (state is TransportState.Connecting) CircularProgressIndicator(Modifier.size(20.dp))
                            ItemMoreButton(onClick = { menu = n })
                        }
                    }
                }
                Text(
                    stringResource(if (signInOnly) R.string.netbird_networks_hint_team else R.string.netbird_networks_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp, start = 4.dp, end = 4.dp),
                )
            }
        }
    }

    editing?.let { n ->
        val id = n.id
        NetworkEditorPage(
            n,
            onDismiss = { editing = null },
            onSignIn = if (id == null || n.hasSetupKey) null else ({ editing = null; signingIn = n }),
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

    menu?.let { n ->
        val id = n.id
        val state = id?.let { states[it] }
        ItemActionsSheet(
            title = n.label,
            subtitle = serverLabel(n) + (state?.let { " · " + it.label() } ?: ""),
            leading = { IconTile(Icons.Filled.Hub) },
            onClose = { menu = null },
            sections = listOf(
                SheetSection(
                    null,
                    listOfNotNull(
                        id?.let { SheetAction(Icons.Filled.Info, str(R.string.vpn_details)) { onOpenNetwork(it) } },
                        if (id != null && state is TransportState.Connected) {
                            SheetAction(Icons.Filled.LinkOff, str(R.string.vpn_disconnect)) {
                                scope.launch {
                                    runCatching { shell.repo.read { transportDisconnect(id) } }.onFailure { shell.notify(it.userMessage()) }
                                }
                            }
                        } else if (id != null && state !is TransportState.Unavailable) {
                            SheetAction(Icons.Filled.Link, str(R.string.vpn_connect), enabled = state !is TransportState.Connecting) {
                                if (!n.hasSetupKey && !n.signedIn) signingIn = n else connect(n)
                            }
                        } else {
                            null
                        },
                        if (id != null && !n.hasSetupKey) {
                            SheetAction(Icons.AutoMirrored.Filled.Login, str(if (n.signedIn) R.string.netbird_sign_in_again else R.string.netbird_sign_in)) { signingIn = n }
                        } else {
                            null
                        },
                        SheetAction(Icons.Filled.Edit, str(R.string.edit)) { editing = n },
                    ),
                ),
                SheetSection(
                    null,
                    listOfNotNull(
                        id?.let {
                            SheetAction(Icons.Filled.Delete, str(R.string.delete), danger = true) {
                                scope.launch {
                                    runCatching { shell.repo.write { deleteTransport(it) } }.onFailure { e -> shell.notify(e.userMessage()) }
                                }
                            }
                        },
                    ),
                ),
            ),
        )
    }

    signingIn?.let { n ->
        NetbirdSignInDialog(n, shell.repo, shell::notify) { ok ->
            signingIn = null
            if (ok) connect(n)
        }
    }
}

/** Full-screen editor of a VPN network, in the shared edit page layout. */
@Composable
internal fun NetworkEditorPage(
    current: TransportItem,
    onDismiss: () -> Unit,
    onSignIn: (() -> Unit)?,
    onDelete: (() -> Unit)?,
    onSave: (TransportItem) -> Unit,
) {
    var label by remember { mutableStateOf(current.label) }
    var provider by remember { mutableStateOf(current.provider.ifEmpty { NETBIRD }) }
    val tailscale = provider == TAILSCALE
    var url by remember { mutableStateOf(current.managementUrl.orEmpty()) }
    var setupKey by remember { mutableStateOf("") }
    var directFirst by remember { mutableStateOf(current.directFirst) }
    var idleSecs by remember { mutableStateOf(current.idleSecs) }
    // A stored key is never shown again: leaving the field empty keeps it. Without
    // a key, each user joins by signing in.
    val valid = label.isNotBlank()
    FullScreenEditor(
        title = stringResource(if (current.id == null) R.string.vpn_new_network else R.string.vpn_edit_network),
        canSave = valid,
        onSave = {
            onSave(
                current.copy(
                    label = label.trim(),
                    provider = provider,
                    managementUrl = url.trim().ifEmpty { null },
                    setupKey = if (current.team) null else setupKey.trim().ifEmpty { null },
                    directFirst = directFirst,
                    idleSecs = idleSecs,
                ),
            )
        },
        onDismiss = onDismiss,
        onDelete = onDelete,
    ) {
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(label, { label = it }, stringResource(R.string.label))
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    listOf(NETBIRD, TAILSCALE).forEachIndexed { i, p ->
                        SegmentedButton(
                            selected = provider == p,
                            onClick = { provider = p },
                            shape = SegmentedButtonDefaults.itemShape(i, 2),
                        ) { Text(vpnProviderName(p)) }
                    }
                }
                FormField(
                    url,
                    { url = it },
                    stringResource(if (tailscale) R.string.tailscale_control_url else R.string.netbird_management_url),
                    placeholder = if (tailscale) "https://headscale.example.com" else "https://netbird.example.com",
                )
            }
        }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (current.team) {
                    // A key here would let every vault member in without a NetBird account.
                    Text(
                        stringResource(R.string.netbird_team_sign_in_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    SecretField(
                        setupKey,
                        { setupKey = it },
                        stringResource(if (tailscale) R.string.tailscale_auth_key_optional else R.string.netbird_setup_key_optional),
                        placeholder = if (current.hasSetupKey) str(R.string.netbird_setup_key_stored) else null,
                    )
                    Text(
                        stringResource(if (tailscale) R.string.tailscale_auth_key_hint else R.string.netbird_setup_key_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (onSignIn != null) {
                    TextButton(onClick = onSignIn) {
                        Text(stringResource(if (current.signedIn) R.string.netbird_sign_in_again else R.string.netbird_sign_in))
                    }
                }
            }
        }
        SectionCard {
            SwitchRow(
                title = stringResource(R.string.vpn_direct_first),
                subtitle = stringResource(R.string.vpn_direct_first_hint),
                checked = directFirst,
                onCheckedChange = { directFirst = it },
            )
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                val choices = IDLE_CHOICES.let { if (idleSecs in it) it else it + idleSecs }
                PickerRow(
                    label = stringResource(R.string.vpn_idle),
                    value = idleChoiceLabel(idleSecs),
                    options = choices.map { it?.toString() to idleChoiceLabel(it) },
                    selected = idleSecs?.toString(),
                    onPick = { idleSecs = it?.toUIntOrNull() },
                    empty = null,
                )
                Text(
                    stringResource(R.string.vpn_idle_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** How long a network may stay up without a connection, in seconds; null = the default
 *  (2 minutes), 0 = until disconnected. The desktop editor offers the same. */
internal val IDLE_CHOICES: List<UInt?> = listOf(30u, null, 600u, 3600u, 0u)

@Composable
internal fun idleChoiceLabel(secs: UInt?): String = when (secs) {
    null -> stringResource(R.string.vpn_idle_default)
    0u -> stringResource(R.string.vpn_idle_forever)
    30u -> stringResource(R.string.vpn_idle_30s)
    600u -> stringResource(R.string.vpn_idle_10min)
    3600u -> stringResource(R.string.vpn_idle_1h)
    else -> if (secs < 120u) {
        stringResource(R.string.vpn_idle_seconds, secs.toInt())
    } else {
        stringResource(R.string.vpn_idle_minutes, (secs / 60u).toInt())
    }
}

/** Opens [url] in a Custom Tab (falls back to the default browser); false when no browser exists. */
private fun openBrowser(context: Context, url: String): Boolean =
    try {
        CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(context, Uri.parse(url))
        true
    } catch (_: ActivityNotFoundException) {
        false
    }

/**
 * Sign in to a NetBird network with the user's own account at the identity provider the
 * NetBird server uses (Authentik, ...). The browser comes back to a short-lived listener on
 * localhost; this device then joins as the user's own peer. Calls [onDone] with true once
 * signed in, false when cancelled or failed.
 */
@Composable
fun NetbirdSignInDialog(
    network: TransportItem,
    repo: VaultRepository,
    onNotify: (String) -> Unit,
    onDone: (Boolean) -> Unit,
) {
    val id = network.id ?: return
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var waiting by remember { mutableStateOf(false) }
    // Reaching the NetBird server can take a while; without this the button looked dead.
    var starting by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var url by remember { mutableStateOf<String?>(null) }
    var account by remember { mutableStateOf(network.account.orEmpty()) }

    fun cancel() {
        // onDone closes the dialog and cancels its scope: start right away and finish regardless.
        if (waiting) {
            scope.launch(start = CoroutineStart.UNDISPATCHED) {
                withContext(NonCancellable) { repo.read { transportSignInCancel(id) } }
            }
        }
        onDone(false)
    }

    AlertDialog(
        onDismissRequest = ::cancel,
        title = { Text(str(R.string.netbird_sign_in_to, network.label)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    stringResource(
                        when {
                            waiting -> R.string.netbird_sign_in_waiting
                            starting -> R.string.netbird_sign_in_starting
                            else -> R.string.netbird_sign_in_hint
                        },
                    ),
                )
                if (waiting || starting) {
                    CircularProgressIndicator(Modifier.size(20.dp))
                } else {
                    FormField(account, { account = it }, stringResource(R.string.netbird_account), placeholder = "alice-admin")
                    Text(stringResource(R.string.netbird_account_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                // Shown here, not only as a snackbar, which sits behind the dialog.
                error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
            }
        },
        confirmButton = {
            if (waiting) {
                TextButton(onClick = { url?.let { openBrowser(context, it) } }) { Text(stringResource(R.string.netbird_reopen_browser)) }
            } else {
                TextButton(enabled = !starting, onClick = {
                    error = null
                    starting = true
                    scope.launch {
                        try {
                            // Keeps the process in the foreground while the browser is in
                            // front: the sign-in's localhost listener lives here.
                            VpnSignIns.add(id, str(R.string.notif_vpn_sign_in, network.label), network.managementUrl ?: "")
                            val u = try {
                                repo.read { transportSignInBegin(id, account.trim().ifEmpty { null }) }
                            } finally {
                                starting = false
                            }
                            url = u
                            if (!openBrowser(context, u)) {
                                repo.read { transportSignInCancel(id) }
                                error = str(R.string.netbird_no_browser)
                                return@launch
                            }
                            waiting = true
                            val email = repo.write { transportSignInFinish(id) }
                            onNotify(if (email != null) str(R.string.netbird_signed_in_as, email) else str(R.string.netbird_signed_in))
                            onDone(true)
                        } catch (e: CancellationException) {
                            throw e
                        } catch (e: Exception) {
                            waiting = false
                            Log.w("LabtermNetbird", "sign-in to ${network.label} failed", e)
                            error = e.userMessage()
                        } finally {
                            VpnSignIns.remove(id)
                        }
                    }
                }) { Text(stringResource(R.string.netbird_sign_in)) }
            }
        },
        dismissButton = { TextButton(onClick = ::cancel) { Text(stringResource(R.string.cancel)) } },
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
