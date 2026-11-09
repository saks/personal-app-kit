//! Hands `android/` to the app's Gradle build, like `tauri-plugin`'s
//! `Builder::android_path` does, without pulling `tauri` into the kit: the
//! app's `tauri-build` turns `DEP_<links>_ANDROID_LIBRARY_PATH` into an
//! `include` + `implementation(project(..))`. The Kotlin library needs no
//! Tauri API.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=android");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        println!("cargo:warning=CARGO_MANIFEST_DIR is not set; the Android library is not linked");
        return;
    };
    let android = Path::new(&manifest_dir).join("android");
    println!("cargo:android_library_path={}", android.display());
}
