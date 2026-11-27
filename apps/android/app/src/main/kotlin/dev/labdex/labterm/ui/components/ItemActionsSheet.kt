package dev.labdex.labterm.ui.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R

/** One entry of an [ItemActionsSheet]. Danger entries are tinted red. */
data class SheetAction(
    val icon: ImageVector,
    val title: String,
    val danger: Boolean = false,
    val enabled: Boolean = true,
    val onClick: () -> Unit,
)

/** A labelled card of actions inside an [ItemActionsSheet]; empty sections are skipped. */
data class SheetSection(val label: String?, val actions: List<SheetAction>)

/**
 * The quick menu every list item opens, by long-press or its ⋯ button: the
 * item's tile, name and subtitle on top, then cards of actions. Hosts,
 * groups, tags, keys, identities, snippets and forwarding rules all use it,
 * so they look and behave the same. "Edit" opens the full-screen editor.
 * Each action closes the sheet first.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ItemActionsSheet(
    title: String,
    subtitle: String?,
    leading: @Composable () -> Unit,
    sections: List<SheetSection>,
    onClose: () -> Unit,
    /** Extra content under the header (badges, open sessions). */
    extra: @Composable ColumnScope.() -> Unit = {},
) {
    ModalBottomSheet(onDismissRequest = onClose) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
                .padding(bottom = 24.dp),
        ) {
            ItemSheetHeader(title, subtitle, leading)
            extra()
            sections.filter { it.actions.isNotEmpty() }.forEach { section ->
                section.label?.let { SectionLabel(it) } ?: Spacer(Modifier.padding(top = 12.dp))
                SectionCard {
                    section.actions.forEachIndexed { i, a ->
                        if (i > 0) RowDivider()
                        ActionRow(
                            a.icon,
                            a.title,
                            tint = if (a.danger) MaterialTheme.colorScheme.error else null,
                            enabled = a.enabled,
                            onClick = { onClose(); a.onClick() },
                        )
                    }
                }
            }
        }
    }
}

/** Tile, name and one-line subtitle at the top of a quick menu. */
@Composable
fun ItemSheetHeader(title: String, subtitle: String?, leading: @Composable () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        leading()
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (!subtitle.isNullOrEmpty()) {
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/** The ⋯ button at the end of a list row; opens the same menu as a long-press. */
@Composable
fun ItemMoreButton(onClick: () -> Unit, contentDescription: String = stringResource(R.string.more)) {
    IconButton(onClick = onClick, modifier = Modifier.size(32.dp)) {
        Icon(Icons.Filled.MoreVert, contentDescription = contentDescription, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Tap opens, long-press opens the quick menu (with a TalkBack label). */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Modifier.itemClicks(onClick: () -> Unit, onMenu: () -> Unit): Modifier =
    combinedClickable(onClick = onClick, onLongClick = onMenu, onLongClickLabel = stringResource(R.string.more))
