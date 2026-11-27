package dev.labdex.labterm.ui.components

import androidx.compose.ui.graphics.Color

/**
 * Tile colours for hosts and groups (`Host.color` / `Group.color`, synced as
 * `#rrggbb`). The palette matches the desktop's tag palette
 * (`apps/desktop/src/hosts/TagChip.tsx`), so a pick looks the same everywhere.
 */
object TileColors {
    val palette = listOf(
        "#2bb884",
        "#5aa9e6",
        "#b07cf2",
        "#f25e61",
        "#f8aa4b",
        "#e3c04b",
        "#4bc8c1",
        "#ef7fb6",
        "#8d99ae",
    )

    private val HEX = Regex("^#?([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$")

    /** `#rrggbb` for user input like `#3a7`, `33aa77` or ` #33AA77 `; null when it isn't a hex colour. */
    fun normalize(input: String?): String? {
        val m = HEX.matchEntire(input?.trim() ?: return null) ?: return null
        val hex = m.groupValues[1].lowercase()
        return "#" + if (hex.length == 3) hex.map { "$it$it" }.joinToString("") else hex
    }

    /** Compose colour of a stored value, or null for none or a malformed one. */
    fun parse(value: String?): Color? {
        val hex = normalize(value) ?: return null
        return Color(0xFF000000 or hex.substring(1).toLong(16))
    }
}
