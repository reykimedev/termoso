package dev.labdex.labterm.ui.vault

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Apps
import androidx.compose.material.icons.filled.Laptop
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.core.VaultKind
import dev.labdex.labterm.ui.components.TileColors

/** A stable colour per vault until the vault carries its own. */
private fun tileColor(v: VaultInfo): Color {
    val i = Math.floorMod(v.id.hashCode(), TileColors.palette.size)
    return TileColors.parse(TileColors.palette[i]) ?: Color.Gray
}

private fun initials(name: String): String {
    val words = name.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
    val s = if (words.size > 1) "${words[0].first()}${words[1].first()}" else (words.firstOrNull() ?: "?").take(2)
    return s.uppercase()
}

/**
 * A vault's tile. The kind is a corner badge (person = Personal, laptop =
 * Local, none = team), never the icon or colour, which every vault may set.
 * Pinned: filled with a pin badge; unpinned: outlined.
 */
@Composable
fun VaultTile(v: VaultInfo, pinned: Boolean, active: Boolean, size: Dp = 40.dp) {
    val color = tileColor(v)
    val shape = RoundedCornerShape(size * 0.28f)
    Box(Modifier.size(size + 6.dp), contentAlignment = Alignment.Center) {
        Box(
            Modifier
                .size(size)
                .then(if (active) Modifier.border(2.dp, MaterialTheme.colorScheme.onSurface, RoundedCornerShape(size * 0.34f)).padding(3.dp) else Modifier)
                .clip(shape)
                .background(if (pinned) color else Color.Transparent)
                .then(if (pinned) Modifier else Modifier.border(BorderStroke(1.5.dp, color), shape)),
            contentAlignment = Alignment.Center,
        ) {
            if (v.locked) {
                Icon(Icons.Filled.Lock, contentDescription = null, tint = if (pinned) Color.White else color, modifier = Modifier.size(size * 0.45f))
            } else {
                Text(
                    initials(vaultLabel(v)),
                    color = if (pinned) Color.White else color,
                    fontWeight = FontWeight.Bold,
                    fontSize = (size.value * 0.34f).sp,
                )
            }
        }
        val badge = when (v.kind) {
            VaultKind.PERSONAL -> Icons.Filled.Person
            VaultKind.LOCAL -> Icons.Filled.Laptop
            VaultKind.TEAM -> null
        }
        if (badge != null) {
            Surface(
                shape = CircleShape,
                color = MaterialTheme.colorScheme.onSurface,
                border = BorderStroke(2.dp, MaterialTheme.colorScheme.surface),
                modifier = Modifier.align(Alignment.BottomEnd).size(18.dp),
            ) {
                Icon(badge, contentDescription = null, tint = MaterialTheme.colorScheme.surface, modifier = Modifier.padding(2.dp))
            }
        }
        if (pinned) {
            Icon(
                Icons.Filled.PushPin,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.align(Alignment.TopEnd).offset(x = 2.dp, y = (-2).dp).size(14.dp).rotate(30f),
            )
        }
    }
}

/**
 * Full-width vault bar under a page title: pinned vaults first as filled
 * tiles, a divider, the other vaults outlined, then "All" for the switcher
 * sheet (search-free list with pin toggles). Shown on every page that
 * belongs to a vault.
 */
@Composable
fun VaultBar(
    vaults: List<VaultInfo>,
    selectedId: String?,
    pinned: List<String>,
    onPick: (String) -> Unit,
    onTogglePin: (String) -> Unit,
    modifier: Modifier = Modifier,
    /** Filter mode (Connections): "All" clears the filter instead of opening the switcher. */
    onAll: (() -> Unit)? = null,
) {
    if (vaults.size < 2) return
    val allActive = onAll != null && selectedId == null
    var sheet by remember { mutableStateOf(false) }
    val (top, rest) = splitPinned(vaults, pinned)
    Column(modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceContainerLowest)) {
        HorizontalDivider()
        Row(
            Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 8.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            verticalAlignment = Alignment.Bottom,
        ) {
            top.forEach { BarItem(it, true, it.id == selectedId, onPick) }
            if (top.isNotEmpty() && rest.isNotEmpty()) {
                VerticalDivider(Modifier.height(40.dp).padding(horizontal = 4.dp).align(Alignment.CenterVertically))
            }
            rest.forEach { BarItem(it, false, it.id == selectedId, onPick) }
            Column(
                Modifier
                    .width(64.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(role = if (onAll != null) Role.Tab else Role.Button) {
                        if (onAll != null) onAll() else sheet = true
                    }
                    .semantics { selected = allActive }
                    .padding(vertical = 2.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Box(
                    Modifier
                        .size(46.dp)
                        .then(if (allActive) Modifier.border(2.dp, MaterialTheme.colorScheme.onSurface, RoundedCornerShape(13.dp)) else Modifier)
                        .padding(3.dp)
                        .clip(RoundedCornerShape(10.dp))
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.Filled.Apps, contentDescription = null)
                }
                Text(stringResource(R.string.vault_bar_all), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        HorizontalDivider()
    }
    if (sheet) {
        VaultPickerSheet(
            vaults = vaults,
            selectedId = selectedId,
            pinned = pinned,
            onPick = onPick,
            onTogglePin = onTogglePin,
            onDismiss = { sheet = false },
        )
    }
}

@Composable
private fun BarItem(v: VaultInfo, pinned: Boolean, active: Boolean, onPick: (String) -> Unit) {
    val label = vaultLabel(v)
    Column(
        Modifier
            .width(64.dp)
            .clip(RoundedCornerShape(8.dp))
            .clickable(enabled = !v.locked, role = Role.Tab) { onPick(v.id) }
            .semantics {
                selected = active
                contentDescription = label
            }
            .padding(vertical = 2.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        VaultTile(v, pinned = pinned, active = active, size = if (pinned) 40.dp else 34.dp)
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal,
            color = if (active) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
