# tauri-plugin-kit-android: device test steps

For the adoption PRs (saks/sickness_tracker#100, saks/octo-budget-rs#151). These need an Android phone or emulator. The steps first appeared in the PR #8 description; this copy is the one to keep up to date.

1. **Build.** In one app, on a branch: add the dependency (pinned by `rev`), register the job with `background_sync!` (set `init_logs` to the app's logger), switch the `enqueue` call sites, and delete the app's own `SyncWorker.kt`, its Rust JNI function and its ProGuard rule. Then run `cargo tauri android build --target aarch64 --apk` (release, R8 on) and check that the Gradle build includes `:tauri-plugin-kit-android`.
2. **Exported symbol (on the host, not on the device).** `unzip` and `nm` run on your machine against the built APK. Don't use `adb shell`:

   ```bash
   APK=path/to/app-release.apk   # where `cargo tauri android build` put it
   unzip -l "$APK" | grep 'lib/arm64-v8a/lib<app>.so'
   unzip -p "$APK" lib/arm64-v8a/lib<app>.so > /tmp/lib<app>.so
   # llvm-nm from the NDK (or any nm that reads aarch64 ELF)
   nm -D /tmp/lib<app>.so | grep -E 'Java_com_saksmlz_kit_android_SyncWorker_runSync|__tauri_plugin_kit_android_background_sync'
   ```

   Check that `Java_com_saksmlz_kit_android_SyncWorker_runSync` is exported (`T`). Also check that `__tauri_plugin_kit_android_background_sync` is defined, not `U`. A `U` means `background_sync!` is missing, and the library will fail to load (see `src/registry.rs` in the crate).
3. **Warm path.** With sync configured, save a record. The app's call to `enqueue` returns `Ok(())`. Confirm the job with `adb shell dumpsys jobscheduler | grep -A5 <applicationId>` (or the WorkManager inspector): one job, with the network constraint and about a 10 s delay. The kit logs `sync: background job enqueued` at `debug`, so the default logcat level does not show it.
4. **Cold path.** Save while offline, swipe the app away (`adb shell am kill <applicationId>`), then turn the network on. The worker should run without the UI. Logcat should show the app's own log lines from `run` in that cold process (the worker called `init_logs` first, so those lines are actually logged), then a successful sync. Open the app and confirm nothing is pending.
5. **Retry.** Make the network flaky or the server unreachable. Confirm `Result.retry()` with a backoff starting near 30 s (logcat or `dumpsys`). A caught native panic is not this retry: it stops after one retry. A job that dies in the native call stops after repeated deaths.
6. **NeedsUser.** Use a bad token. Confirm the job ends as a failure, with no endless retries.
7. **Upgrade.** Install over the previous release while a job is queued under the old class, then save once. The old job should be replaced, and logcat should show no `ClassNotFoundException` for `com.saksmlz.<app>.SyncWorker`.
