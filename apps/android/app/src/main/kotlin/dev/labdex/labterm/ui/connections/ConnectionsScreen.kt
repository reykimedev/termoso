package dev.labdex.labterm.ui.connections

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import kotlinx.coroutines.flow.MutableStateFlow
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.CloudQueue
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.RoundedCornerShape
import dev.labdex.labterm.ui.terminal.TerminalPreview
import dev.labdex.labterm.R
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.data.Tunnel
import dev.labdex.labterm.core.PfRuleItem
import dev.labdex.labterm.core.PfKind
import dev.labdex.labterm.core.TunnelState
import androidx.compose.material.icons.filled.SwapVert
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Alignment
import dev.labdex.labterm.data.SftpConnection
import dev.labdex.labterm.data.TerminalSession
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.HostAvatar
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.closeHostLabel
import dev.labdex.labterm.ui.components.connectingLabel
import dev.labdex.labterm.ui.hosts.ConfirmDialog
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.ui.terminal.JoinLiveDialog
import dev.labdex.labterm.ui.terminal.quickTargetText
import dev.labdex.labterm.ui.terminal.siblingsOf
import dev.labdex.labterm.ui.vault.recentIn
import dev.labdex.labterm.core.FileProtocol
import dev.labdex.labterm.core.HistoryItem
import dev.labdex.labterm.core.MobileException
import dev.labdex.labterm.core.SessionState
import dev.labdex.labterm.core.TransferStatus
import dev.labdex.labterm.core.isLiveLink
import dev.labdex.labterm.core.parseTarget
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.launch

private const val RECENT_MAX = 10

/** History is device-wide; read enough of it that the selected vault's share still fills the list. */
private const val RECENT_SCAN = 100u

