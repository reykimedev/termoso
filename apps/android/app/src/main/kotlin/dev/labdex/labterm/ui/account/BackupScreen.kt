package dev.labdex.labterm.ui.account

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Backup
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Restore
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.BackupCard
import dev.labdex.labterm.core.BackupRestoreResult
import dev.labdex.labterm.core.BackupVaultCard
import dev.labdex.labterm.core.VaultAccess
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.core.VaultKind
import dev.labdex.labterm.core.backupMinPassword
import dev.labdex.labterm.data.AccountManager
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SecretField
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.shell.ShellViewModel
import java.time.LocalDateTime
import java.time.OffsetDateTime
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// Encrypted `.labterm` backups, the same file as on the desktop and Labterm Web.

const val BACKUP_MIME = "application/octet-stream"

/** Largest file read into memory; the desktop's cap. */
private const val MAX_BACKUP_BYTES = 512L * 1024 * 1024

fun backupFileName(): String =
    "labterm-backup-" + LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyyMMdd-HHmm")) + ".labterm"

/** Whole file behind a picked document; refuses anything over [MAX_BACKUP_BYTES]. */
suspend fun readBackupFile(context: Context, uri: Uri): ByteArray = withContext(Dispatchers.IO) {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { c ->
        if (c.moveToFirst() && !c.isNull(0) && c.getLong(0) > MAX_BACKUP_BYTES) error(str(R.string.backup_file_too_large))
    }
    context.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: error(str(R.string.could_not_read_the_file))
}

suspend fun writeBackupFile(context: Context, uri: Uri, bytes: ByteArray) = withContext(Dispatchers.IO) {
    context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(bytes) } ?: error(str(R.string.could_not_write_the_file))
}

private val KIND_LABELS = mapOf(
    "host" to R.plurals.backup_hosts,
    "group" to R.plurals.backup_groups,
    "identity" to R.plurals.backup_identities,
    "ssh_key" to R.plurals.backup_keys,
    "ssh_certificate" to R.plurals.backup_certificates,
    "snippet" to R.plurals.backup_snippets,
    "pf_rule" to R.plurals.backup_forwarding_rules,
    "known_host" to R.plurals.backup_known_hosts,
    "oidc_provider" to R.plurals.backup_openid_providers,
    "tag" to R.plurals.backup_tags,
)

/** "12 hosts · 3 keys · 40 known hosts", the five largest kinds. */
@Composable
fun backupCountsLine(v: BackupVaultCard): String {
    val res = LocalContext.current.resources
    val parts = v.counts.filter { it.kind in KIND_LABELS }.take(5).map {
        val n = it.count.toInt()
        res.getQuantityString(KIND_LABELS.getValue(it.kind), n, n)
    }
    return if (parts.isEmpty()) res.getQuantityString(R.plurals.backup_items, v.entities.toInt(), v.entities.toInt()) else parts.joinToString(" · ")
}

fun backupCreatedLine(card: BackupCard): String {
    val at = runCatching {
        OffsetDateTime.parse(card.createdAt).atZoneSameInstant(java.time.ZoneId.systemDefault())
            .format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM, FormatStyle.SHORT))
    }.getOrDefault(card.createdAt)
    return str(R.string.backup_created_line, at, card.appVersion)
}

fun vaultKindIcon(kind: VaultKind): ImageVector = when (kind) {
    VaultKind.LOCAL -> Icons.Filled.PhoneAndroid
    VaultKind.PERSONAL -> Icons.Filled.Cloud
    VaultKind.TEAM -> Icons.Filled.Group
}

/** Vaults a backup can be restored into: unlocked and not view-only. */
fun restoreTargets(vaults: List<VaultInfo>): List<VaultInfo> =
    vaults.filter { !it.locked && it.access != VaultAccess.VIEW }

/** Restore target: a team vault created for this backup vault, named like it. */
const val NEW_TEAM_VAULT = "new-team-vault"

/** Team choice for [NEW_TEAM_VAULT]: create a team first. */
const val NEW_TEAM = "new-team"

/**
 * Where each vault of a backup goes by default: the same vault when it is
 * still here (local vault, same account), else a vault of the same kind, else
 * the personal vault. Teams do not move with a backup, so a team vault of
 * another server goes into a team vault of the same name or, with
 * [newTeamVaults], a new one; otherwise into the personal vault.
 */
