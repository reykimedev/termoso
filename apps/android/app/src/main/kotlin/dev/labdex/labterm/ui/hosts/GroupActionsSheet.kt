package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.DriveFileMove
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DriveFileRenameOutline
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Sell
import androidx.compose.material.icons.filled.VpnLock
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.GroupItem
import dev.labdex.labterm.core.HostVpnItem
import dev.labdex.labterm.core.TagItem
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.ui.components.CustomIconTile
import dev.labdex.labterm.ui.components.IconCatalog
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.distroVector
import dev.labdex.labterm.ui.components.rememberCustomIcon

/** A group's tile: its picked icon, a custom one, or the folder. */
@Composable
fun GroupTile(g: GroupItem) {
    val picked = remember(g.icon) { IconCatalog.picked(g.icon) }
    val custom = rememberCustomIcon(g.icon)
    when {
        picked != null -> IconTile(distroVector(picked), tint = Color.White, container = picked.color)
        custom != null -> CustomIconTile(custom)
        else -> IconTile(Icons.Filled.Folder)
    }
}

/**
 * The quick menu of a group, the same sheet hosts use: open it, edit it on
 * the full-screen page, pick its VPN network, rename, duplicate, move, and
 * delete. The header shows the network the group's hosts go through.
 */
@Composable
fun GroupActionsSheet(
    repo: VaultRepository,
    group: GroupItem,
    onOpen: () -> Unit,
    onEdit: () -> Unit,
    onVpn: () -> Unit,
    onRename: () -> Unit,
    onDuplicate: () -> Unit,
    onMove: () -> Unit,
    onDelete: () -> Unit,
    onClose: () -> Unit,
) {
    // The group's own network, with its parents' as a fallback (inherited).
    val vpn by produceState<HostVpnItem?>(null, group.id) {
        value = runCatching {
            repo.read {
                val own = groupTransport(group.id)
                val inherited = if (own == null) group.parentId?.let { inherited(it).transportId } else null
                val id = own ?: inherited ?: return@read null
                transports(group.vaultId).firstOrNull { it.id == id }?.let {
                    HostVpnItem(id = id, label = it.label, provider = it.provider, inherited = own == null)
                }
            }
        }.getOrNull()
    }
    ItemActionsSheet(
        title = group.label,
        subtitle = groupSubtitle(group),
        leading = { GroupTile(group) },
        extra = { vpn?.let { HostFactChips(it, null, emptyList(), onVpn = null) } },
        sections = listOf(
            SheetSection(
                stringResource(R.string.group),
                listOf(
                    SheetAction(Icons.Filled.FolderOpen, stringResource(R.string.open), onClick = onOpen),
                    SheetAction(Icons.Filled.Edit, stringResource(R.string.edit), onClick = onEdit),
                    SheetAction(Icons.Filled.VpnLock, stringResource(R.string.vpn_network), onClick = onVpn),
                    SheetAction(Icons.Filled.DriveFileRenameOutline, stringResource(R.string.rename), onClick = onRename),
                    SheetAction(Icons.Filled.ContentCopy, stringResource(R.string.duplicate), onClick = onDuplicate),
                    SheetAction(Icons.AutoMirrored.Filled.DriveFileMove, stringResource(R.string.move_to_group), onClick = onMove),
                ),
            ),
            SheetSection(
                null,
                listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true, onClick = onDelete)),
            ),
        ),
        onClose = onClose,
    )
}

/** Delete a group: keep its contents (moved up one level) or delete them too. */
@Composable
fun DeleteGroupDialog(group: GroupItem, onConfirm: (withContents: Boolean) -> Unit, onDismiss: () -> Unit) {
    val empty = group.hostCount == 0u && group.groupCount == 0u
    var withContents by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.delete_group_title, group.label)) },
        text = {
            Column {
                if (empty) {
                    Text(stringResource(R.string.delete_group_empty))
                } else {
                    Text(groupSubtitle(group).orEmpty(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    ChoiceLine(stringResource(R.string.delete_group_keep_contents), !withContents) { withContents = false }
                    ChoiceLine(stringResource(R.string.delete_group_with_contents), withContents) { withContents = true }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(!empty && withContents) }) {
                Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun ChoiceLine(text: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = onClick)
        Text(text, style = MaterialTheme.typography.bodyMedium)
    }
}

/** Quick menu of a tag: edit (full-screen page with its settings), rename, delete. */
@Composable
fun TagActionsSheet(
    tag: TagItem,
    onEdit: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    onClose: () -> Unit,
) {
    ItemActionsSheet(
        title = tag.label,
        subtitle = tagSubtitle(tag),
        leading = { IconTile(Icons.Filled.Sell) },
        sections = listOf(
            SheetSection(
                stringResource(R.string.tag),
                listOf(
                    SheetAction(Icons.Filled.Edit, stringResource(R.string.edit), onClick = onEdit),
                    SheetAction(Icons.Filled.DriveFileRenameOutline, stringResource(R.string.rename), onClick = onRename),
                ),
            ),
            SheetSection(
                null,
                listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true, onClick = onDelete)),
            ),
        ),
        onClose = onClose,
    )
}

@Composable
fun tagSubtitle(tag: TagItem): String = listOfNotNull(
    pluralStringResource(R.plurals.tag_host_count, tag.hosts.toInt(), tag.hosts.toInt()),
    if (tag.hasConfig) stringResource(R.string.tag_has_settings) else null,
).joinToString(" · ")

/** Groups `group` may move into: all but itself and its own sub-groups. */
fun parentCandidates(groups: List<GroupItem>, selfId: String): List<GroupItem> {
    val blocked = mutableSetOf(selfId)
    var grew = true
    while (grew) {
        grew = false
        for (g in groups) {
            val parent = g.parentId
            if (parent != null && parent in blocked && blocked.add(g.id)) grew = true
        }
    }
    return groups.filter { it.id !in blocked }
}