/** Connections tab: quick connect, active terminals, ways to connect, recent sessions. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConnectionsScreen(
    shell: ShellViewModel,
    onAddHost: () -> Unit,
    onConnectHost: (String) -> Unit,
    onOpenTerminal: () -> Unit,
    onNewSftp: () -> Unit,
    onOpenSftp: (String) -> Unit,
    onOpenForwarding: () -> Unit = {},
    onEditHost: (String) -> Unit = {},
    onAddHostFrom: (String) -> Unit = {},
    /** A USB-UART adapter was just attached: open the serial sheet once. */
    openSerial: Boolean = false,
    onSerialConsumed: () -> Unit = {},
) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val sessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    val activeSessionId by shell.sessions.activeId.collectAsStateWithLifecycle()
    val sftp by shell.sftp.connections.collectAsStateWithLifecycle()
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedVaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vault = vaults.firstOrNull { it.id == selectedVaultId }
    val scope = rememberCoroutineScope()
    var target by remember { mutableStateOf("") }
    var recent by remember { mutableStateOf<List<HistoryItem>>(emptyList()) }
    val tunnels by shell.forwards.tunnels.collectAsStateWithLifecycle()
    var pfRules by remember { mutableStateOf<List<PfRuleItem>>(emptyList()) }
    LaunchedEffect(revision, vault) {
        recent = runCatching { shell.repo.read { history(RECENT_SCAN) } }.getOrDefault(emptyList())
            .recentIn(vault, RECENT_MAX)
    }
    LaunchedEffect(revision, tunnels.keys) {
        pfRules = if (tunnels.isEmpty()) emptyList() else runCatching { shell.repo.read { pfRules(null) } }.getOrDefault(emptyList())
    }

    var joinDialog by remember { mutableStateOf(false) }
    var serialSheet by remember { mutableStateOf(false) }
    LaunchedEffect(openSerial) {
        if (openSerial) {
            serialSheet = true
            onSerialConsumed()
        }
    }
    var topMenu by remember { mutableStateOf(false) }
    var confirmCloseAll by remember { mutableStateOf(false) }
    val open = sessions.size + sftp.size

    fun join(link: String) {
        scope.launch {
            if (shell.joinLive(link) != null) {
                target = ""
                joinDialog = false
                onOpenTerminal()
            }
        }
    }

    fun connect() {
        if (isLiveLink(target.trim())) {
            join(target)
            return
        }
        val parsed = try {
            parseTarget(target)
        } catch (e: MobileException) {
            shell.notify(e.userMessage())
            return
        }
        scope.launch {
            if (shell.connectQuick(parsed) != null) {
                target = ""
                onOpenTerminal()
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.connections)) },
                actions = {
                    if (open > 0) {
                        Box {
                            IconButton(onClick = { topMenu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.more)) }
                            DropdownMenu(expanded = topMenu, onDismissRequest = { topMenu = false }) {
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.close_all_sessions, open), color = MaterialTheme.colorScheme.error) },
                                    leadingIcon = { Icon(Icons.Filled.PowerSettingsNew, null, tint = MaterialTheme.colorScheme.error) },
                                    onClick = { topMenu = false; confirmCloseAll = true },
                                )
                            }
                        }
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            OutlinedTextField(
                value = target,
                onValueChange = { target = it },
                placeholder = { Text(stringResource(R.string.quick_connect_placeholder)) },
                label = { Text(stringResource(R.string.quick_connect)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, capitalization = KeyboardCapitalization.None, imeAction = ImeAction.Go, autoCorrectEnabled = false),
                keyboardActions = KeyboardActions(onGo = { connect() }),
                trailingIcon = {
                    IconButton(onClick = ::connect, enabled = target.isNotBlank()) {
                        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = stringResource(R.string.connect))
                    }
                },
            )

            val workspaces = shell.sessions.workspaces
            val previous by (workspaces?.previous ?: MutableStateFlow(null)).collectAsStateWithLifecycle()
            val templates by (workspaces?.templates ?: MutableStateFlow(emptyList())).collectAsStateWithLifecycle()
            if (previous != null && sessions.isEmpty()) {
                SectionLabel(stringResource(R.string.previous_session))
                SectionCard {
                    ChevronRow(
                        title = pluralStringResource(R.plurals.restore_n_tabs, previous!!.targets.size, previous!!.targets.size),
                        subtitle = stringResource(R.string.previous_session_subtitle),
                        leading = { IconTile(Icons.Filled.History) },
                        modifier = Modifier.clickable {
                            val targets = previous!!.targets
                            workspaces?.dismissPrevious()
                            scope.launch { if (shell.openWorkspace(targets) > 0) onOpenTerminal() }
                        },
                    )
                    RowDivider()
                    ChevronRow(
                        title = stringResource(R.string.dismiss),
                        leading = { IconTile(Icons.Filled.Close) },
                        modifier = Modifier.clickable { workspaces?.dismissPrevious() },
                    )
                }
            }
            if (templates.isNotEmpty()) {
                SectionLabel(stringResource(R.string.workspaces))
                SectionCard {
                    templates.forEachIndexed { i, ws ->
                        if (i > 0) RowDivider()
                        var menu by remember(ws.id) { mutableStateOf(false) }
                        Box {
                            ChevronRow(
                                title = ws.name,
                                subtitle = pluralStringResource(R.plurals.n_tabs, ws.targets.size, ws.targets.size),
                                leading = { IconTile(Icons.Filled.Dashboard) },
                                modifier = Modifier.combinedClickable(
                                    onClick = { scope.launch { if (shell.openWorkspace(ws.targets) > 0) onOpenTerminal() } },
                                    onLongClick = { menu = true },
                                ),
                            )
                            if (menu) ItemActionsSheet(
                                title = ws.name,
                                subtitle = pluralStringResource(R.plurals.n_tabs, ws.targets.size, ws.targets.size),
                                leading = { IconTile(Icons.Filled.Dashboard) },
                                sections = listOf(
                                    SheetSection(
                                        stringResource(R.string.workspaces),
                                        listOf(
                                            SheetAction(Icons.AutoMirrored.Filled.OpenInNew, stringResource(R.string.open_)) {
                                                scope.launch { if (shell.openWorkspace(ws.targets) > 0) onOpenTerminal() }
                                            },
                                        ),
                                    ),
                                    SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true) { workspaces?.delete(ws.id) })),
                                ),
                                onClose = { menu = false },
                            )
                        }
                    }
                }
            }

            if (sessions.isNotEmpty() || sftp.isNotEmpty()) {
                SectionLabel(if (open == 1) stringResource(R.string.active_session) else stringResource(R.string.active_sessions, open))
                SectionCard {
                    // The session to resume (last active, with the thumbnail) leads the list.
                    val ordered = sessions.sortedByDescending { it.id == activeSessionId }
                    ordered.forEachIndexed { i, s ->
                        if (i > 0) RowDivider()
                        ActiveSessionRow(
                            session = s,
                            fontFamily = settings.terminalFontFamily,
                            // Only the session the user left the terminal from gets the big
                            // thumbnail; every tab has one in the terminal's tab switcher.
                            showPreview = s.id == activeSessionId,
                            siblings = siblingsOf(s, sessions),
                            sftpForHost = s.hostId?.let { id -> sftp.filter { it.hostId == id } } ?: emptyList(),
                            onOpen = { shell.sessions.setActive(s.id); onOpenTerminal() },
                            onDuplicate = { scope.launch { if (shell.duplicateSession(s.id) != null) onOpenTerminal() } },
                            onReconnect = { scope.launch { shell.sessions.reconnect(s.id) } },
                            onSftp = {
                                val hostId = s.hostId
                                val vaultId = s.vaultId
                                val quick = s.quick
                                when {
                                    s.local != null -> scope.launch { shell.openLocalFiles()?.let { onOpenSftp(it.id) } }
                                    hostId != null && vaultId != null ->
                                        scope.launch { shell.openSftpHost(hostId, vaultId)?.let { onOpenSftp(it.id) } }
                                    quick != null -> scope.launch { shell.openSftpQuick(quick)?.let { onOpenSftp(it.id) } }
                                }
                            },
                            onEditHost = { s.hostId?.let(onEditHost) },
                            onAddHost = { s.quick?.let { onAddHostFrom(quickTargetText(it)) } },
                            onClose = { scope.launch { shell.sessions.close(s.id) } },
                            onCloseHost = { ids, sftpIds ->
                                scope.launch {
                                    shell.sessions.closeMany(ids)
                                    shell.sftp.closeMany(sftpIds)
                                }
                            },
                        )
                    }
                    sftp.forEachIndexed { i, c ->
                        if (i > 0 || sessions.isNotEmpty()) RowDivider()
                        SftpRow(
                            conn = c,
                            terminalsForHost = c.hostId?.let { id -> sessions.filter { it.hostId == id } } ?: emptyList(),
                            sftpSiblings = c.hostId?.let { id -> sftp.filter { it.hostId == id && it.id != c.id } } ?: emptyList(),
                            onOpen = { onOpenSftp(c.id) },
                            onTerminal = {
                                val hostId = c.hostId
                                val vaultId = c.vaultId
                                val quick = c.quick
                                scope.launch {
                                    val opened = when {
                                        hostId != null && vaultId != null -> shell.connectHost(hostId, vaultId)
                                        quick != null -> shell.connectQuick(quick)
                                        else -> null
                                    }
                                    if (opened != null) onOpenTerminal()
                                }
                            },
                            onEditHost = { c.hostId?.let(onEditHost) },
                            onAddHost = { c.quick?.let { onAddHostFrom(quickTargetText(it)) } },
                            onClose = { scope.launch { shell.sftp.close(c.id) } },
                            onCloseHost = { ids, sftpIds ->
                                scope.launch {
                                    shell.sessions.closeMany(ids)
                                    shell.sftp.closeMany(sftpIds)
                                }
                            },
                        )
                    }
                }
            }

            if (tunnels.isNotEmpty()) {
                SectionLabel(stringResource(R.string.port_forwarding))
                SectionCard {
                    tunnels.values.sortedBy { it.ruleId }.forEachIndexed { i, tunnel ->
                        if (i > 0) RowDivider()
                        ActiveTunnelRow(
                            tunnel = tunnel,
                            rule = pfRules.firstOrNull { it.id == tunnel.ruleId },
                            onOpen = onOpenForwarding,
                            onStop = { scope.launch { shell.forwards.stop(tunnel.ruleId) } },
                        )
                    }
                }
            }

            SectionLabel(stringResource(R.string.ways_to_connect))
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.add_host),
                    subtitle = stringResource(R.string.save_a_server_with_its_credentials),
                    leading = { IconTile(Icons.Filled.Add) },
                    modifier = Modifier.clickable(onClick = onAddHost),
                )
                RowDivider()
                val linux by shell.sessions.linux.installed.collectAsStateWithLifecycle()
                ChevronRow(
                    title = stringResource(R.string.local_terminal),
                    subtitle = linux?.let { "${it.name} ${it.version}" } ?: stringResource(R.string.a_shell_on_this_device),
                    leading = { IconTile(Icons.Filled.PhoneAndroid) },
                    modifier = Modifier.clickable {
                        scope.launch { if (shell.connectLocal() != null) onOpenTerminal() }
                    },
                )
                if (linux != null) {
                    RowDivider()
                    ChevronRow(
                        title = stringResource(R.string.android_shell),
                        subtitle = stringResource(R.string.a_shell_on_this_device),
                        leading = { IconTile(Icons.Filled.PhoneAndroid) },
                        modifier = Modifier.clickable {
                            scope.launch { if (shell.connectAndroidShell() != null) onOpenTerminal() }
                        },
                    )
                }
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.serial_usb),
                    subtitle = stringResource(R.string.serial_usb_subtitle),
                    leading = { IconTile(Icons.Filled.Usb) },
                    modifier = Modifier.clickable { serialSheet = true },
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.files),
                    subtitle = stringResource(R.string.files_protocols_subtitle),
                    leading = { IconTile(Icons.Filled.FolderOpen) },
                    modifier = Modifier.clickable(onClick = onNewSftp),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.join_shared_terminal),
                    subtitle = stringResource(R.string.open_a_join_link_somebody_sent_you),
                    leading = { IconTile(Icons.Filled.Groups) },
                    modifier = Modifier.clickable { joinDialog = true },
                )
            }

            if (recent.isNotEmpty()) {
                SectionLabel(stringResource(R.string.recent))
                SectionCard {
                    recent.forEachIndexed { i, h ->
                        if (i > 0) RowDivider()
                        val hostId = h.hostId
                        ListRow(
                            title = h.label.ifBlank { h.target },
                            subtitle = historySubtitle(h),
                            leading = { IconTile(Icons.Filled.History) },
                            modifier = if (hostId != null) {
                                Modifier.clickable {
                                    scope.launch { if (shell.inSelectedVault(hostId)) onConnectHost(hostId) }
                                }
                            } else {
                                Modifier
                            },
                            titleColor = if (h.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (serialSheet) {
        SerialSheet(
            onConnect = { key, line ->
                serialSheet = false
                scope.launch { if (shell.connectSerial(key, line) != null) onOpenTerminal() }
            },
            onDismiss = { serialSheet = false },
        )
    }
    if (joinDialog) {
        JoinLiveDialog(onDismiss = { joinDialog = false }, onJoin = ::join)
    }
    if (confirmCloseAll) {
        ConfirmDialog(
            title = stringResource(R.string.close_all_sessions_2),
            text = pluralStringResource(R.plurals.close_all_sessions_text, open, open),
            confirm = stringResource(R.string.close_all),
            onConfirm = {
                confirmCloseAll = false
                scope.launch {
                    shell.sessions.closeAll()
                    shell.sftp.closeMany(sftp.map { it.id })
                }
            },
            onDismiss = { confirmCloseAll = false },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ActiveSessionRow(
    session: TerminalSession,
    fontFamily: String,
    showPreview: Boolean,
    siblings: List<TerminalSession>,
    sftpForHost: List<SftpConnection>,
    onOpen: () -> Unit,
    onDuplicate: () -> Unit,
    onReconnect: () -> Unit,
    onSftp: () -> Unit,
    onEditHost: () -> Unit,
    onAddHost: () -> Unit,
    onClose: () -> Unit,
    onCloseHost: (List<String>, List<String>) -> Unit,
) {
    val state by session.state.collectAsStateWithLifecycle()
    val detected by session.detectedOs.collectAsStateWithLifecycle()
    val title by session.title.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> connectingLabel(s)
        is SessionState.Connected -> title ?: session.target
        is SessionState.Closed -> stringResource(R.string.closed) + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    val remote = !session.isView && session.local == null
    val ssh = remote && (session.quick?.protocol?.equals("ssh", true) ?: true)
    val hostTotal = siblings.size + 1 + sftpForHost.size
    Column {
        // A glance at what is on screen, for the session to resume.
        if (showPreview && state is SessionState.Connected) {
            TerminalPreview(
                session = session,
                fontFamily = fontFamily,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(start = 12.dp, end = 12.dp, top = 12.dp)
                    .height(112.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .clickable(onClick = onOpen),
            )
        }
        ListRow(
        title = session.label,
        subtitle = subtitle,
        leading = {
            if (session.isView) IconTile(Icons.Filled.Groups) else HostAvatar(detected ?: session.savedOsName)
        },
        trailing = {
            Box {
                ItemMoreButton({ menu = true }, stringResource(R.string.session_actions))
                if (menu) ItemActionsSheet(
                    title = session.label,
                    subtitle = subtitle,
                    leading = { if (session.isView) IconTile(Icons.Filled.Groups) else HostAvatar(detected ?: session.savedOsName) },
                    sections = listOf(
                        SheetSection(
                            stringResource(R.string.session_actions),
                            buildList {
                                add(SheetAction(Icons.AutoMirrored.Filled.OpenInNew, stringResource(R.string.open_), onClick = onOpen))
                                if (!session.isView) add(SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.duplicate), onClick = onDuplicate))
                                if (session.reconnectable) add(SheetAction(Icons.Filled.Refresh, stringResource(R.string.reconnect), onClick = onReconnect))
                                if (ssh) add(SheetAction(Icons.Filled.FolderOpen, stringResource(R.string.open_sftp), onClick = onSftp))
                                if (!session.isView && session.local != null) add(SheetAction(Icons.Filled.FolderOpen, stringResource(R.string.open_local_files), onClick = onSftp))
                                if (session.hostId != null) add(SheetAction(Icons.Filled.Edit, stringResource(R.string.edit_host), onClick = onEditHost))
                                else if (session.quick != null) add(SheetAction(Icons.Filled.Add, stringResource(R.string.add_to_hosts), onClick = onAddHost))
                            },
                        ),
                        SheetSection(
                            null,
                            buildList {
                                add(SheetAction(Icons.Filled.Close, stringResource(R.string.close_session), danger = true, onClick = onClose))
                                if (hostTotal > 1) {
                                    add(
                                        SheetAction(Icons.Filled.PowerSettingsNew, closeHostLabel(hostTotal), danger = true) {
                                            onCloseHost(siblings.map { it.id } + session.id, sftpForHost.map { it.id })
                                        },
                                    )
                                }
                            },
                        ),
                    ),
                    onClose = { menu = false },
                )
            }
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close_session)) }
        },
        modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }, onLongClickLabel = stringResource(R.string.session_actions)),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SftpRow(
    conn: SftpConnection,
    terminalsForHost: List<TerminalSession>,
    sftpSiblings: List<SftpConnection>,
    onOpen: () -> Unit,
    onTerminal: () -> Unit,
    onEditHost: () -> Unit,
    onAddHost: () -> Unit,
    onClose: () -> Unit,
    onCloseHost: (List<String>, List<String>) -> Unit,
) {
    val state by conn.state.collectAsStateWithLifecycle()
    val transfers by conn.transfers.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val active = transfers.count { it.status is TransferStatus.Running || it.status is TransferStatus.Queued }
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> connectingLabel(s)
        is SessionState.Connected ->
            when (conn.protocol) {
                FileProtocol.WEBDAV -> stringResource(R.string.webdav_target, conn.target)
                FileProtocol.S3 -> "S3 · ${conn.target}"
                FileProtocol.LOCAL -> conn.target
                FileProtocol.SFTP -> stringResource(R.string.sftp, conn.target)
            } +
                if (active > 0) stringResource(R.string.sep_transferring, active) else ""
        is SessionState.Closed -> stringResource(R.string.closed) + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    val hostTotal = terminalsForHost.size + sftpSiblings.size + 1
    ListRow(
        title = conn.label,
        subtitle = subtitle,
        leading = { IconTile(conn.protocol.icon()) },
        trailing = {
            Box {
                ItemMoreButton({ menu = true }, stringResource(R.string.connection_actions))
                if (menu) ItemActionsSheet(
                    title = conn.label,
                    subtitle = subtitle,
                    leading = { IconTile(conn.protocol.icon()) },
                    sections = listOf(
                        SheetSection(
                            stringResource(R.string.connection_actions),
                            buildList {
                                add(SheetAction(Icons.AutoMirrored.Filled.OpenInNew, stringResource(R.string.open_), onClick = onOpen))
                                if (conn.protocol == FileProtocol.SFTP && (conn.hostId != null || conn.quick != null)) {
                                    add(SheetAction(Icons.Filled.Terminal, stringResource(R.string.open_terminal), onClick = onTerminal))
                                }
                                if (conn.hostId != null) add(SheetAction(Icons.Filled.Edit, stringResource(R.string.edit_host), onClick = onEditHost))
                                else if (conn.quick != null) add(SheetAction(Icons.Filled.Add, stringResource(R.string.add_to_hosts), onClick = onAddHost))
                            },
                        ),
                        SheetSection(
                            null,
                            buildList {
                                add(SheetAction(Icons.Filled.Close, stringResource(R.string.close_connection), danger = true, onClick = onClose))
                                if (hostTotal > 1) {
                                    add(
                                        SheetAction(Icons.Filled.PowerSettingsNew, closeHostLabel(hostTotal), danger = true) {
                                            onCloseHost(terminalsForHost.map { it.id }, sftpSiblings.map { it.id } + conn.id)
                                        },
                                    )
                                }
                            },
                        ),
                    ),
                    onClose = { menu = false },
                )
            }
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close_connection)) }
        },
        modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }, onLongClickLabel = stringResource(R.string.connection_actions)),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
}

