package dev.labdex.labterm.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.core.TeamCard
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.core.VaultKind

/** The kind's own glyph: a person (personal), a phone (local), a group (team). */
fun vaultKindVector(kind: VaultKind): ImageVector = when (kind) {
    VaultKind.PERSONAL -> Icons.Filled.Person
    VaultKind.LOCAL -> Icons.Filled.PhoneAndroid
    VaultKind.TEAM -> Icons.Filled.Group
}

/**
 * Tile with a stored look (`color` `#rrggbb`, `icon` built-in id or
 * `custom:data:` image, the same values hosts and groups use) and a fallback
 * glyph. [outlined] draws the colour as a border on a clear tile.
 */
@Composable
fun StyledTile(
    icon: String?,
    color: String?,
    fallback: ImageVector,
    modifier: Modifier = Modifier,
    size: Int = 40,
    outlined: Boolean = false,
) {
    val tile = remember(color) { TileColors.parse(color) }
    val picked = remember(icon) { IconCatalog.picked(icon) }
    val custom = rememberCustomIcon(icon)
    val shape = RoundedCornerShape((size * 0.25f).dp)
    val fill = if (outlined) Color.Transparent else tile ?: picked?.color ?: MaterialTheme.colorScheme.surfaceContainerHighest
    val tint = when {
        outlined -> tile ?: MaterialTheme.colorScheme.onSurfaceVariant
        tile != null || picked != null -> Color.White
        else -> MaterialTheme.colorScheme.onSurface
    }
    Box(
        modifier
            .size(size.dp)
            .clip(shape)
            .background(fill)
            .then(if (outlined) Modifier.border(1.5.dp, tile ?: MaterialTheme.colorScheme.outlineVariant, shape) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        when {
            custom != null -> Image(
                bitmap = custom,
                contentDescription = null,
                modifier = Modifier.size((size * 0.7f).dp),
            )
            picked != null -> Icon(distroVector(picked), contentDescription = null, tint = tint, modifier = Modifier.size((size * 0.6f).dp))
            else -> Icon(fallback, contentDescription = null, tint = tint, modifier = Modifier.size((size * 0.55f).dp))
        }
    }
}

/**
 * Vault tile: its colour and icon (falling back to the kind's glyph), a small
 * person / phone badge in the corner for personal and local vaults, dimmed
 * with a lock while locked. `pinned = false` draws it outlined in the vault
 * colour (unpinned rows in the navigation); the pin badge is the caller's.
 */
@Composable
fun VaultTile(vault: VaultInfo, modifier: Modifier = Modifier, size: Int = 40, pinned: Boolean? = null) {
    Box(modifier) {
        if (vault.locked) {
            Box(Modifier.alpha(0.55f)) { StyledTile(null, null, Icons.Filled.Lock, size = size) }
        } else {
            StyledTile(vault.icon, vault.color, vaultKindVector(vault.kind), size = size, outlined = pinned == false)
        }
        if (vault.kind != VaultKind.TEAM) {
            val badge = maxOf(12, (size * 0.42f).toInt())
            Box(
                Modifier
                    .align(Alignment.BottomEnd)
                    .offset(2.dp, 2.dp)
                    .size(badge.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    vaultKindVector(vault.kind),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size((badge * 0.7f).dp),
                )
            }
        }
    }
}

/** Team tile: the team's colour and icon, else the group glyph. */
@Composable
fun TeamTile(team: TeamCard, modifier: Modifier = Modifier, size: Int = 40) {
    StyledTile(team.icon, team.color, Icons.Filled.Group, modifier = modifier, size = size)
}
