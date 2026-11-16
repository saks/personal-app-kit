//! The application `Context` `runSync` was given.
//!
//! Tao fills `ndk-context` only from the Activity. A WorkManager process has
//! none. `runSync` is given the application `Context` and a `JNIEnv`; this
//! module keeps a global ref of that `Context` for a later `enqueue`.

use std::sync::OnceLock;

use jni::Env;
use jni::objects::JObject;
use jni::sys::jobject;

/// Process-lifetime JNI global ref of the application `Context`.
struct GlobalContext(jobject);

// SAFETY: a JNI global ref is valid from any thread for the process lifetime.
// Nothing deletes this one.
unsafe impl Send for GlobalContext {}
// SAFETY: same as `Send`. Sharing the pointer does not mutate the ref.
unsafe impl Sync for GlobalContext {}

static APPLICATION: OnceLock<GlobalContext> = OnceLock::new();

/// Keep a process-lifetime global ref of `context`. A later `enqueue` uses
/// it instead of tao's `ndk-context` global, which is empty in a cold
/// WorkManager process. A null `context` is an error. A second call is a
/// no-op: the application does not change.
pub(crate) fn note(env: &Env, context: &JObject) -> jni::errors::Result<()> {
    if context.is_null() {
        return Err(jni::errors::Error::NullPtr("Context"));
    }
    if APPLICATION.get().is_some() {
        return Ok(());
    }
    let raw = env.new_global_ref(context)?.into_raw();
    if raw.is_null() {
        return Err(jni::errors::Error::NullPtr("Context"));
    }
    keep(raw);
    Ok(())
}

/// Remember a global ref the caller already keeps alive (tao's, or the one
/// [`note`] just created). A second call is a no-op. This module does not
/// delete the ref. A raced second `NewGlobalRef` lives until process exit.
pub(crate) fn keep(context: jobject) {
    if context.is_null() {
        return;
    }
    let _ = APPLICATION.set(GlobalContext(context));
}

/// The application `Context` [`note`] or [`keep`] stored, if any.
pub(crate) fn application_context() -> Option<jobject> {
    APPLICATION.get().map(|context| context.0)
}
