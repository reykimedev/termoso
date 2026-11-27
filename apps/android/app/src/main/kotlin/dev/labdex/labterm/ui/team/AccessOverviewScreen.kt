package dev.labdex.labterm.ui.team

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.LocalOffer
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.HostAccessCard
import dev.labdex.labterm.core.HostPolicyCard
import dev.labdex.labterm.core.MemberAccessCard
import dev.labdex.labterm.core.PolicyRiskCard
import dev.labdex.labterm.core.PolicySourceCard
import dev.labdex.labterm.core.VaultAccessCard
import dev.labdex.labterm.core.VaultPolicyCard
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SectionLabel
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.components.itemClicks
import dev.labdex.labterm.ui.hosts.DefaultsEditorDialog
import dev.labdex.labterm.ui.hosts.DefaultsTarget
import dev.labdex.labterm.ui.shell.ShellViewModel

/**
 * Access overview (policy viewer): for every host of a vault, the login, VPN
 * network, jump hosts and proxy a connection from this phone ends up with,
 * where each comes from, the setups worth a second look and, in team vaults,
 * who can see the host. Worked out on the phone from the decrypted vault.
 * Each host opens the same quick menu as everywhere else (long-press or ⋯).
 */
@Composable
fun AccessOverviewScreen(
    shell: ShellViewModel,
    vaultId: String,
    teamId: String?,
    vaultName: String,
    onEditHost: (String) -> Unit,
    onBack: () -> Unit,
) {
    var policy by remember { mutableStateOf<VaultPolicyCard?>(null) }
    var access by remember { mutableStateOf<VaultAccessCard?>(null) }
    var failed by remember { mutableStateOf<String?>(null) }
    var reload by remember { mutableIntStateOf(0) }
    var query by remember { mutableStateOf("") }
    var onlyRisks by remember { mutableStateOf(false) }
    var details by remember { mutableStateOf<HostPolicyCard?>(null) }
    var menu by remember { mutableStateOf<HostPolicyCard?>(null) }
    var defaults by remember { mutableStateOf<DefaultsTarget?>(null) }

    LaunchedEffect(vaultId, reload) {
        runCatching { shell.repo.read { vaultPolicy(vaultId) } }
            .onSuccess { policy = it; failed = null }
            .onFailure { failed = it.userMessage() }
    }
    LaunchedEffect(vaultId, teamId) {
        val tid = teamId ?: return@LaunchedEffect
        runCatching { shell.repo.read { vaultHostAccess(tid, vaultId) } }
            .onSuccess { access = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    fun open(s: PolicySourceCard, h: HostPolicyCard) {
        when (s.kind) {
            "group" -> defaults = DefaultsTarget.Group(s.id)
            "tag" -> defaults = DefaultsTarget.Tag(s.id)
            // The member's own credentials live in the host editor's "My credentials".
            else -> onEditHost(h.hostId)
        }
    }

    SubScreen(title = stringResource(R.string.ao_title), onBack = onBack) { padding ->
        val p = policy
        when {
            failed != null -> Text(
                failed.orEmpty(),
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(padding).padding(24.dp),
            )
            p == null -> Row(Modifier.fillMaxSize().padding(padding), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
            }
            else -> {
                val q = query.trim().lowercase()
                val shown = p.hosts.filter { h ->
                    (!onlyRisks || h.risks.isNotEmpty()) &&
                        (q.isEmpty() || h.label.lowercase().contains(q) || h.address.lowercase().contains(q) || h.groupPath.any { it.lowercase().contains(q) })
                }
                LazyColumn(
                    Modifier.fillMaxSize().padding(padding),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                ) {
                    item {
                        Text(
                            vaultName + " · " + stringResource(R.string.ao_summary, p.hosts.size, p.hosts.count { it.risks.isNotEmpty() }),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            stringResource(if (p.team) R.string.ao_info_team else R.string.ao_info_personal),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                        Spacer(Modifier.height(12.dp))
                        FilterRow(onlyRisks) { onlyRisks = it }
                        Spacer(Modifier.height(8.dp))
                        OutlinedTextField(
                            value = query,
                            onValueChange = { query = it },
                            placeholder = { Text(stringResource(R.string.search)) },
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                        )
                        Spacer(Modifier.height(12.dp))
                        if (shown.isEmpty()) {
                            Text(
                                stringResource(if (p.hosts.isEmpty()) R.string.ao_no_hosts else R.string.ao_no_match),
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(vertical = 24.dp),
                            )
                        }
                    }
                    items(shown, key = { it.hostId }) { h ->
                        SectionCard(Modifier.padding(bottom = 8.dp)) {
                            HostRow(h, onOpen = { details = h }, onMenu = { menu = h })
                        }
                    }
                }
            }
        }
    }

    details?.let { h ->
        HostDetailsSheet(
            h = h,
            team = policy?.team == true,
            access = access,
            hostAccess = access?.hosts?.firstOrNull { it.hostId == h.hostId },
            labelOf = { id -> labelOf(policy, id) },
            onOpen = { s -> details = null; open(s, h) },
            onClose = { details = null },
        )
    }
    menu?.let { h ->
        ItemActionsSheet(
            title = h.label,
            subtitle = h.address,
            leading = { IconTile(Icons.Filled.Dns) },
            sections = listOf(
                SheetSection(null, editActions(h) { s -> open(s, h) }),
                SheetSection(
                    stringResource(R.string.ao_needs_a_look),
                    h.risks.map { r -> SheetAction(Icons.Filled.Build, stringResource(R.string.ao_fix, riskText(r))) { open(r.fix, h) } },
                ),
            ),
            onClose = { menu = null },
        )
    }
    defaults?.let { t ->
        DefaultsEditorDialog(shell = shell, target = t, onDismiss = { defaults = null; reload++ })
    }
}

/** Host, group or tag label for an id, for "rule on group …". */
private fun labelOf(p: VaultPolicyCard?, id: String): String =
    p?.groups?.firstOrNull { it.id == id }?.label ?: p?.hosts?.firstOrNull { it.hostId == id }?.label ?: id.take(8)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FilterRow(onlyRisks: Boolean, onChange: (Boolean) -> Unit) {
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        SegmentedButton(selected = !onlyRisks, onClick = { onChange(false) }, shape = SegmentedButtonDefaults.itemShape(0, 2)) {
            Text(stringResource(R.string.ao_all_hosts))
        }
        SegmentedButton(selected = onlyRisks, onClick = { onChange(true) }, shape = SegmentedButtonDefaults.itemShape(1, 2)) {
            Text(stringResource(R.string.ao_needs_a_look))
        }
    }
}

@Composable
private fun HostRow(h: HostPolicyCard, onOpen: () -> Unit, onMenu: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().itemClicks(onClick = onOpen, onMenu = onMenu).padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconTile(
            if (h.risks.isEmpty()) Icons.Filled.Dns else Icons.Filled.Warning,
            tint = if (h.risks.any { it.severity == "danger" }) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.size(16.dp))
        Column(Modifier.weight(1f)) {
            Text(h.label, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val path = (h.groupPath + h.address).joinToString(" / ")
            Text(path, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                listOf(loginText(h), networkText(h), jumpText(h)).joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (h.risks.isNotEmpty()) {
                Text(
                    stringResource(R.string.ao_risk_count, h.risks.size),
                    style = MaterialTheme.typography.labelMedium,
                    color = if (h.risks.any { it.severity == "danger" }) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.tertiary,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
        ItemMoreButton(onClick = onMenu)
    }
}

/** Edit host, then each group, tag or own-credentials place a value comes from. */
@Composable
private fun editActions(h: HostPolicyCard, open: (PolicySourceCard) -> Unit): List<SheetAction> {
    val sources = listOfNotNull(h.auth.source, h.network?.source, h.jumpSource, h.proxy?.source)
        .filter { it.kind != "host" }
        .distinctBy { it.id }
    return listOf(SheetAction(Icons.Filled.Edit, stringResource(R.string.edit_host)) { open(PolicySourceCard("host", h.hostId, h.label)) }) +
        sources.map { s ->
            SheetAction(
                when (s.kind) {
                    "group" -> Icons.Filled.Folder
                    "tag" -> Icons.Filled.LocalOffer
                    else -> Icons.Filled.Person
                },
                when (s.kind) {
                    "group" -> stringResource(R.string.ao_edit_group, s.label)
                    "tag" -> stringResource(R.string.ao_edit_tag, s.label)
                    else -> stringResource(R.string.ao_edit_my_credentials)
                },
            ) { open(s) }
        }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HostDetailsSheet(
    h: HostPolicyCard,
    team: Boolean,
    access: VaultAccessCard?,
    hostAccess: HostAccessCard?,
    labelOf: (String) -> String,
    onOpen: (PolicySourceCard) -> Unit,
    onClose: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onClose) {
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp),
        ) {
            Text(h.label, style = MaterialTheme.typography.titleMedium)
            Text(h.address, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

            if (h.risks.isNotEmpty()) {
                SectionLabel(stringResource(R.string.ao_needs_a_look))
                SectionCard {
                    h.risks.forEachIndexed { i, r ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = riskText(r),
                            subtitle = sourceText(r.fix),
                            titleColor = if (r.severity == "danger") MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.itemClicks(onClick = { onOpen(r.fix) }, onMenu = { onOpen(r.fix) }),
                        )
                    }
                }
            }

            SectionLabel(stringResource(R.string.ao_how_it_connects))
            SectionCard {
                Fact(stringResource(R.string.ao_login), loginText(h), h.auth.source, onOpen)
                RowDivider()
                Fact(stringResource(R.string.vpn_network), networkText(h), h.network?.source, onOpen)
                RowDivider()
                Fact(stringResource(R.string.ao_jump_hosts), jumpText(h), h.jumpSource, onOpen)
                h.proxy?.let { px ->
                    RowDivider()
                    Fact(stringResource(R.string.proxy), px.label, px.source, onOpen)
                }
            }

            if (team) {
                SectionLabel(stringResource(R.string.ao_who_can_see))
                SectionCard {
                    when {
                        access == null || hostAccess == null -> Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        }
                        else -> {
                            hostAccess.members.forEachIndexed { i, m ->
                                if (i > 0) RowDivider()
                                MemberRow(m, h.hostId, labelOf)
                            }
                            hostAccess.groups.forEach { g ->
                                RowDivider()
                                ListRow(
                                    title = stringResource(R.string.ao_idp_group, g.group),
                                    subtitle = ruleText(g.ruleOn, h.hostId, labelOf),
                                    trailing = { Text(modeText(g.mode), color = MaterialTheme.colorScheme.primary) },
                                )
                            }
                        }
                    }
                }
                if (access != null && !access.rulesReadable) {
                    Text(
                        stringResource(R.string.ao_rules_hidden),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun Fact(title: String, value: String, source: PolicySourceCard?, onOpen: (PolicySourceCard) -> Unit) {
    ListRow(
        title = value,
        subtitle = title + " · " + sourceText(source),
        modifier = if (source != null) Modifier.itemClicks(onClick = { onOpen(source) }, onMenu = { onOpen(source) }) else Modifier,
    )
}

@Composable
private fun MemberRow(m: MemberAccessCard, hostId: String, labelOf: (String) -> String) {
    val via = when (m.via) {
        "admin" -> stringResource(R.string.ao_via_admin)
        "vault" -> stringResource(R.string.ao_via_vault)
        else -> ruleText(m.ruleOn, hostId, labelOf)
    }
    val depends = if (m.dependsOnGroups.isEmpty()) "" else " · " + stringResource(R.string.ao_depends_groups, m.dependsOnGroups.joinToString(", "))
    ListRow(
        title = m.name,
        subtitle = via + depends,
        trailing = {
            Text(
                modeText(m.mode),
                color = if (m.mode == "none") MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary,
            )
        },
    )
}

@Composable
private fun ruleText(ruleOn: String?, hostId: String, labelOf: (String) -> String): String = when (ruleOn) {
    null -> stringResource(R.string.ao_via_vault)
    hostId -> stringResource(R.string.ao_via_rule_host)
    else -> stringResource(R.string.ao_via_rule_group, labelOf(ruleOn))
}

@Composable
private fun modeText(mode: String): String = stringResource(
    when (mode) {
        "none" -> R.string.acl_hidden
        "connect" -> R.string.acl_connect
        "read" -> R.string.acl_read
        else -> R.string.acl_edit
    },
)

@Composable
private fun loginText(h: HostPolicyCard): String {
    val kind = stringResource(
        when (h.auth.kind) {
            "opkssh" -> R.string.ao_auth_opkssh
            "ssh_id" -> R.string.ao_auth_ssh_id
            "certificate" -> R.string.ao_auth_certificate
            "key" -> R.string.ao_auth_key
            "password" -> R.string.ao_auth_password
            else -> R.string.ao_auth_none
        },
    )
    return listOf(h.auth.username, kind, h.auth.label).filter { it.isNotEmpty() }.joinToString(" · ")
}

@Composable
private fun networkText(h: HostPolicyCard): String = h.network?.label ?: stringResource(R.string.ao_direct)

@Composable
private fun jumpText(h: HostPolicyCard): String {
    if (h.jumpHosts.isEmpty()) return stringResource(R.string.ao_no_jump_hosts)
    val outside = stringResource(R.string.ao_outside_vault)
    return h.jumpHosts.joinToString(" → ") { if (it.outsideVault) outside else it.label }
}

@Composable
private fun sourceText(s: PolicySourceCard?): String = when (s?.kind) {
    null -> stringResource(R.string.ao_src_none)
    "host" -> stringResource(R.string.ao_src_host)
    "group" -> stringResource(R.string.ao_src_group, s.label)
    "tag" -> stringResource(R.string.ao_src_tag, s.label)
    else -> stringResource(R.string.ao_src_overlay, s.label)
}

@Composable
private fun riskText(r: PolicyRiskCard): String = when (r.kind) {
    "bypasses_vpn" -> stringResource(R.string.ao_risk_bypasses_vpn)
    "password_auth" -> stringResource(R.string.ao_risk_password_auth)
    "password_auth_policy" -> stringResource(R.string.ao_risk_password_auth_policy)
    "jump_outside_vault" -> stringResource(R.string.ao_risk_jump_outside_vault)
    "direct_connection_policy" -> stringResource(R.string.ao_risk_direct_connection_policy)
    "overlay_replaces_identity" -> stringResource(R.string.ao_risk_overlay_replaces_identity)
    else -> stringResource(R.string.ao_risk_unresolved, r.detail)
}
