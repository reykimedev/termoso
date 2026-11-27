package dev.labdex.labterm.ui.shell

import android.net.Uri
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cable
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Cable
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.TextButton
import androidx.compose.ui.text.font.FontFamily
import dev.labdex.labterm.QuickAction
import dev.labdex.labterm.data.RecentShortcuts
import dev.labdex.labterm.R
import dev.labdex.labterm.data.AccountManager
import dev.labdex.labterm.data.OpenTarget
import dev.labdex.labterm.data.AiManager
import dev.labdex.labterm.data.AppContainer
import dev.labdex.labterm.data.ForwardManager
import dev.labdex.labterm.data.PresenceManager
import dev.labdex.labterm.data.SessionManager
import dev.labdex.labterm.data.SftpConnection
import dev.labdex.labterm.data.SftpManager
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.account.AccountScreen
import dev.labdex.labterm.ui.account.AuthMode
import dev.labdex.labterm.ui.account.ReauthHost
import dev.labdex.labterm.ui.account.SecurityKeysScreen
import dev.labdex.labterm.ui.account.SignInScreen
import dev.labdex.labterm.ui.account.SshIdFido2Screen
import dev.labdex.labterm.ui.account.SshIdScreen
import dev.labdex.labterm.ui.connections.ConnectionsScreen
import dev.labdex.labterm.ui.forwarding.ForwardEditorScreen
import dev.labdex.labterm.ui.forwarding.ForwardWizardScreen
import dev.labdex.labterm.ui.forwarding.ForwardingScreen
import dev.labdex.labterm.ui.forwarding.TunnelPromptHost
import dev.labdex.labterm.ui.hosts.HostEditorScreen
import dev.labdex.labterm.ui.hosts.HostsScreen
import dev.labdex.labterm.ui.keychain.Fido2GenerateScreen
import dev.labdex.labterm.ui.keychain.Fido2LoadScreen
import dev.labdex.labterm.ui.keychain.GenerateKeyScreen
import dev.labdex.labterm.ui.keychain.IdentityEditorScreen
import dev.labdex.labterm.ui.keychain.ImportKeyScreen
import dev.labdex.labterm.ui.keychain.KeyDetailScreen
import dev.labdex.labterm.ui.keychain.KeychainScreen
import dev.labdex.labterm.ui.settings.LinuxScreen
import dev.labdex.labterm.ui.settings.SettingsScreen
import dev.labdex.labterm.ui.settings.TerminalAppearanceScreen
import dev.labdex.labterm.ui.settings.TerminalInputScreen
import dev.labdex.labterm.ui.sftp.SftpPickScreen
import dev.labdex.labterm.ui.sftp.SftpScreen
import dev.labdex.labterm.ui.snippets.SnippetEditorScreen
import dev.labdex.labterm.ui.snippets.SnippetsScreen
import dev.labdex.labterm.ui.team.TeamActivityScreen
import dev.labdex.labterm.ui.team.TeamRolesScreen
import dev.labdex.labterm.ui.team.TeamDevicesScreen
import dev.labdex.labterm.data.can
import dev.labdex.labterm.ui.team.TeamAccessScreen
import dev.labdex.labterm.ui.team.TeamAutomationsScreen
import dev.labdex.labterm.ui.team.TeamComplianceScreen
import dev.labdex.labterm.ui.team.TeamDataScreen
import dev.labdex.labterm.ui.team.TeamScreen
import dev.labdex.labterm.ui.team.TeamVaultScreen
import dev.labdex.labterm.ui.team.TeamsScreen
import dev.labdex.labterm.ui.terminal.TerminalScreen
import dev.labdex.labterm.ui.vault.HistoryScreen
import dev.labdex.labterm.ui.hosts.TagsScreen
import dev.labdex.labterm.ui.vault.KnownHostsScreen
import dev.labdex.labterm.ui.vault.SessionLogScreen
import dev.labdex.labterm.ui.vault.SessionLogsScreen
import dev.labdex.labterm.ui.vault.VaultScreen
import dev.labdex.labterm.core.FileProtocol
import dev.labdex.labterm.core.KeyMods
import dev.labdex.labterm.core.PfKind
import dev.labdex.labterm.core.Transport
import dev.labdex.labterm.core.parseTarget

