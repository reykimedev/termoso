plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val rustAbis: List<String> = (findProperty("labterm.abis") as String)
    .split(',')
    .map { it.trim() }
    .filter { it.isNotEmpty() }

// One version for the whole project: the workspace Cargo.toml is the source of truth
// (the desktop release workflow enforces the same for package.json).
val workspaceVersion: String = rootDir.parentFile.parentFile.resolve("Cargo.toml").useLines { lines ->
    lines.firstNotNullOfOrNull { Regex("""^version\s*=\s*"([^"]+)"""").find(it)?.groupValues?.get(1) }
} ?: error("workspace Cargo.toml has no version")

/** `MAJOR.MINOR.PATCH[-pre]` → `MMmmpp`; pre-release suffixes share the code of the final version. */
fun versionCodeOf(v: String): Int {
    val (major, minor, patch) = v.substringBefore('-').split('.').map { it.toInt() }
    require(minor < 100 && patch < 100) { "version $v does not fit MMmmpp" }
    return major * 10_000 + minor * 100 + patch
}

// Release signing comes from the environment only (CI secrets or a developer shell);
// nothing is read from the tree, so a checkout never contains key material.
val releaseKeystore: File? = System.getenv("LABTERM_ANDROID_KEYSTORE")?.let(::File)?.takeIf { it.isFile }

// Debug channel (fork CI): a fixed debug key lets each new build install over the last one,
// and the build number tells the in-app updater which build is newer. Unset = stock debug.
val debugKeystore: File? = System.getenv("LABTERM_ANDROID_DEBUG_KEYSTORE")?.let(::File)?.takeIf { it.isFile }
val debugBuildNumber: Int = System.getenv("LABTERM_DEBUG_BUILD")?.toIntOrNull() ?: 0
// latest.json of the debug channel; empty turns the in-app updater off.
val debugUpdateUrl: String = (findProperty("labterm.debugUpdateUrl") as String?).orEmpty()

// `-Plabterm.splits=true` produces one APK per ABI plus a universal one (release workflow).
val abiSplits: Boolean = (findProperty("labterm.splits") as String?)?.toBoolean() ?: false

// Host whose https://<host>/invite/… and /join/… links open in the app (Android App Links).
// Must match the server publishing this build's signing certificate in /.well-known/assetlinks.json.
val appLinkHost: String = findProperty("labterm.appLinkHost") as String

android {
    namespace = "dev.labdex.labterm"
    compileSdk = 36
    // Without the NDK AGP expects, it can't strip the packaged .so files (ours, JNA, ML Kit,
    // CameraX, proot) and ships them as they are. Pin the one CI installs.
    ndkVersion = findProperty("labterm.ndkVersion") as String

    defaultConfig {
        applicationId = "dev.labdex.labterm"
        minSdk = 26
        targetSdk = 36
        versionCode = versionCodeOf(workspaceVersion)
        versionName = workspaceVersion
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables.useSupportLibrary = true
        // Only ship the ABIs we build Rust for (JNA's AAR carries mips/armeabi too).
        ndk.abiFilters += rustAbis
        manifestPlaceholders["appLinkHost"] = appLinkHost
        buildConfigField("int", "DEBUG_BUILD", "0")
        buildConfigField("String", "DEBUG_UPDATE_URL", "\"\"")
    }

    signingConfigs {
        if (releaseKeystore != null) {
            create("release") {
                storeFile = releaseKeystore
                storePassword = System.getenv("LABTERM_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("LABTERM_ANDROID_KEY_ALIAS") ?: "termoso"
                keyPassword = System.getenv("LABTERM_ANDROID_KEY_PASSWORD") ?: System.getenv("LABTERM_ANDROID_KEYSTORE_PASSWORD")
                enableV2Signing = true
                enableV3Signing = true
            }
        }
        if (debugKeystore != null) {
            create("debugChannel") {
                storeFile = debugKeystore
                storePassword = System.getenv("LABTERM_ANDROID_DEBUG_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("LABTERM_ANDROID_DEBUG_KEY_ALIAS") ?: "termoso-debug"
                keyPassword = System.getenv("LABTERM_ANDROID_DEBUG_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Without LABTERM_ANDROID_KEYSTORE the APK is left unsigned (app-release-unsigned.apk)
            // rather than silently signed with the debug key.
            signingConfig = signingConfigs.findByName("release")
        }
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
            signingConfigs.findByName("debugChannel")?.let { signingConfig = it }
            buildConfigField("int", "DEBUG_BUILD", "$debugBuildNumber")
            buildConfigField("String", "DEBUG_UPDATE_URL", "\"$debugUpdateUrl\"")
        }
    }

    if (abiSplits) {
        splits.abi {
            isEnable = true
            reset()
            include(*rustAbis.toTypedArray())
            isUniversalApk = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlin {
        jvmToolchain(17)
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    packaging {
        resources.excludes += setOf("META-INF/AL2.0", "META-INF/LGPL2.1")
        // proot is exec'd from the native-library dir, so the .so files must be extracted to disk.
        jniLibs.useLegacyPackaging = true
    }

    lint {
        abortOnError = true
        warningsAsErrors = false
        disable += setOf("OldTargetApi")
    }

    // Reproducible, dependency-free: no Google Play services, no analytics.
    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }
}

dependencies {
    implementation(project(":core"))

    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.biometric)
    implementation(libs.androidx.browser)

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.androidsvg)
    // QR import: on-device scanning only (bundled model, no network).
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.mlkit.barcode.scanning)
    debugImplementation(libs.compose.ui.tooling)
    debugImplementation(libs.compose.ui.test.manifest)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
}
