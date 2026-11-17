package app.silentvoice.mobile.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.silentvoice.mobile.R

/**
 * Material 3 colour roles filled with the desktop app's tokens (src/styles.css):
 * cool near-black and slate surfaces, one orange accent, green for "done".
 * Screens use roles (primary, surfaceContainer, ...) and never raw colours,
 * except the pill, which is dark in both themes, like on Windows.
 */
val Brand = Color(0xFFF97316)
/** Dark ink that passes contrast on the brand orange (white does not). */
val OnBrand = Color(0xFF1A1200)

private val Light = lightColorScheme(
    // Desktop light accent is #EA580C; text-sized orange needs the deeper
    // #C2410C to reach 4.5:1 on white.
    primary = Color(0xFFC2410C),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFFFE6D5),
    onPrimaryContainer = Color(0xFF431407),
    secondary = Color(0xFF4B5468),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFE4E7EE),
    onSecondaryContainer = Color(0xFF1A1E28),
    tertiary = Color(0xFF15803D),
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFDCFCE7),
    onTertiaryContainer = Color(0xFF14532D),
    error = Color(0xFFB91C1C),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFEE2E2),
    onErrorContainer = Color(0xFF7F1D1D),
    background = Color(0xFFF5F6F8),
    onBackground = Color(0xFF1A1E28),
    surface = Color(0xFFF5F6F8),
    onSurface = Color(0xFF1A1E28),
    surfaceVariant = Color(0xFFECEEF2),
    onSurfaceVariant = Color(0xFF5C6473),
    outline = Color(0xFF8A92A3),
    outlineVariant = Color(0xFFDDE1E8),
    inverseSurface = Color(0xFF1B2030),
    inverseOnSurface = Color(0xFFE6E9F0),
    inversePrimary = Color(0xFFF97316),
    surfaceDim = Color(0xFFE4E7EC),
    surfaceBright = Color(0xFFFFFFFF),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFFFFFFF),
    surfaceContainer = Color(0xFFECEEF2),
    surfaceContainerHigh = Color(0xFFE6E9EE),
    surfaceContainerHighest = Color(0xFFDDE1E8),
)

private val Dark = darkColorScheme(
    primary = Brand,
    onPrimary = OnBrand,
    primaryContainer = Color(0xFF3B1D0A),
    onPrimaryContainer = Color(0xFFFFD7BC),
    secondary = Color(0xFFB4BBCB),
    onSecondary = Color(0xFF1A1E28),
    secondaryContainer = Color(0xFF262C3D),
    onSecondaryContainer = Color(0xFFE6E9F0),
    tertiary = Color(0xFF22C55E),
    onTertiary = Color(0xFF052E16),
    tertiaryContainer = Color(0xFF0F2A1A),
    onTertiaryContainer = Color(0xFF86EFAC),
    error = Color(0xFFEF4444),
    onError = Color(0xFF2B0505),
    errorContainer = Color(0xFF3A1214),
    onErrorContainer = Color(0xFFFCA5A5),
    background = Color(0xFF0B0E14),
    onBackground = Color(0xFFE6E9F0),
    surface = Color(0xFF0B0E14),
    onSurface = Color(0xFFE6E9F0),
    surfaceVariant = Color(0xFF1B2030),
    onSurfaceVariant = Color(0xFF8B93A7),
    outline = Color(0xFF4A5368),
    outlineVariant = Color(0xFF262C3D),
    inverseSurface = Color(0xFFE6E9F0),
    inverseOnSurface = Color(0xFF131722),
    inversePrimary = Color(0xFFC2410C),
    surfaceDim = Color(0xFF0B0E14),
    surfaceBright = Color(0xFF262C3D),
    surfaceContainerLowest = Color(0xFF07090D),
    surfaceContainerLow = Color(0xFF131722),
    surfaceContainer = Color(0xFF161B27),
    surfaceContainerHigh = Color(0xFF1B2030),
    surfaceContainerHighest = Color(0xFF232838),
)

@OptIn(androidx.compose.ui.text.ExperimentalTextApi::class)
private fun jakarta(weight: Int) = Font(
    R.font.plus_jakarta_sans,
    FontWeight(weight),
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

private val Jakarta = FontFamily(jakarta(400), jakarta(500), jakarta(600), jakarta(700), jakarta(800))

/** The M3 type scale, set in Plus Jakarta Sans, with tighter and heavier headlines. */
private val Type = Typography().run {
    fun TextStyle.j(weight: Int = 400, tracking: Float = 0f) =
        copy(fontFamily = Jakarta, fontWeight = FontWeight(weight), letterSpacing = tracking.em)
    Typography(
        displayLarge = displayLarge.j(800, -0.03f),
        displayMedium = displayMedium.j(800, -0.03f),
        displaySmall = displaySmall.j(800, -0.03f).copy(lineHeight = 42.sp),
        headlineLarge = headlineLarge.j(700, -0.02f),
        headlineMedium = headlineMedium.j(700, -0.02f),
        headlineSmall = headlineSmall.j(700, -0.01f),
        titleLarge = titleLarge.j(700, -0.01f),
        titleMedium = titleMedium.j(600),
        titleSmall = titleSmall.j(600),
        bodyLarge = bodyLarge.j(400).copy(lineHeight = 25.sp),
        bodyMedium = bodyMedium.j(400).copy(lineHeight = 21.sp),
        bodySmall = bodySmall.j(400),
        labelLarge = labelLarge.j(600),
        labelMedium = labelMedium.j(600),
        labelSmall = labelSmall.j(600, 0.04f),
    )
}

private val Round = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(32.dp),
)

@Composable
fun SilentVoiceTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) Dark else Light,
        typography = Type,
        shapes = Round,
        content = content,
    )
}
