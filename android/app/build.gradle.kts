plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "com.mysound.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.mysound.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 3
        versionName = "1.0.0"
        // ไลบรารี yt-dlp + ffmpeg ใหญ่ ตัดสถาปัตยกรรม x86 (เฉพาะอีมูเลเตอร์) ออกเพื่อให้ APK เล็กลง
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }

    // กุญแจเซ็น APK มาจาก environment (GitHub Actions secrets) ไม่ต้องเก็บในโค้ด
    signingConfigs {
        create("release") {
            val path = System.getenv("KEYSTORE_PATH")
            if (path != null) {
                storeFile = file(path)
                storePassword = System.getenv("KEYSTORE_PASSWORD")
                keyAlias = System.getenv("KEY_ALIAS")
                keyPassword = System.getenv("KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }
    lint { checkReleaseBuilds = false } // กัน lint ทำให้ build release ล้มเพราะคำเตือนที่ไม่เกี่ยวกับการทำงาน
    // yt-dlp/ffmpeg เป็นไฟล์ที่ต้องรันจากดิสก์ ต้องแตก .so ออกมา
    packaging { jniLibs { useLegacyPackaging = true } }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation(platform("androidx.compose:compose-bom:2024.12.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("androidx.media3:media3-exoplayer:1.5.1")
    implementation("androidx.media3:media3-session:1.5.1")

    implementation("io.github.junkfood02.youtubedl-android:library:0.17.4")
    implementation("io.github.junkfood02.youtubedl-android:ffmpeg:0.17.4")

    testImplementation("junit:junit:4.13.2")
}
