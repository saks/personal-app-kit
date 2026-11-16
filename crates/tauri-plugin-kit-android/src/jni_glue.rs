//! JNI between `SyncWorker.kt` and Rust. Compiled for Android and for host
//! tests (no Java VM there, so only the types are checked).

use std::fmt;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use jni::vm::JavaVM;
use jni::{EnvUnowned, Outcome, jni_sig, jni_str};
use tracing::{debug, warn};

use crate::config::BackgroundSync;
use crate::process;
use crate::registry::registered;
use crate::worker::{self, NoDataDir};

/// `SyncWorker`'s class, which fixes the `runSync` symbol below.
const WORKER_CLASS: &str = "com.saksmlz.kit.android.SyncWorker";

/// Why `enqueue` could not hand the job to WorkManager. Each app maps this
/// into its own IPC error.
///
/// `#[non_exhaustive]`: a new variant is not a breaking change, so an app's
/// `match` needs a catch-all arm (map it to a generic error).
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnqueueError {
    /// No application `Context`: `runSync` has not stashed one, and (on the
    /// host, or before the Activity) tao's `ndk-context` is not available.
    NoAndroidContext,
    /// A JNI or WorkManager call failed. The string is `jni::errors::Error`'s
    /// `Display`, so this crate's public types do not name `jni`.
    Jni(String),
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAndroidContext => f.write_str("android context is not initialized"),
            Self::Jni(err) => f.write_str(err),
        }
    }
}

impl std::error::Error for EnqueueError {}

/// Store a JNI failure as text. Not `From`: a public impl would export the
/// private `jni` dependency.
fn jni_error(err: jni::errors::Error) -> EnqueueError {
    EnqueueError::Jni(err.to_string())
}

/// `SyncWorker.runSync(dataDir, context)`, on WorkManager's background thread
/// (not a tokio thread, so the app's run may `block_on`). `context` is the
/// application `Context`, kept for a later `enqueue` in this process.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_saksmlz_kit_android_SyncWorker_runSync<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    data_dir: JString<'local>,
    context: JObject<'local>,
) -> jint {
    // `EnvUnowned` is the FFI-safe pointer. `try_to_string` needs an `Env`.
    // Noting the context here also sets this crate's `JavaVM` singleton
    // (`AttachGuard`), before `init_logs` or the app's run. `with_env`
    // catches a panic in this closure. `with_env_no_catch` never builds
    // `Outcome::Panic`, so that arm would be dead and a panic would abort
    // out of this `extern "system"` function.
    let data_dir = data_dir_from(
        unowned_env
            .with_env(|env| {
                if let Err(err) = process::note(env, &context) {
                    warn!(error = %err, "sync: could not keep the application context");
                }
                data_dir.try_to_string(env)
            })
            .into_outcome(),
    );
    // `registered` runs inside the worker's `catch_unwind`.
    worker::run(registered, data_dir).code()
}

/// The data dir `runSync` read, or why that read did not yield one.
fn data_dir_from(outcome: Outcome<String, jni::errors::Error>) -> Result<String, NoDataDir> {
    match outcome {
        Outcome::Ok(dir) => Ok(dir),
        Outcome::Err(err) => Err(NoDataDir::Unreadable(err.to_string())),
        Outcome::Panic(_) => Err(NoDataDir::Panicked),
    }
}

/// `SyncWorker.enqueue(..)`: one network-constrained job, replacing any
/// queued one. Call it from the running app (after a save, on pause) or from
/// the worker's `run` (a follow-up). The worker process has no Tauri
/// Activity: `runSync` keeps the application `Context` `SyncWorker` passed
/// in. The in-app call still needs the Activity to have started (tao's
/// `ndk-context`). A missing context is `Err(EnqueueError::NoAndroidContext)`.
/// A JNI or WorkManager failure is `Err(EnqueueError::Jni(_))`. Both are
/// logged. Neither panics.
pub fn enqueue() -> Result<(), EnqueueError> {
    let result = try_enqueue(&registered());
    match &result {
        Ok(()) => debug!("sync: background job enqueued"),
        Err(err) => warn!(error = %err, "sync: could not enqueue the background job"),
    }
    result
}