private fun FileProtocol.icon(): ImageVector = when (this) {
    FileProtocol.WEBDAV, FileProtocol.S3 -> Icons.Filled.CloudQueue
    FileProtocol.LOCAL -> Icons.Filled.PhoneAndroid
    FileProtocol.SFTP -> Icons.Filled.FolderOpen
}

fun historySubtitle(h: HistoryItem): String {
    val time = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(h.startedAt))
    val duration = h.durationSecs?.let { s ->
        when {
            s < 60u -> str(R.string.duration_s, s.toLong())
            s < 3600u -> str(R.string.duration_m, (s / 60u).toLong())
            else -> str(R.string.duration_h_m, (s / 3600u).toLong(), ((s % 3600u) / 60u).toLong())
        }
    }
    return listOfNotNull(h.target, time, duration, h.error).joinToString(" · ")
}

/** A live port-forwarding tunnel, so Connections lists every kind of active connection like Termius. */
@Composable
private fun ActiveTunnelRow(tunnel: Tunnel, rule: PfRuleItem?, onOpen: () -> Unit, onStop: () -> Unit) {
    val state by tunnel.state.collectAsStateWithLifecycle()
    val endpoints = rule?.let {
        when (it.kind) {
            PfKind.REMOTE -> "${it.remote_host_or_bind()}:${it.remotePort} → localhost:${it.localPort}"
            PfKind.DYNAMIC -> "SOCKS ${it.boundAddress}:${it.localPort}"
            else -> "localhost:${it.localPort} → ${it.remoteHost}:${it.remotePort}"
        } + " · " + it.hostLabel
    } ?: tunnel.ruleId
    val status = when (state) {
        is TunnelState.Running -> stringResource(R.string.running)
        is TunnelState.Reconnecting -> stringResource(R.string.reconnecting)
        else -> stringResource(R.string.connecting)
    }
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(start = 16.dp, end = 4.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Filled.SwapVert, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(rule?.label?.ifBlank { null } ?: endpoints, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                if (rule?.label.isNullOrBlank()) status else "$endpoints · $status",
                style = MaterialTheme.typography.bodySmall,
                color = if (state is TunnelState.Running) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        IconButton(onClick = onStop) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.stop)) }
    }
}

private fun PfRuleItem.remote_host_or_bind(): String = boundAddress.ifBlank { "*" }
