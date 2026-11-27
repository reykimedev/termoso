package dev.labdex.labterm.ui.hosts

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.DriveFileRenameOutline
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Sell
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.TagItem
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.EmptyState
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ItemActionsSheet
import dev.labdex.labterm.ui.components.ItemMoreButton
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.components.SheetAction
import dev.labdex.labterm.ui.components.SheetSection
import dev.labdex.labterm.ui.components.SubScreen
import dev.labdex.labterm.ui.components.itemClicks
import dev.labdex.labterm.ui.shell.ShellViewModel
import kotlinx.coroutines.launch

private sealed interface TagDialog {
    data class Menu(val tag: TagItem) : TagDialog
    data class Edit(val tag: TagItem) : TagDialog
    data class Rename(val tag: TagItem) : TagDialog
    data class Delete(val tag: TagItem) : TagDialog
}

/**
 * The tags of the selected vault as a page of their own, like Hosts: tap a
 * tag for its hosts, ⋯ or long-press for its quick menu (show hosts, edit
 * on the full-screen page, rename, delete).
 */
@Composable
fun TagsScreen(shell: ShellViewModel, onBack: () -> Unit, onOpenTag: (String) -> Unit) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var tags by remember { mutableStateOf<List<TagItem>>(emptyList()) }
    var dialog by remember { mutableStateOf<TagDialog?>(null) }
    LaunchedEffect(vaultId, revision) {
        runCatching { shell.repo.read { tags(vaultId) } }
            .onSuccess { tags = it.sortedBy { t -> t.label.lowercase() } }
            .onFailure { shell.notify(it.userMessage()) }
    }

    fun write(block: suspend () -> Unit) {
        scope.launch { runCatching { block() }.onFailure { shell.notify(it.userMessage()) } }
    }

    SubScreen(stringResource(R.string.tags), onBack) { padding ->
        if (tags.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.no_tags_yet),
                    hint = stringResource(R.string.tag_settings_hint),
                    icon = Icons.Filled.Sell,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    tags.forEachIndexed { i, t ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = t.label,
                            subtitle = tagSubtitle(t),
                            leading = { IconTile(Icons.Filled.Sell) },
                            modifier = Modifier.itemClicks(onClick = { onOpenTag(t.label) }, onMenu = { dialog = TagDialog.Menu(t) }),
                        ) {
                            ItemMoreButton({ dialog = TagDialog.Menu(t) }, stringResource(R.string.tag_actions))
                        }
                    }
                }
            }
        }
    }

    when (val d = dialog) {
        null -> Unit
        is TagDialog.Menu -> ItemActionsSheet(
            title = d.tag.label,
            subtitle = tagSubtitle(d.tag),
            leading = { IconTile(Icons.Filled.Sell) },
            sections = listOf(
                SheetSection(
                    stringResource(R.string.tag),
                    listOf(
                        SheetAction(Icons.Filled.Dns, stringResource(R.string.show_hosts)) { onOpenTag(d.tag.label) },
                        SheetAction(Icons.Filled.Edit, stringResource(R.string.edit)) { dialog = TagDialog.Edit(d.tag) },
                        SheetAction(Icons.Filled.DriveFileRenameOutline, stringResource(R.string.rename)) { dialog = TagDialog.Rename(d.tag) },
                    ),
                ),
                SheetSection(null, listOf(SheetAction(Icons.Filled.Delete, stringResource(R.string.delete), danger = true) { dialog = TagDialog.Delete(d.tag) })),
            ),
            onClose = { if (dialog == d) dialog = null },
        )
        is TagDialog.Edit -> DefaultsEditorDialog(shell = shell, target = DefaultsTarget.Tag(d.tag.id), onDismiss = { dialog = null })
        is TagDialog.Rename -> NameDialog(
            title = stringResource(R.string.rename_tag),
            initial = d.tag.label,
            onConfirm = { label -> dialog = null; write { shell.repo.write { saveTag(d.tag.id, label.trim()) } } },
            onDismiss = { dialog = null },
        )
        is TagDialog.Delete -> ConfirmDialog(
            title = stringResource(R.string.delete_tag_title, d.tag.label),
            text = stringResource(R.string.delete_tag_text),
            confirm = stringResource(R.string.delete),
            onConfirm = { dialog = null; write { shell.repo.write { deleteTag(d.tag.id) } } },
            onDismiss = { dialog = null },
        )
    }
}