fn try_enqueue(job: &BackgroundSync) -> Result<(), EnqueueError> {
    let (vm, context_ptr) = vm_and_context()?;
    // Milliseconds. `Schedule::new` already checked WorkManager's limits.
    let [initial_delay_ms, backoff_ms] = job.schedule.millis();
    // Attaches for the thread's lifetime and pushes a local frame. A pending
    // exception becomes `CaughtJavaException`, which `enqueue` logs.
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        // SAFETY: a process-lifetime global application Context (stashed in
        // `runSync`, or kept by tao). The wrapper only borrows it; nothing
        // here deletes the reference.
        let context = unsafe { JObject::from_raw(env, context_ptr) };
        // FindClass on a non-main thread misses the app loader. Load
        // `SyncWorker` through the application context's loader instead.
        let loader = env
            .call_method(
                &context,
                jni_str!("getClassLoader"),
                jni_sig!("()Ljava/lang/ClassLoader;"),
                &[],
            )?
            .l()?;
        let name = env.new_string(WORKER_CLASS)?;
        let class = env
            .call_method(
                &loader,
                jni_str!("loadClass"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                &[JValue::Object(&name)],
            )?
            .l()?;
        // SAFETY: `ClassLoader.loadClass` returns `java.lang.Class` or throws.
        let class = unsafe { JClass::from_raw(env, class.into_raw()) };
        let library = env.new_string(job.lib_name.as_str())?;
        let log_tag = env.new_string(job.log_tag.as_str())?;
        env.call_static_method(
            &class,
            jni_str!("enqueue"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;JJ)V"),
            &[
                JValue::Object(&context),
                JValue::Object(&library),
                JValue::Object(&log_tag),
                JValue::Long(initial_delay_ms),
                JValue::Long(backoff_ms),
            ],
        )?;
        Ok(())
    })
    .map_err(jni_error)
}

/// The stashed application `Context` and this crate's `JavaVM`, or tao's
/// `ndk-context` when the Activity has started and `runSync` has not.
fn vm_and_context() -> Result<(JavaVM, jni::sys::jobject), EnqueueError> {
    if let Some(context) = process::application_context() {
        let vm = JavaVM::singleton().map_err(|_| EnqueueError::NoAndroidContext)?;
        return Ok((vm, context));
    }
    // No stashed context. If this crate has already seen a JavaVM, `runSync`
    // attached and `note` failed. tao's global is unset there, and
    // `android_context` panics. The in-app path has no singleton yet (tao
    // uses another copy of `jni`), so it falls through.
    if JavaVM::singleton().is_ok() {
        return Err(EnqueueError::NoAndroidContext);
    }
    let (vm, context) = vm_and_context_from_activity()?;
    // `from_raw` has now set the singleton. Remember tao's global so the
    // next call takes the stash branch instead of the one above.
    process::keep(context);
    Ok((vm, context))
}

#[cfg(target_os = "android")]
fn vm_and_context_from_activity() -> Result<(JavaVM, jni::sys::jobject), EnqueueError> {
    let ctx = ndk_context::android_context();
    let vm_ptr: *mut jni::sys::JavaVM = ctx.vm().cast();
    if vm_ptr.is_null() {
        return Err(jni_error(jni::errors::Error::NullPtr("JavaVM")));
    }
    // SAFETY: non-null process JavaVM, valid for the process lifetime.
    let vm = unsafe { JavaVM::from_raw(vm_ptr) };
    let context_ptr: jni::sys::jobject = ctx.context().cast();
    if context_ptr.is_null() {
        return Err(jni_error(jni::errors::Error::NullPtr("Context")));
    }
    Ok((vm, context_ptr))
}

/// Host tests have no Activity and no `runSync`. Calling `android_context`
/// would panic; the missing context is an error.
#[cfg(not(target_os = "android"))]
fn vm_and_context_from_activity() -> Result<(JavaVM, jni::sys::jobject), EnqueueError> {
    Err(EnqueueError::NoAndroidContext)
}

#[cfg(test)]
mod tests;