fun defaultRestoreTargets(
    card: BackupCard,
    targets: List<VaultInfo>,
    skipSameVault: Boolean = false,
    newTeamVaults: Boolean = false,
): Map<Int, String?> =
    card.vaults.mapIndexed { i, v ->
        val same = targets.firstOrNull { it.id == v.id }
        val sameName = targets.firstOrNull { it.kind == VaultKind.TEAM && it.name == v.name }
        val target = when {
            same != null -> if (skipSameVault) null else same.id
            v.kind == VaultKind.TEAM && sameName != null -> sameName.id
            v.kind == VaultKind.TEAM && newTeamVaults -> NEW_TEAM_VAULT
            else -> (
                targets.firstOrNull { it.kind == v.kind && v.kind != VaultKind.TEAM }
                    ?: targets.firstOrNull { it.kind == VaultKind.PERSONAL }
                    ?: targets.firstOrNull { it.kind == v.kind }
                    ?: targets.firstOrNull()
                )?.id
        }
        i to target
    }.toMap()

/**
 * Create what [NEW_TEAM_VAULT] rows ask for: the team when [team] is
 * [NEW_TEAM] (its first vault, named after it, is reused for the row of the
 * same name), then one team vault per remaining row. Returns the mapping with
 * real vault ids, and the team used.
 */
suspend fun createRestoreTargets(
    repo: VaultRepository,
    card: BackupCard,
    targets: Map<Int, String?>,
    team: String,
    teamName: String,
): Pair<Map<Int, String?>, String> {
    if (targets.values.none { it == NEW_TEAM_VAULT }) return targets to team
    var teamId = team
    val free = mutableListOf<VaultInfo>()
    if (teamId == NEW_TEAM) {
        val created = repo.write { createTeam(teamName.trim()) }
        teamId = created.id
        free += repo.read { vaults() }.filter { it.teamId == created.id }
    }
    val out = targets.toMutableMap()
    card.vaults.forEachIndexed { i, v ->
        if (out[i] != NEW_TEAM_VAULT) return@forEachIndexed
        val reuse = free.firstOrNull { it.name == v.name }
        out[i] = if (reuse != null) {
            free.remove(reuse)
            reuse.id
        } else {
            repo.write { createTeamVault(teamId, v.name, emptyList()) }
        }
    }
    return out to teamId
}

/** Restore every mapped vault of an opened backup; the sum of the reports. */
suspend fun restoreBackup(repo: VaultRepository, card: BackupCard, targets: Map<Int, String?>): BackupRestoreResult {
    var added = 0u
    var replaced = 0u
    var skipped = 0u
    val warnings = mutableListOf<String>()
    card.vaults.indices.forEach { i ->
        val target = targets[i] ?: return@forEach
        val r = repo.write { backupRestore(card.previewId, i.toUInt(), target) }
        added += r.added
        replaced += r.replaced
        skipped += r.skipped
        warnings += r.warnings
    }
    return BackupRestoreResult(added, replaced, skipped, warnings)
}

@Composable
fun BackupPasswordFields(
    password: String,
    onPassword: (String) -> Unit,
    confirm: String?,
    onConfirm: (String) -> Unit = {},
    enabled: Boolean = true,
) {
    val min = remember { backupMinPassword().toInt() }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SecretField(password, onPassword, stringResource(R.string.backup_password), enabled = enabled)
        if (confirm != null) {
            SecretField(confirm, onConfirm, stringResource(R.string.backup_repeat_password), enabled = enabled)
            val hint = when {
                password.isNotEmpty() && password.length < min -> stringResource(R.string.backup_password_min, min)
                confirm.isNotEmpty() && confirm != password -> stringResource(R.string.backup_passwords_differ)
                else -> stringResource(R.string.backup_password_hint, min)
            }
            Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

fun backupPasswordReady(password: String, confirm: String): Boolean =
    password.length >= backupMinPassword().toInt() && password == confirm

/** One row per vault of the backup with a "restore into" picker. */
@Composable
fun RestoreMapping(
    card: BackupCard,
    targets: List<VaultInfo>,
    chosen: Map<Int, String?>,
    allowNewTeamVault: Boolean = false,
    onChoose: (Int, String?) -> Unit,
) {
    SectionCard {
        card.vaults.forEachIndexed { i, v ->
            if (i > 0) RowDivider()
            var open by remember { mutableStateOf(false) }
            val into = targets.firstOrNull { it.id == chosen[i] }
            ListRow(
                title = v.name,
                subtitle = backupCountsLine(v),
                leading = { IconTile(vaultKindIcon(v.kind)) },
            ) {
                Box {
                    TextButton(onClick = { open = true }, enabled = targets.isNotEmpty() || allowNewTeamVault) {
                        Text(
                            when {
                                chosen[i] == NEW_TEAM_VAULT -> stringResource(R.string.backup_into_new_team_vault)
                                into != null -> stringResource(R.string.backup_into, into.name)
                                else -> stringResource(R.string.skip)
                            },
                        )
                    }
                    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.skip)) }, onClick = { onChoose(i, null); open = false })
                        targets.forEach { t ->
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.backup_into, t.name)) },
                                leadingIcon = { androidx.compose.material3.Icon(vaultKindIcon(t.kind), null) },
                                onClick = { onChoose(i, t.id); open = false },
                            )
                        }
                        if (allowNewTeamVault) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.backup_into_new_team_vault)) },
                                leadingIcon = { androidx.compose.material3.Icon(Icons.Filled.Group, null) },
                                onClick = { onChoose(i, NEW_TEAM_VAULT); open = false },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun RestoreResultCard(r: BackupRestoreResult) {
    SectionCard {
        ListRow(
            title = stringResource(R.string.backup_restore_counts, r.added.toInt(), r.replaced.toInt()),
            subtitle = if (r.skipped > 0u) stringResource(R.string.backup_restore_skipped, r.skipped.toInt()) else null,
            leading = { IconTile(Icons.Filled.Restore, tint = MaterialTheme.colorScheme.primary) },
        )
        r.warnings.take(10).forEach {
            RowDivider()
            ListRow(title = it, titleColor = MaterialTheme.colorScheme.error)
        }
    }
}

