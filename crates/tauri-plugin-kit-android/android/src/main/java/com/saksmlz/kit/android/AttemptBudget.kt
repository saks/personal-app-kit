package com.saksmlz.kit.android

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import java.io.File
import java.io.IOException

/**
 * How many times this install has died inside `runSync`, and how many caught
 * panics have come back in a row.
 *
 * WorkManager does not keep a retry's output, so the next attempt still sees
 * the original input. The counters are one line in `noBackupFilesDir`
 * (`versionCode unclean panics`). Missing or malformed means a fresh budget.
 * An app upgrade (`versionCode` changed) starts over too.
 *
 * `unclean` counts process deaths. [SyncWorker] stops before loading the
 * library once it reaches [MAX_UNCLEAN]. `panics` counts caught panics
 * ([PANIC]); the second one fails the job. Offline [RETRY] clears both.
 */
internal object AttemptBudget {
  /** Deaths in a row before the next `doWork` fails without loading the library. */
  const val MAX_UNCLEAN = 5

  /** Caught panics before the job fails. The first one retries. */
  const val MAX_PANICS = 2

  // tauri_plugin_kit_android::WorkerResult, plus PANIC which is not a variant.
  const val SUCCESS = 0
  const val RETRY = 1
  const val FAILURE = 2
  const val PANIC = 3

  private const val FILE_NAME = "kit-android-sync-attempt"

  fun file(context: Context): File =
    File(context.applicationContext.noBackupFilesDir, FILE_NAME)

  fun versionCode(context: Context, tag: String): Long {
    val app = context.applicationContext
    return try {
      packageVersionCode(app)
    } catch (e: PackageManager.NameNotFoundException) {
      Log.w(tag, "sync: could not read the app version", e)
      0L
    }
  }

  /** Record this attempt, or [Begin.Stop] when the crash budget is already spent. */
  fun begin(file: File, versionCode: Long, tag: String): Begin {
    val decision = decideBegin(read(file, versionCode), versionCode)
    when (decision) {
      is Begin.Run -> write(file, decision.inFlight, tag)
      Begin.Stop -> clear(file)
    }
    return decision
  }

  /** A clean return from `runSync`. Drops this attempt's crash bump. */
  fun finish(file: File, run: Begin.Run, code: Int, tag: String): Outcome {
    return when (val decision = decideAfter(run.inFlight, code)) {
      is After.Keep -> {
        write(file, decision.counters, tag)
        decision.outcome
      }
      is After.Clear -> {
        clear(file)
        decision.outcome
      }
    }
  }

  /** `REPLACE` cancelled this attempt. It was not a crash. */
  fun rollback(file: File, run: Begin.Run, tag: String) {
    write(file, run.rollback, tag)
  }

  fun clear(context: Context) {
    clear(file(context))
  }

  sealed class Begin {
    class Run(val inFlight: Counters, val rollback: Counters) : Begin()
    object Stop : Begin()
  }

  sealed class Outcome {
    object Success : Outcome()
    object Retry : Outcome()
    object Failure : Outcome()
    object GaveUpOnPanic : Outcome()
    class Unknown(val code: Int) : Outcome()
  }

  data class Counters(val versionCode: Long, val unclean: Int, val panics: Int)

  internal fun decideBegin(previous: Counters, versionCode: Long): Begin {
    val current = if (previous.versionCode == versionCode) {
      previous
    } else {
      Counters(versionCode, 0, 0)
    }
    if (current.unclean >= MAX_UNCLEAN) {
      return Begin.Stop
    }
    return Begin.Run(
      current.copy(unclean = current.unclean + 1),
      current,
    )
  }

  internal fun decideAfter(inFlight: Counters, code: Int): After = when (code) {
    SUCCESS -> After.Clear(Outcome.Success)
    RETRY -> After.Clear(Outcome.Retry)
    FAILURE -> After.Clear(Outcome.Failure)
    PANIC -> {
      val panics = inFlight.panics + 1
      if (panics >= MAX_PANICS) {
        After.Clear(Outcome.GaveUpOnPanic)
      } else {
        After.Keep(
          inFlight.copy(
            unclean = (inFlight.unclean - 1).coerceAtLeast(0),
            panics = panics,
          ),
          Outcome.Retry,
        )
      }
    }
    else -> After.Clear(Outcome.Unknown(code))
  }

  internal sealed class After {
    class Keep(val counters: Counters, val outcome: Outcome) : After()
    class Clear(val outcome: Outcome) : After()
  }

  private fun packageVersionCode(context: Context): Long {
    val pm = context.packageManager
    val name = context.packageName
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      pm.getPackageInfo(name, PackageManager.PackageInfoFlags.of(0)).longVersionCode
    } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      @Suppress("DEPRECATION")
      pm.getPackageInfo(name, 0).longVersionCode
    } else {
      @Suppress("DEPRECATION")
      pm.getPackageInfo(name, 0).versionCode.toLong()
    }
  }

  private fun read(file: File, versionCode: Long): Counters {
    val text = try {
      file.readText()
    } catch (_: IOException) {
      return Counters(versionCode, 0, 0)
    }
    val parts = text.trim().split(Regex("\\s+"))
    if (parts.size != 3) {
      return Counters(versionCode, 0, 0)
    }
    val version = parts[0].toLongOrNull()
    val unclean = parts[1].toIntOrNull()
    val panics = parts[2].toIntOrNull()
    if (version == null || unclean == null || panics == null || unclean < 0 || panics < 0) {
      return Counters(versionCode, 0, 0)
    }
    return Counters(version, unclean, panics)
  }

  private fun write(file: File, counters: Counters, tag: String) {
    val dir = file.parentFile
    if (dir == null || (!dir.exists() && !dir.mkdirs())) {
      Log.w(tag, "sync: could not record the background attempt")
      return
    }
    val tmp = File(dir, "$FILE_NAME.tmp")
    try {
      tmp.writeText("${counters.versionCode} ${counters.unclean} ${counters.panics}\n")
      // `renameTo` does not replace an existing file on Android.
      if (file.exists() && !file.delete()) {
        tmp.delete()
        Log.w(tag, "sync: could not record the background attempt")
        return
      }
      if (!tmp.renameTo(file)) {
        tmp.delete()
        Log.w(tag, "sync: could not record the background attempt")
      }
    } catch (e: IOException) {
      tmp.delete()
      Log.w(tag, "sync: could not record the background attempt", e)
    }
  }

  private fun clear(file: File) {
    file.delete()
    file.parentFile?.let { parent -> File(parent, "$FILE_NAME.tmp").delete() }
  }
}
