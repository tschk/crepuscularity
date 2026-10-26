import android.content.ContentResolver
import android.content.Context
import android.content.Cursor
import android.net.Uri
import androidx.activity.ComponentActivity
import java.io.ByteArrayInputStream
import java.io.File
import java.io.InputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import org.json.JSONArray
import org.json.JSONObject

private class Provider(val main: Thread, val prefix: String): ContentResolver() {
 val entered = CountDownLatch(1)
 val release = CountDownLatch(1)
 val gated = AtomicBoolean(true)
 private fun worker() { check(Thread.currentThread() !== main); check(Thread.currentThread().name == "crepus-picker-io") }
 override fun getType(uri: Uri): String? {
  worker()
  if(uri.id == "denied") throw SecurityException("permission denied")
  return "application/octet-stream"
 }
 override fun query(uri: Uri, columns: Array<String>, selection: String?, args: Array<String>?, order: String?): Cursor? {
  worker(); return Cursor(uri.id + ".bin")
 }
 override fun openInputStream(uri: Uri): InputStream? {
  worker()
  if(uri.id == "gated" && gated.compareAndSet(true, false)) {
   entered.countDown(); check(release.await(5, TimeUnit.SECONDS))
  }
  if(uri.id == "fail") return object: InputStream() {
   var calls = 0
   override fun read(): Int = throw java.io.IOException("failed provider")
   override fun read(bytes: ByteArray, offset: Int, length: Int): Int {
    if(calls++ == 0) {bytes[offset] = 1; return 1}
    throw java.io.IOException("failed after partial bytes")
   }
  }
  return ByteArrayInputStream((prefix + uri.id).toByteArray())
 }
}

fun main() {
 val main = Thread.currentThread()
 android.os.Looper.getMainLooper()
 val root = kotlin.io.path.createTempDirectory("picker-provider-").toFile()
 val results = mutableListOf<JSONObject>()
 fun sink(raw: String) {check(Thread.currentThread() === main); results.add(JSONObject.lookup(raw))}
 fun pump(count: Int) {
  while(results.size < count) {
   val task = android.os.MainPosts.queue.poll(5, TimeUnit.SECONDS) ?: error("missing main delivery $count")
   task.run()
  }
 }
 fun pick(activity: ComponentActivity, vararg names: String) {
  check(PickerUnderTest.request("import_files") != null)
  activity.deliver(names.map(::Uri))
 }
 fun files(result: JSONObject): List<JSONObject> =
  (((result.fields["value"] as JSONObject).fields["files"] as JSONArray).items).map { it as JSONObject }
 try {
  val aDir = File(root,"a").apply {mkdirs()}
  val aProvider = Provider(main,"A:")
  val a = ComponentActivity(Context(aProvider,aDir))
  PickerUnderTest.install(a); CrepusActions.resultSink = ::sink
  pick(a,"gated")
  check(aProvider.entered.await(5,TimeUnit.SECONDS))
  // Callback returned while provider is held: main can run an independent heartbeat.
  var heartbeat = false
  android.os.Handler(android.os.Looper.getMainLooper()).post(Runnable {heartbeat=true})
  android.os.MainPosts.queue.poll(5,TimeUnit.SECONDS)!!.run()
  check(heartbeat && results.isEmpty())
  CrepusActions.resultSink = {error("request did not capture its sink")}
  aProvider.release.countDown(); pump(1)
  CrepusActions.resultSink = ::sink
  val first=results.single()
  check(first.fields["ok"] == true && first.fields["action"] == "import_files")
  val imported = files(first).single()
  check(imported.fields["name"] == "gated.bin")
  check(File(imported.fields["filePath"] as String).readText() == "A:gated")
  pick(a,"fail"); pump(2)
  check(results.last().fields["ok"] == false && results.last().fields["failedCount"] == 1)
  check(aDir.listFiles()!!.size == 1) // Partial destination was deleted.
  pick(a,"denied"); pump(3)
  check(results.last().fields["ok"] == false && results.last().fields["failedCount"] == 1)

  // A stale in-flight request must not read a replacement context or publish.
  val staleDir = File(root,"stale").apply {mkdirs()}
  val staleProvider = Provider(main,"STALE:")
  val stale = ComponentActivity(Context(staleProvider,staleDir))
  PickerUnderTest.install(stale); CrepusActions.resultSink = ::sink
  pick(stale,"gated"); check(staleProvider.entered.await(5,TimeUnit.SECONDS))
  check(PickerUnderTest.detach(stale))
  val bDir = File(root,"b").apply {mkdirs()}
  val bProvider = Provider(main,"B:")
  val b = ComponentActivity(Context(bProvider,bDir))
  PickerUnderTest.install(b); CrepusActions.resultSink = ::sink
  check(!PickerUnderTest.detach(stale)) // Old Activity must not detach new install.
  check(PickerUnderTest.request("import_files") != null)
  stale.deliver(listOf(Uri("obsolete"))) // Does not consume new pending request.
  b.deliver(listOf(Uri("fresh")))
  staleProvider.release.countDown(); pump(4)
  check(results.size == 4 && staleDir.listFiles()!!.isEmpty())
  check(File(files(results.last()).single().fields["filePath"] as String).readText() == "B:fresh")

  // Bound queue: one active copy plus four pending requests, then explicit failure.
  pick(b,"gated"); check(bProvider.entered.await(5,TimeUnit.SECONDS))
  repeat(5) { pick(b,"queued$it") }
  check(results.size == 5 && results.last().fields["error"] == "file import queue is full")
  bProvider.release.countDown(); pump(10)
  check(results.size == 10)
  check(results.drop(5).all {it.fields["ok"] == true})
  // FutureTask captures sink exceptions: one invocation, no fallback delivery,
  // and no promise that the exception is surfaced on Android's main thread.
  var throwingInvocations = 0
  CrepusActions.resultSink = {
   check(Thread.currentThread() === main)
   throwingInvocations += 1
   throw IllegalStateException("consumer sink failed")
  }
  pick(b,"throwing-sink")
  CrepusActions.resultSink = ::sink
  pick(b,"after-throw")
  pump(11)
  check(throwingInvocations == 1 && results.size == 11)
  check(File(files(results.last()).single().fields["filePath"] as String).readText() == "B:after-throw")
  check(android.os.MainPosts.queue.isEmpty())

  // Rejected main publication must not call get() on an unrun FutureTask.
  // A following accepted job proves worker progress and stale file cleanup.
  val beforeRejected = bDir.listFiles()!!.size
  android.os.MainPosts.accept = false
  pick(b,"rejected-post")
  check(android.os.MainPosts.rejected.await(5,TimeUnit.SECONDS))
  check(results.size == 11)
  android.os.MainPosts.accept = true
  pick(b,"after-rejection")
  pump(12)
  check(results.size == 12 && bDir.listFiles()!!.size == beforeRejected + 1)
  check(File(files(results.last()).single().fields["filePath"] as String).readText() == "B:after-rejection")
  check(PickerUnderTest.detach(b))
  println("PASS actual Android picker helpers: gated worker/main heartbeat/exact results/permission+partial errors/captured context/stale cleanup/identity detach/bounded queue/sinkthrow/rejected post")
 } finally {root.deleteRecursively()}
}