object Routes {
    const val VAULT = "vault"
    const val CONNECTIONS = "connections"
    const val SETTINGS = "settings"
    const val HOSTS = "hosts?group={group}&tag={tag}"
    const val TAGS = "tags"
    const val HOST_NEW = "hostNew?group={group}&target={target}"
    const val HOST_EDIT = "hostEdit/{id}"
    const val KEYCHAIN = "keychain"
    const val KEY_GENERATE = "keyGenerate"
    const val KEY_IMPORT = "keyImport"
    const val KEY_FIDO2 = "keyFido2"
    const val KEY_FIDO2_LOAD = "keyFido2Load"
    const val KEY_DETAIL = "key/{id}"
    const val IDENTITY_NEW = "identityNew"
    const val IDENTITY_EDIT = "identity/{id}"
    const val TERMINAL_APPEARANCE = "terminalAppearance"
    const val TERMINAL_INPUT = "terminalInput"
    const val LINUX = "linux"
    const val KNOWN_HOSTS = "knownHosts"
    const val PROXIES = "proxies"
    const val JUMP_CHAINS = "jumpChains"
    const val OIDC_PROVIDERS = "oidcProviders"
    const val NETBIRD = "netbird"
    const val VPN_NETWORK = "vpn/{id}"
    const val HISTORY = "history"
    const val LOGS = "logs"
    const val LOG = "log/{id}"
    const val TERMINAL = "terminal"
    const val ACCOUNT = "account"
    const val TEAMS = "teams"
    const val SSH_ID = "sshId"
    const val SSH_ID_FIDO2 = "sshIdFido2"
    const val SECURITY_KEYS = "securityKeys"
    const val TEAM = "team/{id}"
    const val TEAM_ACTIVITY = "team/{id}/activity"
    const val TEAM_ROLES = "team/{id}/roles"
    const val TEAM_DEVICES = "team/{id}/devices"
    const val TEAM_ACCESS = "team/{id}/access"
    const val TEAM_LOGS = "team/{id}/logs"
    const val TEAM_DATA = "team/{id}/data"
    const val TEAM_COMPLIANCE = "team/{id}/compliance"
    const val TEAM_AUTOMATIONS = "team/{id}/automations"
    const val TEAM_VAULT = "teamVault/{id}"
    const val SIGN_IN = "signIn/{mode}"
    const val SFTP = "sftp/{id}"
    const val SFTP_PICK = "sftpPick"
    const val FORWARDING = "forwarding"
    const val PF_WIZARD = "pfWizard"
    const val PF_NEW = "pfNew?kind={kind}&vault={vault}&host={host}"
    const val PF_EDIT = "pfEdit/{id}"
    const val SNIPPETS = "snippets?pkg={pkg}"
    const val SNIPPET_NEW = "snippetNew?vault={vault}&pkg={pkg}"
    const val SNIPPET_EDIT = "snippetEdit/{id}"

    fun hosts(group: String?) = if (group == null) "hosts" else "hosts?group=$group"

    /** Every host of the vault with this tag label. */
    fun hostsTagged(tag: String) = "hosts?tag=${Uri.encode(tag)}"
    fun hostNew(group: String?) = if (group == null) "hostNew" else "hostNew?group=$group"

    /** New-host form prefilled from a quick-connect target (`user@host:port`, `telnet://host`). */
    fun hostNewFrom(target: String) = "hostNew?target=${Uri.encode(target)}"
    fun hostEdit(id: String) = "hostEdit/$id"
    fun vpnNetwork(id: String) = "vpn/$id"
    fun key(id: String) = "key/$id"
    fun identity(id: String) = "identity/$id"
    fun log(id: String) = "log/$id"
    fun signIn(mode: AuthMode) = "signIn/${mode.name}"
    fun team(id: String) = "team/$id"
    fun teamActivity(id: String) = "team/$id/activity"
    fun teamRoles(id: String) = "team/$id/roles"
    fun teamDevices(id: String) = "team/$id/devices"
    fun teamAccess(id: String) = "team/$id/access"
    fun teamLogs(id: String) = "team/$id/logs"
    fun teamData(id: String) = "team/$id/data"
    fun teamCompliance(id: String) = "team/$id/compliance"
    fun teamAutomations(id: String) = "team/$id/automations"
    fun teamVault(id: String) = "teamVault/$id"
    fun sftp(id: String) = "sftp/$id"
    fun pfNew(kind: PfKind, vault: String?, host: String?) =
        "pfNew?kind=${kind.name}" + (vault?.let { "&vault=$it" } ?: "") + (host?.let { "&host=$it" } ?: "")
    fun pfEdit(id: String) = "pfEdit/$id"
    fun snippets(pkg: String?) = if (pkg == null) "snippets" else "snippets?pkg=$pkg"
    fun snippetNew(vault: String, pkg: String?) = "snippetNew?vault=$vault" + (pkg?.let { "&pkg=$it" } ?: "")
    fun snippetEdit(id: String) = "snippetEdit/$id"
}

private class Tab(val route: String, @StringRes val label: Int, val icon: ImageVector, val selectedIcon: ImageVector)

private val tabs = listOf(
    Tab(Routes.VAULT, R.string.vaults, Icons.Outlined.Lock, Icons.Filled.Lock),
    Tab(Routes.CONNECTIONS, R.string.connections, Icons.Outlined.Cable, Icons.Filled.Cable),
    Tab(Routes.SETTINGS, R.string.settings, Icons.Outlined.Settings, Icons.Filled.Settings),
)

