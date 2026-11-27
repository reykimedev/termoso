package dev.labdex.labterm.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.data.CommandTarget
import dev.labdex.labterm.data.SessionManager
import dev.labdex.labterm.data.TabGroup
import dev.labdex.labterm.data.TerminalSession
import dev.labdex.labterm.data.hostGroupPaths
import dev.labdex.labterm.data.tagGroupNames

@Composable
fun TabGroupDot(group: TabGroup, size: Int = 10) {
    Box(Modifier.size(size.dp).background(Color(group.color), CircleShape))
}

/** Tick the tab groups this session is in, or make a new one. A session can be in several. */
@Composable
fun TabGroupsDialog(sessions: SessionManager, session: TerminalSession, onDismiss: () -> Unit) {
    val groups by sessions.tabGroups.collectAsStateWithLifecycle()
    val mine by session.tabGroupIds.collectAsStateWithLifecycle()
    var newName by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.tab_groups)) },
        text = {
            Column {
                groups.values.sortedBy { it.name.lowercase() }.forEach { g ->
                    val member = g.id in mine
                    Row(
                        Modifier.fillMaxWidth().clickable { sessions.setInTabGroup(session.id, g.id, !member) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = member, onCheckedChange = { sessions.setInTabGroup(session.id, g.id, it) })
                        TabGroupDot(g)
                        Spacer(Modifier.width(8.dp))
                        Text(g.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                OutlinedTextField(
                    value = newName,
                    onValueChange = { newName = it },
                    singleLine = true,
                    label = { Text(stringResource(R.string.new_tab_group)) },
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    trailingIcon = {
                        TextButton(enabled = newName.isNotBlank(), onClick = {
                            sessions.setInTabGroup(session.id, sessions.ensureTabGroup(newName.trim()), true)
                            newName = ""
                        }) { Text(stringResource(R.string.add)) }
                    },
                )
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.done)) } },
    )
}

/**
 * Where the compose sheet sends: this session, all sessions, a tab group, a
 * host group or a tag (the last two come from the open sessions' hosts,
 * parents included). Each session gets a line once.
 */
@Composable
fun CommandTargetPicker(sessions: SessionManager) {
    val target by sessions.commandTarget.collectAsStateWithLifecycle()
    val groups by sessions.tabGroups.collectAsStateWithLifecycle()
    val open by sessions.sessions.collectAsStateWithLifecycle()
    var expanded by remember { mutableStateOf(false) }
    val tags = remember(open) { tagGroupNames(open.flatMap { it.hostInfo.tags }) }
    val hostGroups = remember(open) { hostGroupPaths(open.map { it.hostInfo.groupPath }) }
    val label = when (val t = target) {
        CommandTarget.Active -> stringResource(R.string.this_session)
        CommandTarget.All -> stringResource(R.string.all_sessions)
        is CommandTarget.Group -> groups[t.id]?.name ?: stringResource(R.string.this_session)
        is CommandTarget.Tag -> "#" + t.tag
        is CommandTarget.HostGroup -> t.path.joinToString(" / ")
    }
    fun pick(t: CommandTarget) {
        sessions.commandTarget.value = t
        expanded = false
    }
    Box {
        OutlinedButton(onClick = { expanded = true }) {
            Text(stringResource(R.string.send_to_target, label), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.this_session)) }, onClick = { pick(CommandTarget.Active) })
            DropdownMenuItem(text = { Text(stringResource(R.string.all_sessions)) }, onClick = { pick(CommandTarget.All) })
            if (groups.isNotEmpty()) {
                HorizontalDivider()
                MenuHeader(stringResource(R.string.tab_groups))
                groups.values.sortedBy { it.name.lowercase() }.forEach { g ->
                    DropdownMenuItem(
                        text = { Text(g.name) },
                        leadingIcon = { TabGroupDot(g) },
                        onClick = { pick(CommandTarget.Group(g.id)) },
                    )
                }
            }
            if (hostGroups.isNotEmpty()) {
                HorizontalDivider()
                MenuHeader(stringResource(R.string.host_groups))
                hostGroups.forEach { p ->
                    DropdownMenuItem(
                        text = { Text(p.last(), Modifier.padding(start = (12 * (p.size - 1)).dp)) },
                        onClick = { pick(CommandTarget.HostGroup(p)) },
                    )
                }
            }
            if (tags.isNotEmpty()) {
                HorizontalDivider()
                MenuHeader(stringResource(R.string.tags))
                tags.forEach { t ->
                    DropdownMenuItem(
                        text = { Text("#$t", Modifier.padding(start = (12 * t.count { it == '.' }).dp)) },
                        onClick = { pick(CommandTarget.Tag(t)) },
                    )
                }
            }
        }
    }
}

@Composable
private fun MenuHeader(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
    )
}
