package dev.labdex.labterm.ui.account

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Login
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Restore
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.BackupCard
import dev.labdex.labterm.core.BackupRestoreResult
import dev.labdex.labterm.core.TeamCard
import dev.labdex.labterm.core.TeamRole
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.data.AccountManager
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.FormField
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.shell.ShellViewModel
import dev.labdex.labterm.ui.theme.Danger
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Steps of the move, in order. */
enum class MoveStep { Intro, Backup, SignOut, SignIn, Restore, Done }

data class MoveState(
    val step: MoveStep = MoveStep.Intro,
    /** Server the move started from, for the reminders at the end. */
    val fromServer: String? = null,
    /** The backup made in step 2, opened again so it restores without the password. */
    val backup: BackupCard? = null,
    val result: BackupRestoreResult? = null,
)

/**
 * The move lives in the process, not in the screen: signing in happens on the
 * sign-in screen and the guide picks up where it was when the user comes back.
 * If the app was killed in between, the restore step asks for the file again.
 */
object MoveServerFlow {
    private val _state = MutableStateFlow(MoveState())
    val state: StateFlow<MoveState> = _state.asStateFlow()

    fun update(transform: (MoveState) -> MoveState) = _state.update(transform)

    fun reset() {
        _state.value = MoveState()
    }
}

/** Items that stay behind, shown before the move starts and again at the end. */
private val STAYS_BEHIND = listOf(
    R.string.move_stays_teams,
    R.string.move_stays_history,
    R.string.move_stays_devices,
    R.string.move_stays_sshid,
    R.string.move_stays_mfa,
    R.string.move_stays_denied,
)

/**
 * "Move to another server": back up every vault, sign out, sign in or sign up
 * on the new server, restore, and list what has to be set up again there.
 */
