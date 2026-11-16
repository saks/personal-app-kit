package com.saksmlz.kit.android

import android.content.Context
import android.util.Log
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Guaranteed background sync, shared by both apps (sickness_tracker#96).
 * The Rust host enqueues this after each save and on pause
 * (`tauri_plugin_kit_android::enqueue` -> [enqueue]). It runs the app's Rust
 * Sync through JNI, possibly in a cold process without the UI or a Tauri
 * runtime: it loads the app's native library (named in the input data) and
 * calls the job the app registered with `background_sync!`.
 */
class SyncWorker(context: Context, params: WorkerParameters) :
  CoroutineWorker(context, params) {
  override suspend fun doWork(): Result {
    val tag = inputData.getString(KEY_LOG_TAG) ?: FALLBACK_TAG
    val library = inputData.getString(KEY_LIBRARY)
    if (library == null) {
      // Enqueued without the app's library name: retrying can't fix that.
      Log.w(tag, "sync: background job has no native library name")
      return Result.failure()
    }
    // A death in `runSync` leaves a count. After five, fail before loading
    // the library. A caught panic is retried once. Offline RETRY is not counted.
    val budgetFile = AttemptBudget.file(applicationContext)
    val begun = AttemptBudget.begin(
      budgetFile,
      AttemptBudget.versionCode(applicationContext, tag),
      tag,
    )
    return when (begun) {
      AttemptBudget.Begin.Stop -> {
        Log.w(tag, "sync: giving up after a native crash")
        Result.failure()
      }
      is AttemptBudget.Begin.Run -> finishRun(tag, library, budgetFile, begun)
    }
  }

  private suspend fun finishRun(
    tag: String,
    library: String,
    budgetFile: File,
    begun: AttemptBudget.Begin.Run,
  ): Result {
    // runSync blocks (the app's run may block_on), so keep it off Default.
    val code = try {
      withContext(Dispatchers.IO) {
        try {
          System.loadLibrary(library)
          runSync(applicationContext.dataDir.absolutePath, applicationContext)
        } catch (e: UnsatisfiedLinkError) {
          // Permanent: a missing library or symbol (a forgotten
          // `background_sync!` shows up here as a dlopen error) stays missing
          // until a fixed build is installed, so retrying can't fix it.
          Log.e(tag, "sync: background job could not load the native library; failing", e)
          AttemptBudget.FAILURE
        }
      }
    } catch (e: CancellationException) {
      // REPLACE cancelled this attempt. Put the crash count back.
      AttemptBudget.rollback(budgetFile, begun, tag)
      throw e
    }
    return when (val outcome = AttemptBudget.finish(budgetFile, begun, code, tag)) {
      AttemptBudget.Outcome.Success -> Result.success()
      AttemptBudget.Outcome.Retry -> Result.retry()
      AttemptBudget.Outcome.Failure -> Result.failure()
      AttemptBudget.Outcome.GaveUpOnPanic -> {
        Log.w(tag, "sync: giving up after a native panic")
        Result.failure()
      }
      is AttemptBudget.Outcome.Unknown -> {
        Log.e(tag, "sync: unknown result code ${outcome.code}")
        Result.failure()
      }
    }
  }

  companion object {
    /** Same unique name the apps used, so REPLACE also replaces their old job. */
    private const val WORK_NAME = "sync"
    private const val KEY_LIBRARY = "kit.android.library"
    private const val KEY_LOG_TAG = "kit.android.log_tag"
    /** Only if the input data has no tag (never from `enqueue`). */
    private const val FALLBACK_TAG = "SyncWorker"

    /** `dataDir` is Tauri's `app_data_dir`. `context` is the application. */
    @JvmStatic
    private external fun runSync(dataDir: String, context: Context): Int

    /** Called from Rust. One queued job at most: a new one replaces it. */
    @JvmStatic
    fun enqueue(
      context: Context,
      library: String,
      logTag: String,
      initialDelaySeconds: Long,
      backoffSeconds: Long,
    ) {
      // A new save replaces the job and starts a fresh crash budget.
      AttemptBudget.clear(context)
      val request = OneTimeWorkRequestBuilder<SyncWorker>()
        .setInputData(workDataOf(KEY_LIBRARY to library, KEY_LOG_TAG to logTag))
        .setConstraints(
          Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()
        )
        // Let the in-app Sync go first; if it finished, the job finds
        // nothing pending and makes no network call.
        .setInitialDelay(initialDelaySeconds, TimeUnit.SECONDS)
        .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, backoffSeconds, TimeUnit.SECONDS)
        .build()
      WorkManager.getInstance(context.applicationContext)
        .enqueueUniqueWork(WORK_NAME, ExistingWorkPolicy.REPLACE, request)
    }
  }
}
