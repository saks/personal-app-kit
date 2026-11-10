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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
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
    // runSync blocks (the app's run may block_on), so keep it off Default.
    val code = withContext(Dispatchers.IO) {
      try {
        System.loadLibrary(library)
        runSync(applicationContext.dataDir.absolutePath)
      } catch (e: UnsatisfiedLinkError) {
        Log.w(tag, "sync: background job could not load the native library", e)
        RETRY
      }
    }
    return when (code) {
      SUCCESS -> Result.success()
      FAILURE -> Result.failure()
      else -> Result.retry()
    }
  }

  companion object {
    /** Same unique name the apps used, so REPLACE also replaces their old job. */
    private const val WORK_NAME = "sync"
    private const val KEY_LIBRARY = "kit.android.library"
    private const val KEY_LOG_TAG = "kit.android.log_tag"
    /** Only if the input data has no tag (never from `enqueue`). */
    private const val FALLBACK_TAG = "SyncWorker"

    // tauri_plugin_kit_android::WorkerResult
    private const val SUCCESS = 0
    private const val RETRY = 1
    private const val FAILURE = 2

    /** `dataDir` is Tauri's `app_data_dir`. */
    @JvmStatic
    private external fun runSync(dataDir: String): Int

    /** Called from Rust. One queued job at most: a new one replaces it. */
    @JvmStatic
    fun enqueue(
      context: Context,
      library: String,
      logTag: String,
      initialDelaySeconds: Long,
      backoffSeconds: Long,
    ) {
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
