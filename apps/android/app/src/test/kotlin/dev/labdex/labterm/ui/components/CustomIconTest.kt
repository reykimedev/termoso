package dev.labdex.labterm.ui.components

import android.graphics.Bitmap
import android.util.Base64
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.util.zip.CRC32
import java.util.zip.Deflater
import java.util.zip.DeflaterOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import dev.labdex.labterm.ResourceTest

class CustomIconTest : ResourceTest() {
    private fun png(side: Int): String {
        val bitmap = Bitmap.createBitmap(side, side, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(0xFF336699.toInt())
        val out = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
        return "custom:data:data:image/png;base64," + Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP)
    }

    @Test
    fun decodesPng() {
        val value = png(64)
        assertTrue(CustomIcon.isCustom(value))
        val bitmap = CustomIcon.bitmap(value)
        assertNotNull(bitmap)
        assertEquals(64, bitmap!!.width)
    }

    @Test
    fun decodesSvg() {
        val svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"""
        val value = "custom:data:data:image/svg+xml;base64," + Base64.encodeToString(svg.toByteArray(), Base64.NO_WRAP)
        assertNotNull(CustomIcon.bitmap(value))
    }

    @Test
    fun rejectsBuiltInIdsUrlsAndJunk() {
        assertFalse(CustomIcon.isCustom("ubuntu"))
        assertFalse(CustomIcon.isCustom(null))
        assertFalse(CustomIcon.isCustom("custom:url:https://example.com/a.png"))
        assertFalse(CustomIcon.isCustom("custom:data:data:text/html;base64,PGI+"))
        // Over the 32 KiB limit: rejected before decoding.
        assertNull(CustomIcon.bitmap("custom:data:data:image/png;base64," + "A".repeat(40 * 1024)))
        assertNull(CustomIcon.bitmap("custom:data:data:image/png;base64,"))
    }

    @Test
    fun aStoredRasterIsNeverDecodedAtFullSize() {
        // What another client (or a hand-made value) can sync: small in bytes, large in pixels.
        val large = CustomIcon.bitmap(png(1024))
        assertNotNull(large)
        assertTrue(large!!.width < CustomIcon.MAX_SHOWN_SIDE * 2)
        val huge = hugePng(CustomIcon.MAX_SOURCE_SIDE * 2)
        assertTrue(CustomIcon.isCustom(huge))
        assertNull(CustomIcon.bitmap(huge))
    }

    /** A one-bit grey PNG of [side] px: a few KiB that would decode to hundreds of MB. */
    private fun hugePng(side: Int): String {
        fun chunk(out: ByteArrayOutputStream, type: String, data: ByteArray) {
            val body = type.toByteArray(Charsets.US_ASCII) + data
            val crc = CRC32().apply { update(body) }.value.toInt()
            out.write(ByteBuffer.allocate(4).putInt(data.size).array())
            out.write(body)
            out.write(ByteBuffer.allocate(4).putInt(crc).array())
        }
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))
        // Bit depth 1, colour type 0 (grey), no interlace.
        chunk(out, "IHDR", ByteBuffer.allocate(13).putInt(side).putInt(side).put(1).put(0).put(0).put(0).put(0).array())
        val pixels = ByteArrayOutputStream()
        DeflaterOutputStream(pixels, Deflater(Deflater.BEST_COMPRESSION)).use { z ->
            val row = ByteArray(1 + (side + 7) / 8)
            repeat(side) { z.write(row) }
        }
        chunk(out, "IDAT", pixels.toByteArray())
        chunk(out, "IEND", ByteArray(0))
        return "custom:data:data:image/png;base64," + Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP)
    }

    @Test
    fun encodesLargeRasterAsSmallPng() {
        val bitmap = Bitmap.createBitmap(1200, 600, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(0xFF336699.toInt())
        val out = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
        val value = CustomIcon.encode(out.toByteArray(), "image/png")
        assertNotNull(value)
        assertTrue(value!!.startsWith("custom:data:data:image/png;base64,"))
        assertEquals(CustomIcon.RASTER_SIDE, CustomIcon.bitmap(value)!!.width)
    }

    @Test
    fun keepsSmallSvgAndRefusesBigOrBrokenFiles() {
        val svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"""
        val value = CustomIcon.encode(svg.toByteArray(), "image/svg+xml")
        assertTrue(value!!.startsWith("custom:data:data:image/svg+xml;base64,"))
        // Some file pickers report SVGs as octet-stream.
        assertNotNull(CustomIcon.encode(svg.toByteArray(), "application/octet-stream"))
        val big = svg.replace("<rect", "<!--" + "x".repeat(CustomIcon.MAX_SVG_BYTES) + "--><rect")
        assertNull(CustomIcon.encode(big.toByteArray(), "image/svg+xml"))
        assertNull(CustomIcon.encode("not an image".toByteArray(), "image/png"))
        assertNull(CustomIcon.encode(ByteArray(0), "image/png"))
    }
}
