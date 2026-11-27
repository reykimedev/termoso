package dev.labdex.labterm.ui.components

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AddPhotoAlternate
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Block
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import dev.labdex.labterm.R

/** Reads a picked image and turns it into a stored custom icon (see [CustomIcon.encode]). */
private suspend fun readCustomIcon(context: Context, uri: Uri): String? = withContext(Dispatchers.IO) {
    runCatching {
        val resolver = context.contentResolver
        val mime = resolver.getType(uri)
        val bytes = resolver.openInputStream(uri)?.use { input ->
            // Stop reading past the limit instead of loading a huge file.
            val buf = java.io.ByteArrayOutputStream()
            val chunk = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(chunk)
                if (n < 0) break
                buf.write(chunk, 0, n)
                if (buf.size() > CustomIcon.MAX_FILE_BYTES) return@runCatching null
            }
            buf.toByteArray()
        } ?: return@runCatching null
        CustomIcon.encode(bytes, mime)
    }.getOrNull()
}

/**
 * Icon picker for hosts and groups: "Automatic" (`null`, the OS logo or the
 * folder), a custom image, the generic glyphs, product logos and OS logos.
 * Ids are the same on every client (see [IconCatalog]). With [onColor] it
 * also offers the tile colour (`#rrggbb`, see [TileColors]); picking a colour
 * keeps the sheet open so the icon can be picked next.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun IconPickerSheet(
    selected: String?,
    automaticLabel: String,
    onPick: (String?) -> Unit,
    onDismiss: () -> Unit,
    color: String? = null,
    onColor: ((String?) -> Unit)? = null,
) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val isCustom = CustomIcon.isCustom(selected)
    val current = if (isCustom) null else selected?.trim()?.lowercase()
    val customBitmap = rememberCustomIcon(selected)
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var imageError by remember { mutableStateOf(false) }
    val imageErrorText = stringResource(R.string.icon_custom_image_error)
    val pickImage = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) {
            scope.launch {
                val value = readCustomIcon(context, uri)
                if (value != null) onPick(value) else imageError = true
            }
        }
    }
    val tile = TileColors.parse(color)
    val title = stringResource(if (onColor != null) R.string.icon_and_colour else R.string.icon)
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
            if (onColor != null) {
                item(span = { GridItemSpan(maxLineSpan) }) { ColorRow(color, onColor) }
            }
            item {
                Choice(automaticLabel, selected == null, { onPick(null) }) {
                    if (tile != null && selected == null) {
                        IconTile(Icons.Filled.AutoAwesome, tint = Color.White, container = tile)
                    } else {
                        IconTile(Icons.Filled.AutoAwesome, selected = selected == null)
                    }
                }
            }
            item {
                Choice(stringResource(R.string.icon_custom_image), isCustom, { imageError = false; pickImage.launch("image/*") }) {
                    if (customBitmap != null) {
                        CustomIconTile(customBitmap, container = tile)
                    } else {
                        IconTile(Icons.Filled.AddPhotoAlternate)
                    }
                }
            }
            if (imageError) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        imageErrorText,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                }
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

/** "No colour", the palette and a hex field; a pick applies at once. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ColorRow(color: String?, onColor: (String?) -> Unit) {
    val current = TileColors.normalize(color)
    var hex by remember(current) { mutableStateOf(current ?: "") }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(horizontal = 4.dp)) {
        Text(
            stringResource(R.string.icon_colour),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Swatch(null, current == null, stringResource(R.string.icon_colour_none)) { onColor(null) }
            TileColors.palette.forEach { c -> Swatch(c, current == c, c) { onColor(c) } }
        }
        OutlinedTextField(
            value = hex,
            onValueChange = { v ->
                hex = v
                TileColors.normalize(v)?.let { if (it != current) onColor(it) }
            },
            label = { Text(stringResource(R.string.icon_colour_hex)) },
            singleLine = true,
            isError = hex.isNotBlank() && TileColors.normalize(hex) == null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun Swatch(hex: String?, on: Boolean, label: String, onClick: () -> Unit) {
    val fill = TileColors.parse(hex)
    Box(
        Modifier
            .size(36.dp)
            .clip(CircleShape)
            .then(if (fill != null) Modifier.background(fill) else Modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape))
            .then(if (on) Modifier.border(3.dp, MaterialTheme.colorScheme.primary, CircleShape) else Modifier)
            .clickable(onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        when {
            fill == null -> Icon(Icons.Filled.Block, contentDescription = label, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
            on -> Icon(Icons.Filled.Check, contentDescription = label, tint = Color.White, modifier = Modifier.size(20.dp))
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
