package dev.labdex.labterm.ui.components

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.Base64
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp
import androidx.core.graphics.createBitmap
import com.caverock.androidsvg.PreserveAspectRatio
import com.caverock.androidsvg.SVG
import java.io.ByteArrayOutputStream

/**
 * Custom host/group icons picked on the desktop: `Host.icon` / `Group.icon`
 * hold `custom:data:<data URL>` — a 64 px PNG (or any raster the desktop
 * accepted) or an SVG of at most 16 KiB, the whole value at most 32 KiB.
 * Same validation as `apps/desktop/src/hosts/customIcon.ts`; anything else
 * falls back to the built-in icon / folder tile.
 */
object CustomIcon {
    private const val PREFIX = "custom:data:"
    private const val MAX_LENGTH = 32 * 1024
    /** Raster size for SVGs; desktop stores rasters at 64 px as well. */
    private const val SVG_SIDE = 128
    private val DATA_URL = Regex(
        "^data:(image/(?:png|jpeg|gif|webp|svg\\+xml|x-icon|avif));base64,([a-z0-9+/]+={0,2})$",
        RegexOption.IGNORE_CASE,
    )

    private class Parsed(val mime: String, val bytes: ByteArray)

    private val cache = LruCache<String, ImageBitmap>(64)

    /** Picked files above this are refused before decoding (same as desktop). */
    const val MAX_FILE_BYTES = 8 * 1024 * 1024
    /** SVGs are stored as they are, so they stay small. */
    const val MAX_SVG_BYTES = 16 * 1024
    /** Rasters are stored as a transparent square PNG of this side. */
    const val RASTER_SIDE = 64

    /**
     * Stored value for a picked image: an SVG of at most [MAX_SVG_BYTES] as
     * it is (it is drawn by AndroidSVG, which runs no scripts and loads
     * nothing), any other image scaled into a [RASTER_SIDE] px PNG. Null
     * when the file is too big, not an image, or does not decode.
     */
    fun encode(bytes: ByteArray, mime: String?): String? {
        if (bytes.isEmpty() || bytes.size > MAX_FILE_BYTES) return null
        val type = mime?.lowercase()
        val value = if (type == "image/svg+xml" || looksLikeSvg(bytes)) {
            if (bytes.size > MAX_SVG_BYTES) return null
            if (runCatching { decodeSvg(bytes) }.getOrNull() == null) return null
            PREFIX + "data:image/svg+xml;base64," + Base64.encodeToString(bytes, Base64.NO_WRAP)
        } else {
            val png = rasterPng(bytes) ?: return null
            PREFIX + "data:image/png;base64," + Base64.encodeToString(png, Base64.NO_WRAP)
        }
        return value.takeIf { isCustom(it) }
    }

    private fun looksLikeSvg(bytes: ByteArray): Boolean {
        val head = String(bytes, 0, minOf(bytes.size, 512), Charsets.UTF_8).trimStart()
        return head.startsWith("<svg") || (head.startsWith("<?xml") && head.contains("<svg"))
    }

    private fun rasterPng(bytes: ByteArray): ByteArray? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        // Decode at the smallest power-of-two size still above the target, so a
        // photo from the camera does not need tens of MB.
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= RASTER_SIDE && bounds.outHeight / (sample * 2) >= RASTER_SIDE) sample *= 2
        val source = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
            ?: return null
        val scale = minOf(RASTER_SIDE.toFloat() / source.width, RASTER_SIDE.toFloat() / source.height)
        val w = source.width * scale
        val h = source.height * scale
        val out = createBitmap(RASTER_SIDE, RASTER_SIDE)
        val left = (RASTER_SIDE - w) / 2
        val top = (RASTER_SIDE - h) / 2
        Canvas(out).drawBitmap(source, null, RectF(left, top, left + w, top + h), Paint(Paint.FILTER_BITMAP_FLAG))
        source.recycle()
        return ByteArrayOutputStream().use { stream ->
            out.compress(Bitmap.CompressFormat.PNG, 100, stream)
            stream.toByteArray()
        }
    }

    /** True for a well-formed custom icon value. */
    fun isCustom(value: String?): Boolean = parse(value) != null

    private fun parse(value: String?): Parsed? {
        if (value == null || !value.startsWith(PREFIX) || value.length > MAX_LENGTH + PREFIX.length) return null
        val m = DATA_URL.matchEntire(value.substring(PREFIX.length)) ?: return null
        val payload = m.groupValues[2]
        if (payload.isEmpty()) return null
        val bytes = try {
            Base64.decode(payload, Base64.DEFAULT)
        } catch (_: IllegalArgumentException) {
            return null
        }
        return Parsed(m.groupValues[1].lowercase(), bytes)
    }

    /** Decoded bitmap for [value], or null when it is not a usable custom icon. */
    fun bitmap(value: String?): ImageBitmap? {
        val parsed = parse(value) ?: return null
        cache.get(value)?.let { return it }
        val bitmap = try {
            if (parsed.mime == "image/svg+xml") decodeSvg(parsed.bytes) else BitmapFactory.decodeByteArray(parsed.bytes, 0, parsed.bytes.size)
        } catch (_: Exception) {
            null
        } ?: return null
        return bitmap.asImageBitmap().also { cache.put(value, it) }
    }

    private fun decodeSvg(bytes: ByteArray): Bitmap? {
        val svg = SVG.getFromString(String(bytes, Charsets.UTF_8))
        if (svg.documentWidth <= 0f || svg.documentHeight <= 0f) {
            svg.setDocumentWidth("100%")
            svg.setDocumentHeight("100%")
        }
        val bitmap = createBitmap(SVG_SIDE, SVG_SIDE)
        svg.setDocumentPreserveAspectRatio(PreserveAspectRatio.LETTERBOX)
        svg.renderToCanvas(Canvas(bitmap), RectF(0f, 0f, SVG_SIDE.toFloat(), SVG_SIDE.toFloat()))
        return bitmap
    }
}

/** Rounded tile showing a custom icon image on [container], or a neutral background. */
@Composable
fun CustomIconTile(bitmap: ImageBitmap, modifier: Modifier = Modifier, size: Int = 40, container: Color? = null) {
    Box(
        modifier = modifier
            .size(size.dp)
            .clip(RoundedCornerShape((size * 0.25f).dp))
            .background(container ?: MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            bitmap = bitmap,
            contentDescription = null,
            modifier = Modifier.size((size * 0.7f).dp).padding(1.dp),
        )
    }
}

/** Decoded custom icon for [value], remembered per value; null for built-in ids. */
@Composable
fun rememberCustomIcon(value: String?): ImageBitmap? = remember(value) { CustomIcon.bitmap(value) }
