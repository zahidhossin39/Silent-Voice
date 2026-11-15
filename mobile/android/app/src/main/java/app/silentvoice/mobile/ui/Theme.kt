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
 * Material 3 colour roles generated from the Silent Voice orange (#F97316) as
 * the seed, the way m3.material.io describes: the seed gives the primary tonal
 * palette, and every surface is a warm neutral from the same hue. Screens use
 * roles (primary, surfaceContainer, ...) and never raw colours.
 */
val Brand = Color(0xFFF97316)
/** Dark ink that passes contrast on the brand orange (white does not). */
val OnBrand = Color(0xFF2F1500)

private val Light = lightColorScheme(
    primary = Color(0xFF904D00),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFFFDCC3),
    onPrimaryContainer = Color(0xFF2F1500),
    secondary = Color(0xFF755845),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFFFFDCC7),
    onSecondaryContainer = Color(0xFF2B1708),
    tertiary = Color(0xFF5F6135),
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = Color(0xFFE5E6AE),
    onTertiaryContainer = Color(0xFF1C1D00),
    error = Color(0xFFBA1A1A),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
    background = Color(0xFFFFF8F5),
    onBackground = Color(0xFF221A14),
    surface = Color(0xFFFFF8F5),
    onSurface = Color(0xFF221A14),
    surfaceVariant = Color(0xFFF3DFD1),
    onSurfaceVariant = Color(0xFF51443A),
    outline = Color(0xFF847468),
    outlineVariant = Color(0xFFD6C3B6),
    inverseSurface = Color(0xFF382E28),
    inverseOnSurface = Color(0xFFFEEEE4),
    inversePrimary = Color(0xFFFFB77D),
    surfaceDim = Color(0xFFE7D7CD),
    surfaceBright = Color(0xFFFFF8F5),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFFFF1E8),
    surfaceContainer = Color(0xFFFCEBE0),
    surfaceContainerHigh = Color(0xFFF6E5DA),
    surfaceContainerHighest = Color(0xFFF0DFD5),
)

private val Dark = darkColorScheme(
    primary = Color(0xFFFFB77D),
    onPrimary = Color(0xFF4D2600),
    primaryContainer = Color(0xFF6D3900),
    onPrimaryContainer = Color(0xFFFFDCC3),
    secondary = Color(0xFFE5BFA8),
    onSecondary = Color(0xFF432B1B),
    secondaryContainer = Color(0xFF5B412F),
    onSecondaryContainer = Color(0xFFFFDCC7),
    tertiary = Color(0xFFC9CA94),
    onTertiary = Color(0xFF31320B),
    tertiaryContainer = Color(0xFF474920),
    onTertiaryContainer = Color(0xFFE5E6AE),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    background = Color(0xFF19120C),
    onBackground = Color(0xFFF0DFD5),
    surface = Color(0xFF19120C),
    onSurface = Color(0xFFF0DFD5),
    surfaceVariant = Color(0xFF51443A),
    onSurfaceVariant = Color(0xFFD6C3B6),
    outline = Color(0xFF9E8E81),
    outlineVariant = Color(0xFF51443A),
    inverseSurface = Color(0xFFF0DFD5),
    inverseOnSurface = Color(0xFF382E28),
    inversePrimary = Color(0xFF904D00),
    surfaceDim = Color(0xFF19120C),
    surfaceBright = Color(0xFF413730),
    surfaceContainerLowest = Color(0xFF140D08),
    surfaceContainerLow = Color(0xFF221A14),
    surfaceContainer = Color(0xFF261E18),
    surfaceContainerHigh = Color(0xFF312822),
    surfaceContainerHighest = Color(0xFF3C332C),
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