/** Bottom-navigation shell: Vaults · Connections · Settings, with nested host screens. */
@Composable
fun MainShell(
    container: AppContainer,
    repo: VaultRepository,
    sessions: SessionManager,
    sftp: SftpManager,
    forwards: ForwardManager,
    account: AccountManager,
    presence: PresenceManager,
    ai: AiManager,
    onLock: () -> Unit,
) {
    val shell: ShellViewModel = viewModel { ShellViewModel(repo, sessions, sftp, forwards, presence, ai) }
    val workspaces = sessions.workspaces
    LaunchedEffect(shell) { account.personalVaultsHidden.collect { shell.hidePersonalVaults.value = it } }
    LaunchedEffect(Unit) {
        runCatching { dev.labdex.labterm.data.DebugUpdater.check() }.getOrNull()
            ?.let { shell.notify(dev.labdex.labterm.str(R.string.debug_update_available_notice, it.build)) }
    }
    val nav = rememberNavController()
    val scope = rememberCoroutineScope()
    val backStack by nav.currentBackStackEntryAsState()
    val currentRoute = backStack?.destination?.route
    // The host list is the Vaults tab's landing page, so the bar stays while browsing hosts.
    val vaultsTabRoute = currentRoute == Routes.VAULT || currentRoute == Routes.HOSTS
    val showBar = vaultsTabRoute || tabs.any { it.route == currentRoute }
    val snackbar = remember { SnackbarHostState() }

    val context = LocalContext.current
    val notice by shell.notice.collectAsStateWithLifecycle()
    LaunchedEffect(notice) {
        val text = notice ?: return@LaunchedEffect
        snackbar.showSnackbar(text)
        shell.noticeShown()
    }
    LaunchedEffect(account) {
        account.notices.collect { snackbar.showSnackbar(it) }
    }
    LaunchedEffect(forwards) { forwards.autoStartOnce() }
    TunnelPromptHost(forwards)
    ReauthHost(account)
    // A host behind a NetBird network this user has not signed in to yet: sign in, then connect.
    var netbirdSignIn by remember { mutableStateOf<Pair<dev.labdex.labterm.core.TransportItem, () -> Unit>?>(null) }
    netbirdSignIn?.let { (network, then) ->
        dev.labdex.labterm.ui.netbird.NetbirdSignInDialog(network, shell.repo, shell::notify) { ok ->
            netbirdSignIn = null
            if (ok) then()
        }
    }

    val pendingInvite by container.pendingInvite.collectAsStateWithLifecycle()
    val accountStatus by account.status.collectAsStateWithLifecycle()
    LaunchedEffect(pendingInvite, accountStatus.account?.userId) {
        if (pendingInvite == null) return@LaunchedEffect
        if (accountStatus.account == null) {
            shell.notify(str(R.string.sign_in_to_accept_the_team_invitation))
            nav.navigate(Routes.signIn(AuthMode.SignIn)) { launchSingleTop = true }
        } else if (nav.currentDestination?.route != Routes.TEAMS) {
            // Re-navigating onto the visible Teams screen would recreate it and
            // drop the dialog its previous instance just opened for this link.
            nav.navigate(Routes.TEAMS) { launchSingleTop = true }
        }
    }

    fun openTerminal() {
        nav.navigate(Routes.TERMINAL) { launchSingleTop = true }
    }

    // A tap on one of the shade's notifications: land on that session / list.
    val pendingOpen by container.pendingOpen.collectAsStateWithLifecycle()
    LaunchedEffect(pendingOpen) {
        val target = pendingOpen ?: return@LaunchedEffect
        container.consumeOpen()
        fun top(route: String) = nav.navigate(route) {
            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
        when (target) {
            is OpenTarget.Terminal -> {
                shell.sessions.setActive(target.sessionId)
                openTerminal()
            }
            OpenTarget.Connections -> top(Routes.CONNECTIONS)
            OpenTarget.Forwarding -> nav.navigate(Routes.FORWARDING) { launchSingleTop = true }
            is OpenTarget.Files -> {
                val id = target.connectionId?.takeIf { c -> shell.sftp.find(c) != null }
                if (id != null) nav.navigate(Routes.sftp(id)) { launchSingleTop = true } else top(Routes.CONNECTIONS)
            }
        }
    }

    // A labterm://join link: needs an account, then becomes a viewer terminal.
    val pendingJoin by container.pendingJoin.collectAsStateWithLifecycle()
    val pendingSerial by container.pendingSerial.collectAsStateWithLifecycle()
    val restoring by account.restoring.collectAsStateWithLifecycle()
    LaunchedEffect(pendingJoin, accountStatus.account?.userId, restoring) {
        val link = pendingJoin ?: return@LaunchedEffect
        if (restoring) return@LaunchedEffect
        if (accountStatus.account == null) {
            shell.notify(str(R.string.sign_in_to_join_the_shared_terminal))
            nav.navigate(Routes.signIn(AuthMode.SignIn)) { launchSingleTop = true }
            return@LaunchedEffect
        }
        // Consuming re-keys this effect, so the join itself runs on the shell scope.
        container.consumeJoin()
        shell.launch { if (shell.joinLive(link) != null) openTerminal() }
    }

    // Launcher shortcuts and labterm://host links: act once the shell is up.
    val pendingAction by container.pendingAction.collectAsStateWithLifecycle()
    LaunchedEffect(pendingAction, restoring) {
        val action = pendingAction ?: return@LaunchedEffect
        if (restoring) return@LaunchedEffect
        container.consumeAction()
        when (action) {
            QuickAction.NewConnection -> nav.navigate(Routes.hostNew(null)) { launchSingleTop = true }
            QuickAction.LocalTerminal -> shell.launch { if (shell.connectLocal() != null) openTerminal() }
            QuickAction.Sftp -> nav.navigate(Routes.CONNECTIONS) {
                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        }
    }
    val pendingHost by container.pendingHost.collectAsStateWithLifecycle()
    LaunchedEffect(pendingHost, restoring) {
        val id = pendingHost ?: return@LaunchedEffect
        if (restoring) return@LaunchedEffect
        container.consumeHost()
        shell.launch { if (shell.connectHost(id) != null) openTerminal() }
    }

    // Files shared into the app: sent to the active terminal, or held until one is opened.
    val pendingShare by container.pendingShare.collectAsStateWithLifecycle()
    val openSessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    val openSftp by shell.sftp.connections.collectAsStateWithLifecycle()
    LaunchedEffect(pendingShare, openSessions.isEmpty()) {
        if (pendingShare == null) return@LaunchedEffect
        if (openSessions.isEmpty()) {
            shell.notify(str(R.string.connect_to_a_host_the_shared_files_will))
            nav.navigate(Routes.CONNECTIONS) {
                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        } else if (nav.currentDestination?.route != Routes.TERMINAL) {
            openTerminal()
        }
    }

    // Text shared into the app: pasted into the active terminal after a confirmation.
    val pendingShareText by container.pendingShareText.collectAsStateWithLifecycle()
    LaunchedEffect(pendingShareText, openSessions.isEmpty()) {
        if (pendingShareText == null) return@LaunchedEffect
        if (openSessions.isEmpty()) {
            shell.notify(str(R.string.connect_to_a_host_the_shared_files_will))
            nav.navigate(Routes.CONNECTIONS) {
                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        } else if (nav.currentDestination?.route != Routes.TERMINAL) {
            openTerminal()
        }
    }
    pendingShareText?.let { text ->
        val target = shell.sessions.active ?: openSessions.firstOrNull()
        if (target != null) {
            AlertDialog(
                onDismissRequest = { container.consumeShareText() },
                title = { Text(stringResource(R.string.paste_shared_text)) },
                text = { Text(text.take(400), style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, maxLines = 8) },
                confirmButton = {
                    TextButton(onClick = { target.rust.paste(text); container.consumeShareText() }) { Text(stringResource(R.string.paste)) }
                },
                dismissButton = { TextButton(onClick = { container.consumeShareText() }) { Text(stringResource(R.string.cancel)) } },
            )
        }
    }

    fun openSftp(connectionId: String) {
        nav.navigate(Routes.sftp(connectionId)) { launchSingleTop = true }
    }

    /** SFTP for SSH hosts; a WebDAV-only host has no other browser, so it gets its share. */
    fun sftpHost(hostId: String) {
        shell.launch { shell.openSftpHost(hostId)?.let { openSftp(it.id) } }
    }

    fun webdavHost(hostId: String) {
        shell.launch { shell.openSftpHost(hostId, FileProtocol.WEBDAV)?.let { openSftp(it.id) } }
    }

    fun s3Host(hostId: String) {
        shell.launch { shell.openSftpHost(hostId, FileProtocol.S3)?.let { openSftp(it.id) } }
    }

    /** "Connect" on a host: a terminal, or the Files browser when the host is WebDAV-only. */
    fun connectHost(hostId: String, transport: Transport = Transport.AUTO) {
        shell.launch {
            val signIn = shell.selectedVaultId.value?.let { v ->
                runCatching { shell.repo.read { hostTransportSignIn(hostId, v) } }.getOrNull()
            }
            if (signIn != null) {
                netbirdSignIn = signIn to { connectHost(hostId, transport) }
            } else if (!shell.hasTerminal(hostId)) {
                shell.openSftpHost(hostId)?.let { openSftp(it.id) }
            } else if (shell.connectHost(hostId, transport) != null) {
                shell.selectedVaultId.value?.let { RecentShortcuts.record(context, shell.repo, hostId, it) }
                openTerminal()
            }
        }
    }

    /**
     * "Connect" on several hosts or a whole group: one session each, file-only
     * hosts skipped. A VPN network that needs a sign-in asks once and the rest
     * continue after it; hosts on the same network then share its tunnel.
     */
    fun connectHosts(hostIds: List<String>, tabGroup: String? = null) {
        shell.launch {
            val vault = shell.selectedVaultId.value ?: return@launch
            val groupId = tabGroup?.let { shell.sessions.ensureTabGroup(it) }
            var opened = 0
            for ((i, id) in hostIds.withIndex()) {
                val signIn = runCatching { shell.repo.read { hostTransportSignIn(id, vault) } }.getOrNull()
                if (signIn != null) {
                    val rest = hostIds.drop(i)
                    netbirdSignIn = signIn to { connectHosts(rest, tabGroup) }
                    break
                }
                val terminal = runCatching { shell.repo.read { host(id) } }
                    .map { it.protocol.equals("ssh", true) || it.protocol.equals("telnet", true) }
                    .getOrDefault(false)
                val session = if (terminal) shell.connectHost(id) else null
                if (session != null) {
                    if (groupId != null) shell.sessions.setInTabGroup(session.id, groupId, true)
                    RecentShortcuts.record(context, shell.repo, id, vault)
                    opened++
                }
            }
            if (opened > 0) openTerminal()
        }
    }

    /** Termius-style "Edit": a terminal to the same host running an editor on the file, exiting with it. */
    fun editInTerminal(conn: SftpConnection, path: String) {
        if (conn.protocol != FileProtocol.SFTP) return
        shell.launch {
            val session = when {
                conn.hostId != null && conn.vaultId != null -> shell.connectHost(conn.hostId, conn.vaultId)
                conn.quick != null -> shell.connectQuick(conn.quick)
                else -> null
            } ?: return@launch
            val quoted = "'" + path.replace("'", "'\\''") + "'"
            session.rust.sendText("\${EDITOR:-vi} $quoted; exit\n", KeyMods(ctrl = false, alt = false, shift = false))
            openTerminal()
        }
    }

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            if (showBar) {
                NavigationBar {
                    tabs.forEach { tab ->
                        val selected = if (tab.route == Routes.VAULT) vaultsTabRoute else currentRoute == tab.route
                        NavigationBarItem(
                            selected = selected,
                            onClick = {
                                if (tab.route == Routes.VAULT && currentRoute == Routes.HOSTS) {
                                    nav.popBackStack(Routes.VAULT, inclusive = false)
                                } else {
                                    nav.navigate(tab.route) {
                                        popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                                        launchSingleTop = true
                                        restoreState = true
                                    }
                                }
                            },
                            icon = {
                                if (tab.route == Routes.CONNECTIONS) {
                                    ConnectionsTabIcon(openSessions, openSftp, selected)
                                } else {
                                    Icon(if (selected) tab.selectedIcon else tab.icon, contentDescription = stringResource(tab.label))
                                }
                            },
                            label = { Text(stringResource(tab.label)) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        val groupArg = navArgument("group") { type = NavType.StringType; nullable = true; defaultValue = null }
        val tagArg = navArgument("tag") { type = NavType.StringType; nullable = true; defaultValue = null }
        val targetArg = navArgument("target") { type = NavType.StringType; nullable = true; defaultValue = null }
        val idArg = navArgument("id") { type = NavType.StringType }
        NavHost(
            navController = nav,
            startDestination = Routes.VAULT,
            modifier = Modifier.padding(bottom = if (showBar) padding.calculateBottomPadding() else 0.dp),
        ) {
            composable(Routes.VAULT) {
                VaultScreen(
                    shell = shell,
                    account = account,
                    onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
                    onSignIn = { nav.navigate(Routes.signIn(AuthMode.SignIn)) },
                    onOpenHosts = { nav.navigate(Routes.hosts(null)) },
                    onOpenKeychain = { nav.navigate(Routes.KEYCHAIN) },
                    onOpenForwarding = { nav.navigate(Routes.FORWARDING) },
                    onOpenSnippets = { nav.navigate(Routes.snippets(null)) },
                    onOpenKnownHosts = { nav.navigate(Routes.KNOWN_HOSTS) },
                    onOpenProxies = { nav.navigate(Routes.PROXIES) },
                    onOpenJumpChains = { nav.navigate(Routes.JUMP_CHAINS) },
                    onOpenOidcProviders = { nav.navigate(Routes.OIDC_PROVIDERS) },
                    onOpenNetbird = { nav.navigate(Routes.NETBIRD) },
                    onOpenHistory = { nav.navigate(Routes.HISTORY) },
                    onOpenLogs = { nav.navigate(Routes.LOGS) },
                )
            }
            composable(Routes.CONNECTIONS) {
                ConnectionsScreen(
                    openSerial = pendingSerial,
                    onSerialConsumed = container::consumeSerial,
                    shell = shell,
                    onAddHost = { nav.navigate(Routes.hostNew(null)) },
                    onConnectHost = { connectHost(it) },
                    onOpenTerminal = ::openTerminal,
                    onNewSftp = { nav.navigate(Routes.SFTP_PICK) },
                    onOpenSftp = ::openSftp,
                    onEditHost = { nav.navigate(Routes.hostEdit(it)) },
                    onAddHostFrom = { nav.navigate(Routes.hostNewFrom(it)) },
                    onOpenForwarding = { nav.navigate(Routes.FORWARDING) },
                )
            }
            composable(Routes.SETTINGS) {
                SettingsScreen(
                    shell = shell,
                    container = container,
                    account = account,
                    onAccount = { nav.navigate(Routes.ACCOUNT) },
                    onSignIn = { nav.navigate(Routes.signIn(AuthMode.SignIn)) },
                    onLock = onLock,
                    onTerminalAppearance = { nav.navigate(Routes.TERMINAL_APPEARANCE) },
                    onTerminalInput = { nav.navigate(Routes.TERMINAL_INPUT) },
                    onLinux = { nav.navigate(Routes.LINUX) },
                )
            }
            composable(Routes.LINUX) { LinuxScreen(shell = shell, onBack = { nav.popBackStack() }, onOpenTerminal = ::openTerminal) }
            composable(Routes.TERMINAL_APPEARANCE) { TerminalAppearanceScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.TERMINAL_INPUT) { TerminalInputScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.ACCOUNT) {
                AccountScreen(
                    shell = shell,
                    account = account,
                    onBack = { nav.popBackStack() },
                    onSignedOut = { nav.popBackStack() },
                    onTeams = { nav.navigate(Routes.TEAMS) },
                    onSshId = { nav.navigate(Routes.SSH_ID) },
                    onSecurityKeys = { nav.navigate(Routes.SECURITY_KEYS) },
                )
            }
            composable(Routes.SSH_ID) {
                SshIdScreen(
                    shell = shell,
                    account = account,
                    onBack = { nav.popBackStack() },
                    onAddSecurityKey = { nav.navigate(Routes.SSH_ID_FIDO2) },
                )
            }
            composable(Routes.SSH_ID_FIDO2) {
                SshIdFido2Screen(shell = shell, account = account, onClose = { nav.popBackStack() }, onDone = { nav.popBackStack() })
            }
            composable(Routes.SECURITY_KEYS) {
                SecurityKeysScreen(shell = shell, account = account, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAMS) {
                TeamsScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpenTeam = { nav.navigate(Routes.team(it)) },
                    joinLink = pendingInvite,
                    onJoinLinkShown = container::consumeInvite,
                )
            }
            composable(Routes.TEAM, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamScreen(
                    shell = shell,
                    account = account,
                    teamId = id,
                    onBack = { nav.popBackStack() },
                    onOpenVault = { nav.navigate(Routes.teamVault(it)) },
                    onActivity = { nav.navigate(Routes.teamActivity(id)) },
                    onRoles = { nav.navigate(Routes.teamRoles(id)) },
                    onDevices = { nav.navigate(Routes.teamDevices(id)) },
                    onAccess = { nav.navigate(Routes.teamAccess(id)) },
                    onRecordings = { nav.navigate(Routes.teamLogs(id)) },
                    onData = { nav.navigate(Routes.teamData(id)) },
                    onCompliance = { nav.navigate(Routes.teamCompliance(id)) },
                    onAutomations = { nav.navigate(Routes.teamAutomations(id)) },
                    onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
                )
            }
            composable(Routes.TEAM_ACTIVITY, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamActivityScreen(shell = shell, teamId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_ROLES, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamRolesScreen(shell = shell, teamId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_DEVICES, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamDevicesScreen(shell = shell, teamId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_DATA, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                val t = shell.repo.perms.teams.collectAsState().value.firstOrNull { it.id == id }
                TeamDataScreen(shell = shell, teamId = id, canRotation = t?.can("rotation.manage") == true, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_LOGS, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                SessionLogsScreen(shell = shell, onBack = { nav.popBackStack() }, onOpen = { nav.navigate(Routes.log(it)) }, teamId = id)
            }
            composable(Routes.TEAM_ACCESS, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamAccessScreen(shell = shell, account = account, teamId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_COMPLIANCE, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                val t = shell.repo.perms.teams.collectAsState().value.firstOrNull { it.id == id }
                TeamComplianceScreen(shell = shell, account = account, teamId = id, canDecide = t?.can("members.remove") == true, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_AUTOMATIONS, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                val t = shell.repo.perms.teams.collectAsState().value.firstOrNull { it.id == id }
                TeamAutomationsScreen(shell = shell, teamId = id, canRun = t?.can("automations.run") == true, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_VAULT, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamVaultScreen(shell = shell, account = account, vaultId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.SIGN_IN, arguments = listOf(navArgument("mode") { type = NavType.StringType })) { entry ->
                val mode = entry.arguments?.getString("mode")?.let { m -> AuthMode.entries.firstOrNull { it.name == m } } ?: AuthMode.SignIn
                SignInScreen(
                    container = container,
                    account = account,
                    mode = mode,
                    onBack = { nav.popBackStack() },
                    onDone = {
                        if (!nav.popBackStack(Routes.ACCOUNT, inclusive = false)) {
                            nav.navigate(Routes.ACCOUNT) { popUpTo(Routes.SIGN_IN) { inclusive = true } }
                        }
                    },
                )
            }
            composable(Routes.HOSTS, arguments = listOf(groupArg, tagArg)) { entry ->
                val group = entry.arguments?.getString("group")
                HostsScreen(
                    shell = shell,
                    groupId = group,
                    tag = entry.arguments?.getString("tag"),
                    onOpenTags = { nav.navigate(Routes.TAGS) },
                    onBack = { nav.popBackStack() },
                    onOpenGroup = { nav.navigate(Routes.hosts(it)) },
                    onNewHost = { nav.navigate(Routes.hostNew(group)) },
                    onNewHostFrom = { nav.navigate(Routes.hostNewFrom(it)) },
                    onEditHost = { nav.navigate(Routes.hostEdit(it)) },
                    onConnect = { connectHost(it) },
                    onConnectWith = ::connectHost,
                    onConnectMany = { ids, group -> connectHosts(ids, group) },
                    onSftp = ::sftpHost,
                    onWebdav = ::webdavHost,
                    onS3 = ::s3Host,
                    onOpenSftp = ::openSftp,
                    onOpenTerminal = ::openTerminal,
                    onOpenVpn = { nav.navigate(Routes.vpnNetwork(it)) },
                    onForward = { nav.navigate(Routes.pfNew(PfKind.LOCAL, null, it)) },
                )
            }
            composable(Routes.TAGS) {
                TagsScreen(shell = shell, onBack = { nav.popBackStack() }, onOpenTag = { nav.navigate(Routes.hostsTagged(it)) })
            }
            composable(Routes.HOST_NEW, arguments = listOf(groupArg, targetArg)) { entry ->
                val target = entry.arguments?.getString("target")
                val prefill = remember(target) { target?.let { runCatching { parseTarget(it) }.getOrNull() } }
                HostEditorScreen(
                    shell = shell,
                    hostId = null,
                    groupId = entry.arguments?.getString("group"),
                    onClose = { nav.popBackStack() },
                    prefill = prefill,
                    files = container.files,
                    onConnect = { connectHost(it) },
                    onSftp = ::sftpHost,
                    onWebdav = ::webdavHost,
                    onS3 = ::s3Host,
                    onForward = { nav.navigate(Routes.pfNew(PfKind.LOCAL, null, it)) },
                )
            }
            composable(Routes.HOST_EDIT, arguments = listOf(navArgument("id") { type = NavType.StringType })) { entry ->
                HostEditorScreen(
                    shell = shell,
                    hostId = entry.arguments?.getString("id"),
                    groupId = null,
                    onClose = { nav.popBackStack() },
                    files = container.files,
                    onConnect = { connectHost(it) },
                    onSftp = ::sftpHost,
                    onWebdav = ::webdavHost,
                    onS3 = ::s3Host,
                    onForward = { nav.navigate(Routes.pfNew(PfKind.LOCAL, null, it)) },
                )
            }
            composable(Routes.KEYCHAIN) {
                KeychainScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onGenerate = { nav.navigate(Routes.KEY_GENERATE) },
                    onImport = { nav.navigate(Routes.KEY_IMPORT) },
                    onFido2 = { nav.navigate(Routes.KEY_FIDO2) },
                    onFido2Load = { nav.navigate(Routes.KEY_FIDO2_LOAD) },
                    onOpenKey = { nav.navigate(Routes.key(it)) },
                    onNewIdentity = { nav.navigate(Routes.IDENTITY_NEW) },
                    onOpenIdentity = { nav.navigate(Routes.identity(it)) },
                )
            }
            composable(Routes.KEY_GENERATE) {
                GenerateKeyScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_IMPORT) {
                ImportKeyScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_FIDO2) {
                Fido2GenerateScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_FIDO2_LOAD) {
                Fido2LoadScreen(shell = shell, onClose = { nav.popBackStack() }, onLoaded = { ids ->
                    if (ids.size == 1) nav.navigate(Routes.key(ids.single())) { popUpTo(Routes.KEYCHAIN) } else nav.popBackStack()
                })
            }
            composable(Routes.KEY_DETAIL, arguments = listOf(idArg)) { entry ->
                KeyDetailScreen(shell = shell, keyId = entry.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() })
            }
            composable(Routes.IDENTITY_NEW) { IdentityEditorScreen(shell = shell, identityId = null, onClose = { nav.popBackStack() }) }
            composable(Routes.IDENTITY_EDIT, arguments = listOf(idArg)) { entry ->
                IdentityEditorScreen(shell = shell, identityId = entry.arguments?.getString("id"), onClose = { nav.popBackStack() })
            }
            composable(Routes.KNOWN_HOSTS) { KnownHostsScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.PROXIES) { dev.labdex.labterm.ui.hosts.ProxiesScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.JUMP_CHAINS) { dev.labdex.labterm.ui.hosts.JumpChainsScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.OIDC_PROVIDERS) { dev.labdex.labterm.ui.keychain.OidcProvidersScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.NETBIRD) {
                dev.labdex.labterm.ui.netbird.NetbirdNetworksScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpenNetwork = { nav.navigate(Routes.vpnNetwork(it)) },
                )
            }
            composable(Routes.VPN_NETWORK, arguments = listOf(idArg)) { entry ->
                dev.labdex.labterm.ui.netbird.VpnNetworkScreen(
                    shell = shell,
                    networkId = entry.arguments?.getString("id") ?: "",
                    onBack = { nav.popBackStack() },
                    onOpenHost = { nav.navigate(Routes.hostEdit(it)) },
                )
            }
            composable(Routes.HISTORY) {
                HistoryScreen(shell = shell, onBack = { nav.popBackStack() }, onOpenHost = { nav.navigate(Routes.hostEdit(it)) }, onConnect = { connectHost(it) })
            }
            composable(Routes.LOGS) {
                SessionLogsScreen(shell = shell, onBack = { nav.popBackStack() }, onOpen = { nav.navigate(Routes.log(it)) })
            }
            composable(Routes.LOG, arguments = listOf(idArg)) { entry ->
                SessionLogScreen(shell = shell, logId = entry.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() })
            }
            composable(Routes.FORWARDING) {
                ForwardingScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onNewRule = { nav.navigate(Routes.PF_WIZARD) },
                    onEditRule = { nav.navigate(Routes.pfEdit(it)) },
                )
            }
            composable(Routes.PF_WIZARD) {
                ForwardWizardScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onContinue = { kind, vault ->
                        nav.navigate(Routes.pfNew(kind, vault, null)) { popUpTo(Routes.PF_WIZARD) { inclusive = true } }
                    },
                )
            }
            val optArg = { name: String -> navArgument(name) { type = NavType.StringType; nullable = true; defaultValue = null } }
            composable(Routes.PF_NEW, arguments = listOf(optArg("kind"), optArg("vault"), optArg("host"))) { entry ->
                val kind = entry.arguments?.getString("kind")?.let { k -> PfKind.entries.firstOrNull { it.name == k } } ?: PfKind.LOCAL
                ForwardEditorScreen(
                    shell = shell,
                    ruleId = null,
                    kind = kind,
                    vaultId = entry.arguments?.getString("vault"),
                    hostId = entry.arguments?.getString("host"),
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.PF_EDIT, arguments = listOf(idArg)) { entry ->
                ForwardEditorScreen(
                    shell = shell,
                    ruleId = entry.arguments?.getString("id") ?: "",
                    kind = PfKind.LOCAL,
                    vaultId = null,
                    hostId = null,
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SNIPPETS, arguments = listOf(optArg("pkg"))) { entry ->
                SnippetsScreen(
                    shell = shell,
                    packageId = entry.arguments?.getString("pkg"),
                    onBack = { nav.popBackStack() },
                    onOpenPackage = { nav.navigate(Routes.snippets(it)) },
                    onNewSnippet = { vault, pkg -> nav.navigate(Routes.snippetNew(vault, pkg)) },
                    onEditSnippet = { nav.navigate(Routes.snippetEdit(it)) },
                    onOpenTerminal = ::openTerminal,
                )
            }
            composable(Routes.SNIPPET_NEW, arguments = listOf(optArg("vault"), optArg("pkg"))) { entry ->
                SnippetEditorScreen(
                    shell = shell,
                    snippetId = null,
                    vaultId = entry.arguments?.getString("vault"),
                    packageId = entry.arguments?.getString("pkg"),
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SNIPPET_EDIT, arguments = listOf(idArg)) { entry ->
                SnippetEditorScreen(
                    shell = shell,
                    snippetId = entry.arguments?.getString("id") ?: "",
                    vaultId = null,
                    packageId = null,
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SFTP_PICK) {
                SftpPickScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpened = { id -> nav.navigate(Routes.sftp(id)) { popUpTo(Routes.SFTP_PICK) { inclusive = true } } },
                )
            }
            composable(Routes.SFTP, arguments = listOf(idArg)) { entry ->
                SftpScreen(
                    shell = shell,
                    connectionId = entry.arguments?.getString("id") ?: "",
                    onBack = { nav.popBackStack() },
                    onEdit = ::editInTerminal,
                )
            }
            composable(Routes.TERMINAL) {
                TerminalScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpenSnippets = { nav.navigate(Routes.snippets(null)) },
                    onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
                    onCustomizeKeys = { nav.navigate(Routes.TERMINAL_INPUT) },
                    pendingShare = pendingShare,
                    onShareConsumed = { container.consumeShare() },
                    onHardwareKeyHook = { container.hardwareKeyHook = it },
                    onOpenSftp = ::openSftp,
                    onForward = { nav.navigate(Routes.pfNew(PfKind.LOCAL, null, it)) },
                    onEditHost = { nav.navigate(Routes.hostEdit(it)) },
                    onAddHost = { nav.navigate(Routes.hostNewFrom(it)) },
                    onNewSession = {
                        // Leave the terminal first so it is not part of the
                        // saved tab state that restoreState would bring back.
                        nav.popBackStack()
                        nav.navigate(Routes.CONNECTIONS) {
                            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                            launchSingleTop = true
                            restoreState = true
                        }
                    },
                )
            }
        }
    }
}
