package com.mysound.android.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density

/** ชุดสีเดียวกับแอปบนคอม (THEMES ใน look.ts) */
class ThemeDef(val name: String, val accent: Color, val base: Color, val raised: Color, val outer: Color)

val Themes = listOf(
    ThemeDef("Turquoise", Color(0xFF5EEAD4), Color(0xFF0F1115), Color(0xFF181B21), Color(0xFF000000)),
    ThemeDef("Midnight", Color(0xFF7AA2FF), Color(0xFF0D1321), Color(0xFF162036), Color(0xFF060A14)),
    ThemeDef("Rose", Color(0xFFFF7AA8), Color(0xFF1A1015), Color(0xFF26171E), Color(0xFF0D0709)),
    ThemeDef("Amber", Color(0xFFFBBF24), Color(0xFF17130D), Color(0xFF231C12), Color(0xFF0C0905)),
    ThemeDef("Forest", Color(0xFF86EFAC), Color(0xFF0E1511), Color(0xFF16221B), Color(0xFF060B08)),
    ThemeDef("Light", Color(0xFF0D9488), Color(0xFFF6F7F9), Color(0xFFFFFFFF), Color(0xFFE1E5EA)),
)

/** เลย์เอาต์สามแบบเหมือนบนคอม: ความสูงแถวเพลง มุมโค้ง ขนาดตัวอักษร */
class LayoutDef(val name: String, val hint: String, val radius: Int, val row: Int, val font: Int)

val Layouts = listOf(
    LayoutDef("Classic", "Balanced, like the default", 8, 56, 100),
    LayoutDef("Compact", "More songs per screen", 6, 44, 94),
    LayoutDef("Comfortable", "Large, airy, rounded corners", 16, 68, 108),
)

/** สีเน้นที่เลือกเองได้ทับสีของธีม */
val Accents = listOf(
    "Turquoise" to Color(0xFF5EEAD4),
    "Blue" to Color(0xFF7AA2FF),
    "Rose" to Color(0xFFFF7AA8),
    "Amber" to Color(0xFFFBBF24),
    "Forest" to Color(0xFF86EFAC),
)

@Immutable
class Look(val accent: Color, val theme: ThemeDef, val layout: LayoutDef, val tint: Boolean) {
    val light get() = theme.base.luminance() > 0.4f
    val text get() = if (light) Color(0xFF14171C) else Color(0xFFF2F4F7)
    val dim get() = if (light) Color(0xFF5B6472) else Color(0xFF9AA3AF)
}

val LocalLook = compositionLocalOf { Look(Themes[0].accent, Themes[0], Layouts[0], true) }

@Composable
fun MySoundTheme(look: Look, content: @Composable () -> Unit) {
    val t = look.theme
    val onAccent = if (look.accent.luminance() > 0.35f) Color(0xFF04201C) else Color.White
    val hover = look.text.copy(alpha = 0.14f).compositeOver(t.raised)
    val line = look.text.copy(alpha = 0.13f)
    val scheme = if (look.light) {
        lightColorScheme(
            primary = look.accent, onPrimary = onAccent, background = t.base, onBackground = look.text,
            surface = t.base, onSurface = look.text, surfaceVariant = t.raised, onSurfaceVariant = look.dim,
            surfaceContainer = t.raised, surfaceContainerHigh = t.raised, secondaryContainer = hover,
            onSecondaryContainer = look.text, outlineVariant = line, outline = line,
        )
    } else {
        darkColorScheme(
            primary = look.accent, onPrimary = onAccent, background = t.base, onBackground = look.text,
            surface = t.base, onSurface = look.text, surfaceVariant = t.raised, onSurfaceVariant = look.dim,
            surfaceContainer = t.raised, surfaceContainerHigh = t.raised, secondaryContainer = hover,
            onSecondaryContainer = look.text, outlineVariant = line, outline = line,
        )
    }
    val d = LocalDensity.current
    CompositionLocalProvider(
        LocalLook provides look,
        LocalDensity provides Density(d.density, d.fontScale * look.layout.font / 100f),
    ) {
        MaterialTheme(colorScheme = scheme, content = content)
    }
}
