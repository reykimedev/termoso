package dev.labdex.labterm.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R

/**
 * Icon picker for hosts and groups: "Automatic" (`null`, the OS logo or the
 * folder), the generic glyphs, product logos and OS logos. Ids are the same
 * on every client (see [IconCatalog]).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IconPickerSheet(selected: String?, automaticLabel: String, onPick: (String?) -> Unit, onDismiss: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val current = selected?.trim()?.lowercase()
    val title = stringResource(R.string.icon)
    val generic = stringResource(R.string.icon_section_generic)
    val apps = stringResource(R.string.icon_section_apps)
    val systems = stringResource(R.string.icon_section_systems)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(76.dp),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) { Header(title) }
            item {
                Choice(automaticLabel, current == null, { onPick(null) }) { IconTile(Icons.Filled.AutoAwesome, selected = current == null) }
            }
            section(generic, IconCatalog.generic, current, onPick)
            section(apps, IconCatalog.brands, current, onPick)
            section(systems, DistroIcons.all, current, onPick)
            item(span = { GridItemSpan(maxLineSpan) }) { Spacer(Modifier.height(24.dp)) }
        }
    }
}

private fun androidx.compose.foundation.lazy.grid.LazyGridScope.section(
    title: String,
    icons: List<DistroIcon>,
    current: String?,
    onPick: (String?) -> Unit,
) {
    item(span = { GridItemSpan(maxLineSpan) }) { Header(title) }
    items(icons, key = { it.id }) { icon ->
        Choice(icon.title, current == icon.id, { onPick(icon.id) }) {
            IconTile(distroVector(icon), tint = Color.White, container = icon.color, selected = current == icon.id)
        }
    }
}

@Composable
private fun Header(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 4.dp, top = 12.dp, bottom = 4.dp),
    )
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit, tile: @Composable () -> Unit) {
    Column(
        Modifier
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp, horizontal = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        tile()
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
    }
}
