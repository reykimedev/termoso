package com.termoso.android.ui.theme

import androidx.compose.ui.graphics.Color

// Brand palette shared with the desktop client (apps/desktop/src/theme/theme.ts):
// labdex's lantern orange and Blueprint neutrals. Text on the orange is dark ink.
val Lantern = Color(0xFFF59A23)
val LanternDark = Color(0xFFE8870E)
val LanternLight = Color(0xFFF7A93F)
val LanternInk = Color(0xFF1A1206)
val Sky = Color(0xFF5AA9FF)
val SkyDark = Color(0xFF2563EB)

// Old names, kept so code from before the Labterm rebrand still builds.
val Emerald = Lantern
val EmeraldDark = LanternDark
val EmeraldLight = LanternLight

object DarkLayers {
    val lowest = Color(0xFF0B0B0A)
    val base = Color(0xFF141413)
    val high = Color(0xFF1E1E1C)
    val highest = Color(0xFF272725)
    val strong = Color(0xFF363633)
    val disabled = Color(0xFF5F5F5C)
    val secondary = Color(0xFFA3A39E)
    val text = Color(0xFFECECEA)
}

object LightLayers {
    val lowest = Color(0xFFF0F0ED)
    val base = Color(0xFFF7F7F5)
    val high = Color(0xFFFFFFFF)
    val highest = Color(0xFFE3E3E0)
    val strong = Color(0xFFD3D3CF)
    val disabled = Color(0xFF9A9A96)
    val secondary = Color(0xFF5F5F5C)
    val text = Color(0xFF1A1A19)
}

val Danger = Color(0xFFF2555A)
val DangerLight = Color(0xFFC2410C)
val Warning = Color(0xFFF5C84C)