private sealed interface BackupStep {
    data object Menu : BackupStep
    data object Export : BackupStep
    data class Exported(val card: BackupCard) : BackupStep
    data class Unlock(val bytes: ByteArray, val name: String) : BackupStep
    data class Preview(val card: BackupCard) : BackupStep
    data class Restored(val result: BackupRestoreResult) : BackupStep
}

/** Settings → Backup: export unlocked vaults to a `.labterm` file or restore one. */
@Composable
fun BackupScreen(shell: ShellViewModel, account: AccountManager, onBack: () -> Unit, onMoveServer: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val status by account.status.collectAsStateWithLifecycle()
    var step by remember { mutableStateOf<BackupStep>(BackupStep.Menu) }
    var busy by remember { mutableStateOf(false) }
    var vaults by remember { mutableStateOf<List<VaultInfo>>(emptyList()) }
    LaunchedEffect(step) { vaults = runCatching { shell.repo.read { vaults() } }.getOrDefault(emptyList()) }

    // Export state.
    val unlocked = vaults.filter { !it.locked }
    val picked = remember { mutableStateMapOf<String, Boolean>() }
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    val chosen = remember { mutableStateMapOf<Int, String?>() }

    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(BACKUP_MIME)) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        busy = true
        scope.launch {
            val ids = unlocked.filter { picked[it.id] != false }.map { it.id }
            runCatching {
                val file = shell.repo.read { backupExport(ids, password, false) }
                writeBackupFile(context, uri, file.bytes)
                file.card
            }.onSuccess { password = ""; confirm = ""; step = BackupStep.Exported(it) }
                .onFailure { shell.notify(it.userMessage()) }
            busy = false
        }
    }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            runCatching { readBackupFile(context, uri) }
                .onSuccess { password = ""; step = BackupStep.Unlock(it, uri.lastPathSegment ?: "") }
                .onFailure { shell.notify(it.userMessage()) }
        }
    }

    val back: () -> Unit = back@{
        if (step != BackupStep.Menu) {
            step = BackupStep.Menu
            return@back
        }
        onBack()
    }
    SubScreen(title = stringResource(R.string.backup), onBack = back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            when (val s = step) {
                BackupStep.Menu -> {
                    Text(
                        stringResource(R.string.backup_intro),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
                    )
                    SectionCard {
                        ChevronRow(
                            title = stringResource(R.string.backup_export),
                            subtitle = stringResource(R.string.backup_export_hint),
                            leading = { IconTile(Icons.Filled.Backup) },
                            modifier = Modifier.clickable { step = BackupStep.Export },
                        )
                        RowDivider()
                        ChevronRow(
                            title = stringResource(R.string.backup_restore),
                            subtitle = stringResource(R.string.backup_restore_hint),
                            leading = { IconTile(Icons.Filled.Restore) },
                            modifier = Modifier.clickable { open.launch(arrayOf("*/*")) },
                        )
                    }
                    if (status.account != null) {
                        SectionLabel(stringResource(R.string.move_server))
                        SectionCard {
                            ChevronRow(
                                title = stringResource(R.string.move_server),
                                subtitle = stringResource(R.string.move_server_hint),
                                leading = { IconTile(Icons.AutoMirrored.Filled.ArrowForward) },
                                modifier = Modifier.clickable(onClick = onMoveServer),
                            )
                        }
                    }
                }
                BackupStep.Export -> {
                    SectionLabel(stringResource(R.string.backup_vaults))
                    SectionCard {
                        if (unlocked.isEmpty()) ListRow(title = stringResource(R.string.backup_no_unlocked_vault))
                        unlocked.forEachIndexed { i, v ->
                            if (i > 0) RowDivider()
                            val on = picked[v.id] != false
                            ListRow(
                                title = v.name,
                                leading = { IconTile(vaultKindIcon(v.kind)) },
                                modifier = Modifier.clickable { picked[v.id] = !on },
                            ) { Checkbox(checked = on, onCheckedChange = { picked[v.id] = it }) }
                        }
                    }
                    Text(
                        stringResource(R.string.backup_export_policy),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
                    )
                    SectionLabel(stringResource(R.string.backup_password))
                    BackupPasswordFields(password, { password = it }, confirm, { confirm = it }, enabled = !busy)
                    Spacer(Modifier.height(16.dp))
                    Button(
                        onClick = { save.launch(backupFileName()) },
                        enabled = !busy && backupPasswordReady(password, confirm) && unlocked.any { picked[it.id] != false },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.backup_save))
                    }
                }
                is BackupStep.Exported -> {
                    SectionCard {
                        ListRow(
                            title = stringResource(R.string.backup_saved),
                            subtitle = stringResource(R.string.backup_saved_hint),
                            leading = { IconTile(Icons.Filled.Lock, tint = MaterialTheme.colorScheme.primary) },
                        )
                    }
                    Spacer(Modifier.height(8.dp))
                    BackupSummary(s.card)
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = { step = BackupStep.Menu }, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.done)) }
                }
                is BackupStep.Unlock -> {
                    if (s.name.isNotEmpty()) {
                        Text(s.name, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(8.dp))
                    }
                    BackupPasswordFields(password, { password = it }, confirm = null, enabled = !busy)
                    Spacer(Modifier.height(16.dp))
                    Button(
                        onClick = {
                            busy = true
                            scope.launch {
                                runCatching { shell.repo.read { backupInspect(s.bytes, password) } }
                                    .onSuccess { card ->
                                        password = ""
                                        chosen.clear()
                                        chosen.putAll(defaultRestoreTargets(card, restoreTargets(vaults)))
                                        step = BackupStep.Preview(card)
                                    }
                                    .onFailure { shell.notify(it.userMessage()) }
                                busy = false
                            }
                        },
                        enabled = !busy && password.isNotEmpty(),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.backup_unlock))
                    }
                }
                is BackupStep.Preview -> {
                    Text(backupCreatedLine(s.card), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.height(8.dp))
                    val targets = restoreTargets(vaults)
                    if (targets.isEmpty()) ListRow(title = stringResource(R.string.backup_no_target), titleColor = MaterialTheme.colorScheme.error)
                    RestoreMapping(s.card, targets, chosen) { i, id -> chosen[i] = id }
                    Text(
                        stringResource(R.string.backup_restore_explain),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
                    )
                    Button(
                        onClick = {
                            busy = true
                            scope.launch {
                                runCatching { restoreBackup(shell.repo, s.card, chosen.toMap()) }
                                    .onSuccess {
                                        shell.repo.read { backupDiscard(s.card.previewId) }
                                        step = BackupStep.Restored(it)
                                    }
                                    .onFailure { shell.notify(it.userMessage()) }
                                busy = false
                            }
                        },
                        enabled = !busy && chosen.values.any { it != null },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.backup_restore_button))
                    }
                }
                is BackupStep.Restored -> {
                    RestoreResultCard(s.result)
                    Spacer(Modifier.height(16.dp))
                    OutlinedButton(onClick = { step = BackupStep.Menu }, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.done)) }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
fun BackupSummary(card: BackupCard) {
    Text(backupCreatedLine(card), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    Spacer(Modifier.height(8.dp))
    SectionCard {
        card.vaults.forEachIndexed { i, v ->
            if (i > 0) RowDivider()
            ListRow(title = v.name, subtitle = backupCountsLine(v), leading = { IconTile(vaultKindIcon(v.kind)) })
        }
    }
}