@Composable
fun MoveServerScreen(shell: ShellViewModel, account: AccountManager, onBack: () -> Unit, onSignIn: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by MoveServerFlow.state.collectAsState()
    val status by account.status.collectAsStateWithLifecycle()
    var busy by remember { mutableStateOf(false) }
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var vaults by remember { mutableStateOf<List<VaultInfo>>(emptyList()) }
    val chosen = remember { mutableStateMapOf<Int, String?>() }
    // Teams of the new account we may add vaults to, for "into a new team vault".
    var myTeams by remember { mutableStateOf<List<TeamCard>>(emptyList()) }
    var teamPick by remember { mutableStateOf<String?>(null) }
    var teamName by remember { mutableStateOf<String?>(null) }
    // The opened backup is gone when the app was restarted mid-move.
    var backupOpen by remember { mutableStateOf(true) }

    val signedIn = status.account != null
    LaunchedEffect(state.step, signedIn) {
        // Signed in on the new server: go on to the restore.
        if (state.step == MoveStep.SignIn && signedIn) MoveServerFlow.update { it.copy(step = MoveStep.Restore) }
        vaults = runCatching { shell.repo.read { vaults() } }.getOrDefault(emptyList())
        if (signedIn) {
            myTeams = runCatching { shell.repo.read { teams() } }.getOrDefault(emptyList())
                .filter { it.myRole == TeamRole.OWNER || it.myRole == TeamRole.ADMIN }
        }
        val b = state.backup
        if (state.step == MoveStep.Restore && b != null) {
            backupOpen = shell.repo.read { backupIsOpen(b.previewId) }
            if (chosen.isEmpty()) {
                chosen.putAll(defaultRestoreTargets(b, restoreTargets(vaults), skipSameVault = true, newTeamVaults = true))
            }
        }
    }

    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(BACKUP_MIME)) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        busy = true
        scope.launch {
            runCatching {
                // Everything the policies allow, kept open for the restore
                // after signing in to the new server.
                val file = shell.repo.read { backupExport(emptyList(), password, true) }
                writeBackupFile(context, uri, file.bytes)
                file.card
            }.onSuccess { card ->
                password = ""
                confirm = ""
                MoveServerFlow.update { it.copy(backup = card, step = MoveStep.SignOut) }
            }.onFailure { shell.notify(it.userMessage()) }
            busy = false
        }
    }
    val reopen = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        busy = true
        scope.launch {
            runCatching {
                val bytes = readBackupFile(context, uri)
                shell.repo.read { backupInspect(bytes, password) }
            }
                .onSuccess { card ->
                    password = ""
                    chosen.clear()
                    chosen.putAll(defaultRestoreTargets(card, restoreTargets(vaults), skipSameVault = true, newTeamVaults = true))
                    backupOpen = true
                    MoveServerFlow.update { it.copy(backup = card) }
                }
                .onFailure { shell.notify(it.userMessage()) }
            busy = false
        }
    }

    val leave = {
        // Before anything changed, leaving cancels the move; afterwards the
        // guide keeps its place so the user can come back to it.
        if (state.step == MoveStep.Intro || state.step == MoveStep.Done) MoveServerFlow.reset()
        onBack()
    }

    SubScreen(title = stringResource(R.string.move_server), onBack = leave) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            val index = state.step.ordinal
            LinearProgressIndicator(
                progress = { (index + 1f) / MoveStep.entries.size },
                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
            )
            Text(
                stringResource(R.string.move_step_of, index + 1, MoveStep.entries.size),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(8.dp))
            when (state.step) {
                MoveStep.Intro -> {
                    Text(stringResource(R.string.move_intro), style = MaterialTheme.typography.bodyMedium)
                    SectionLabel(stringResource(R.string.move_carries_over))
                    SectionCard {
                        ListRow(
                            title = stringResource(R.string.move_carries_items),
                            leading = { IconTile(Icons.Filled.Check, tint = MaterialTheme.colorScheme.primary) },
                        )
                    }
                    StaysBehind()
                    Spacer(Modifier.height(16.dp))
                    Button(
                        onClick = { MoveServerFlow.update { it.copy(step = MoveStep.Backup, fromServer = status.account?.serverUrl) } },
                        enabled = signedIn,
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(stringResource(R.string.continue_)) }
                }
                MoveStep.Backup -> {
                    Text(stringResource(R.string.move_backup_explain), style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.height(12.dp))
                    BackupPasswordFields(password, { password = it }, confirm, { confirm = it }, enabled = !busy)
                    Spacer(Modifier.height(16.dp))
                    Button(
                        onClick = { save.launch(backupFileName()) },
                        enabled = !busy && backupPasswordReady(password, confirm),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.backup_save))
                    }
                }
                MoveStep.SignOut -> {
                    state.backup?.let { BackupSummary(it) }
                    Spacer(Modifier.height(12.dp))
                    Text(
                        stringResource(R.string.move_sign_out_explain, serverName(state.fromServer)),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Spacer(Modifier.height(16.dp))
                    Button(
                        onClick = {
                            busy = true
                            scope.launch {
                                runCatching { account.signOut() }
                                    .onSuccess { MoveServerFlow.update { it.copy(step = MoveStep.SignIn) } }
                                    .onFailure { shell.notify(it.userMessage()) }
                                busy = false
                            }
                        },
                        enabled = !busy,
                        colors = ButtonDefaults.buttonColors(containerColor = Danger),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        if (busy) {
                            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                        } else {
                            androidx.compose.material3.Icon(Icons.AutoMirrored.Filled.Logout, null)
                            Spacer(Modifier.size(8.dp))
                            Text(stringResource(R.string.move_sign_out, serverName(state.fromServer)))
                        }
                    }
                }
                MoveStep.SignIn -> {
                    Text(stringResource(R.string.move_sign_in_explain), style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = onSignIn, modifier = Modifier.fillMaxWidth()) {
                        androidx.compose.material3.Icon(Icons.AutoMirrored.Filled.Login, null)
                        Spacer(Modifier.size(8.dp))
                        Text(stringResource(R.string.move_sign_in))
                    }
                }
                MoveStep.Restore -> {
                    val now = status.account?.serverUrl
                    if (now != null && now.trimEnd('/') == state.fromServer?.trimEnd('/')) {
                        SectionCard {
                            ListRow(title = stringResource(R.string.move_same_server), titleColor = MaterialTheme.colorScheme.error)
                        }
                        Spacer(Modifier.height(8.dp))
                    }
                    val b = state.backup
                    if (b == null || !backupOpen) {
                        Text(stringResource(R.string.move_reopen_explain), style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(12.dp))
                        BackupPasswordFields(password, { password = it }, confirm = null, enabled = !busy)
                        Spacer(Modifier.height(16.dp))
                        Button(
                            onClick = { reopen.launch(arrayOf("*/*")) },
                            enabled = !busy && password.isNotEmpty(),
                            modifier = Modifier.fillMaxWidth(),
                        ) { Text(stringResource(R.string.move_open_backup)) }
                    } else {
                        Text(stringResource(R.string.move_restore_explain, serverName(now)), style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(12.dp))
                        RestoreMapping(b, restoreTargets(vaults), chosen, allowNewTeamVault = true) { i, id -> chosen[i] = id }
                        val firstNew = b.vaults.indices.firstOrNull { chosen[it] == NEW_TEAM_VAULT }
                        val team = teamPick ?: myTeams.firstOrNull()?.id ?: NEW_TEAM
                        val newTeamName = teamName ?: firstNew?.let { b.vaults[it].name }.orEmpty()
                        val needsTeamName = firstNew != null && team == NEW_TEAM
                        if (firstNew != null) {
                            Spacer(Modifier.height(12.dp))
                            TeamPicker(myTeams, team) { teamPick = it }
                            if (needsTeamName) {
                                Spacer(Modifier.height(8.dp))
                                FormField(newTeamName, { teamName = it }, stringResource(R.string.move_team_name), enabled = !busy)
                            }
                        }
                        Spacer(Modifier.height(16.dp))
                        Button(
                            onClick = {
                                busy = true
                                scope.launch {
                                    runCatching {
                                        val (into, teamId) = createRestoreTargets(shell.repo, b, chosen.toMap(), team, newTeamName)
                                        // A retry must not create the team or the vaults twice.
                                        chosen.putAll(into)
                                        if (teamId != team) teamPick = teamId
                                        vaults = shell.repo.read { vaults() }
                                        restoreBackup(shell.repo, b, into)
                                    }
                                        .onSuccess { r ->
                                            shell.repo.read { backupDiscard(b.previewId) }
                                            runCatching { account.syncNow() }
                                            MoveServerFlow.update { it.copy(step = MoveStep.Done, result = r, backup = null) }
                                        }
                                        .onFailure { shell.notify(it.userMessage()) }
                                    busy = false
                                }
                            },
                            enabled = !busy && chosen.values.any { it != null } && !(needsTeamName && newTeamName.isBlank()),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            if (busy) {
                                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                            } else {
                                androidx.compose.material3.Icon(Icons.Filled.Restore, null)
                                Spacer(Modifier.size(8.dp))
                                Text(stringResource(R.string.backup_restore_button))
                            }
                        }
                    }
                }
                MoveStep.Done -> {
                    state.result?.let { RestoreResultCard(it) }
                    Spacer(Modifier.height(8.dp))
                    Text(stringResource(R.string.move_done_explain), style = MaterialTheme.typography.bodyMedium)
                    StaysBehind()
                    state.fromServer?.let {
                        Spacer(Modifier.height(8.dp))
                        Text(
                            stringResource(R.string.move_old_account, serverName(it)),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Spacer(Modifier.height(16.dp))
                    OutlinedButton(onClick = { MoveServerFlow.reset(); onBack() }, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.done))
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun StaysBehind() {
    SectionLabel(stringResource(R.string.move_stays_behind))
    SectionCard {
        STAYS_BEHIND.forEachIndexed { i, res ->
            if (i > 0) RowDivider()
            ListRow(
                title = stringResource(res),
                leading = { IconTile(Icons.Filled.Close, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
            )
        }
    }
}

private fun serverName(url: String?): String =
    url?.removePrefix("https://")?.removePrefix("http://")?.trimEnd('/') ?: ""

/** "Team for the new vaults": a team we manage, or a new one. */
@Composable
private fun TeamPicker(teams: List<TeamCard>, team: String, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val name = teams.firstOrNull { it.id == team }?.name ?: stringResource(R.string.move_new_team)
    SectionCard {
        ListRow(title = stringResource(R.string.move_team_for_new_vaults)) {
            Box {
                TextButton(onClick = { open = true }) { Text(name) }
                DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                    teams.forEach { t ->
                        DropdownMenuItem(text = { Text(t.name) }, onClick = { onPick(t.id); open = false })
                    }
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.move_new_team)) },
                        onClick = { onPick(NEW_TEAM); open = false },
                    )
                }
            }
        }
    }
}
