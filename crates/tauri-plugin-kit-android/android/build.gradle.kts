// The app's tauri-build includes this project (`:tauri-plugin-kit-android`)
// through DEP_TAURI_PLUGIN_KIT_ANDROID_ANDROID_LIBRARY_PATH (see ../build.rs).
// Plugin versions come from the app's root buildscript.
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.saksmlz.kit.android"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
        // SyncWorker is reached through JNI and WorkManager reflection.
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }

    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    implementation("androidx.work:work-runtime-ktx:2.10.0")
}
