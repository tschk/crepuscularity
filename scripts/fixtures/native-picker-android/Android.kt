package android.os
import java.util.concurrent.LinkedBlockingQueue
object MainPosts {
 val queue = LinkedBlockingQueue<Runnable>()
 @Volatile var accept = true
 val rejected = java.util.concurrent.CountDownLatch(1)
}
class Looper private constructor() {
 companion object {
  private val main = Looper()
  private val mainThread = Thread.currentThread()
  fun getMainLooper() = main
  fun myLooper(): Looper? = if(Thread.currentThread() === mainThread) main else null
 }
}
class Handler(looper: Looper) {
 fun post(task: Runnable): Boolean {
  if (!MainPosts.accept) { MainPosts.rejected.countDown(); return false }
  return MainPosts.queue.offer(task)
 }
}
