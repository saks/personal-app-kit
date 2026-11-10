//! JNI between `SyncWorker.kt` and Rust. Compiled for Android and for host
//! tests (no Java VM there, so only the types are checked).

use jni::errors::Result;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use jni::vm::JavaVM;
use jni::{EnvUnowned, Outcome, jni_sig, jni_str};
use tracing::{info, warn};

use crate::config::{BackgroundSync, Schedule};
use crate::registry::registered;
use crate::worker::{self, NoDataDir};

/// `SyncWorker`'s class, which fixes the `runSync` symbol below.
const WORKER_CLASS: &str = "com.saksmlz.kit.android.SyncWorker";

/// `SyncWorker.runSync(dataDir)`, on WorkManager's background thread (not a
/// tokio thread, so the app's run may `block_on`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_saksmlz_kit_android_SyncWorker_runSync<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    data_dir: JString<'local>,
) -> jint {
    // `EnvUnowned` is the FFI-safe pointer. `try_to_string` needs an `Env`.
    let data_dir = match unowned_env
        .with_env_no_catch(|env| data_dir.try_to_string(env))
        .into_outcome()
    {
        Outcome::Ok(dir) => Ok(dir),
        Outcome::Err(err) => Err(NoDataDir::Unreadable(err.to_string())),
        Outcome::Panic(_) => Err(NoDataDir::Panicked),
    };
    // `registered` runs inside the worker's `catch_unwind`.
    worker::run(registered, data_dir).code()
}

/// `SyncWorker.enqueue(..)`: one network-constrained job, replacing any
/// queued one. Call it from the running app (after a save, on pause).
pub fn enqueue() {
    match try_enqueue(&registered()) {
        Ok(()) => info!("sync: background job enqueued"),
        Err(err) => warn!(error = %err, "sync: could not enqueue the background job"),
    }
}

/// Whole seconds for `SyncWorker.enqueue`'s `Long`s.
fn seconds(schedule: Schedule) -> [i64; 2] {
    [schedule.initial_delay, schedule.backoff]
        .map(|wait| i64::try_from(wait.as_secs()).unwrap_or(i64::MAX))
}

fn try_enqueue(job: &BackgroundSync) -> Result<()> {
    let ctx = ndk_context::android_context();
    let vm_ptr: *mut jni::sys::JavaVM = ctx.vm().cast();
    if vm_ptr.is_null() {
        return Err(jni::errors::Error::NullPtr("JavaVM"));
    }
    // SAFETY: non-null process JavaVM, valid for the process lifetime.
    let vm = unsafe { JavaVM::from_raw(vm_ptr) };
    let context_ptr: jni::sys::jobject = ctx.context().cast();
    if context_ptr.is_null() {
        return Err(jni::errors::Error::NullPtr("Context"));
    }
    let [initial_delay_s, backoff_s] = seconds(job.schedule);
    // Attaches for the thread's lifetime and pushes a local frame. A pending
    // exception becomes `CaughtJavaException`, which `enqueue` logs.
    vm.attach_current_thread(|env| {
        // SAFETY: tao keeps this global application Context for the process.
        // The wrapper only borrows it; nothing here deletes the reference.
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
                JValue::Long(initial_delay_s),
                JValue::Long(backoff_s),
            ],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests;
