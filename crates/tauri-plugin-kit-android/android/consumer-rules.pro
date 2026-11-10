# SyncWorker (sickness_tracker#96): Rust calls enqueue(..) through JNI and
# implements runSync; WorkManager instantiates the class by name.
-keep class com.saksmlz.kit.android.SyncWorker {
   public <init>(android.content.Context, androidx.work.WorkerParameters);
   public static void enqueue(android.content.Context, java.lang.String, java.lang.String, long, long);
   native <methods>;
}
