package com.mysound.android.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val Base = Color(0xFF0F1115)
private val Raised = Color(0xFF181B21)
private val Text = Color(0xFFF2F4F7)
private val Dim = Color(0xFF9AA3AF)

/** สีเน้นเหมือนชุดสีของแอป PC */
val Accents = listOf(
    "Turquoise" to Color(0xFF5EEAD4),
    "Blue" to Color(0xFF7AA2FF),
    "Rose" to Color(0xFFFF7AA8),
    "Amber" to Color(0xFFFBBF24),
    "Forest" to Color(0xFF86EFAC),
)

@Composable
fun MySoundTheme(accent: Color, content: @Composable () -> Unit) {
    val scheme = darkColorScheme(
        primary = accent,
        onPrimary = Color(0xFF04201C),
        background = Base,
        onBackground = Text,
        surface = Base,
        onSurface = Text,
        surfaceVariant = Raised,
        onSurfaceVariant = Dim,
        surfaceContainer = Raised,
        surfaceContainerHigh = Raised,
        secondaryContainer = Color(0xFF232830),
        onSecondaryContainer = Text,
        outlineVariant = Color(0xFF2A2F38),
    )
    MaterialTheme(colorScheme = scheme, content = content)
}
