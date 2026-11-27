package dev.labdex.labterm.ui.components

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TileColorsTest {
    @Test
    fun normalizesShortAndLongHex() {
        assertEquals("#33aa77", TileColors.normalize("#3a7"))
        assertEquals("#33aa77", TileColors.normalize(" 33AA77 "))
        assertEquals("#2bb884", TileColors.normalize("#2bb884"))
    }

    @Test
    fun rejectsNonHex() {
        assertNull(TileColors.normalize(null))
        assertNull(TileColors.normalize(""))
        assertNull(TileColors.normalize("red"))
        assertNull(TileColors.normalize("#12"))
        assertNull(TileColors.normalize("#1234567"))
        assertNull(TileColors.parse("url(https://example.com/x)"))
    }

    @Test
    fun parsesToOpaqueColour() {
        assertEquals(Color(0xFF2BB884), TileColors.parse("#2bb884"))
    }

    @Test
    fun paletteIsNormalized() {
        TileColors.palette.forEach { assertEquals(it, TileColors.normalize(it)) }
    }
}
