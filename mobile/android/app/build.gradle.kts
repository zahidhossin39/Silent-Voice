plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// One pin for desktop and phone (ADR 0002): the desktop sidecar's VERSION file.
val sherpaVersion = rootProject.file("../../src-tauri/sidecars/sherpa/VERSION").readText().trim()
val sherpaAar = file("libs/sherpa-onnx-$sherpaVersion.aar")
if (!sherpaAar.exists()) {
    throw GradleException("Missing ${sherpaAar.name}. Run mobile/android/scripts/fetch-sherpa.ps1 first.")
}

android {
    namespace = "app.silentvoice.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.silentvoice.mobile"
        minSdk = 29
        targetSdk = 35
        versionCode = 6
        versionName = "0.1.4"
        // arm64 = real phones, x86_64 = emulator. Leaving out 32-bit keeps the APK small.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildFeatures { compose = true }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    implementation(files(sherpaAar))
    implementation(platform("androidx.compose:compose-bom:2024.12.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material:material-icons-core")
    // UniFFI's Kotlin bindings call the Rust library (crates/sv-core) through JNA.
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // Model archives are .tar.bz2, which Android cannot unpack by itself.
    implementation("org.apache.commons:commons-compress:1.27.1")
    testImplementation("junit:junit:4.13.2")
}
