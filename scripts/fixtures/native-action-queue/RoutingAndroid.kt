package android.os
import java.util.concurrent.LinkedBlockingQueue
object MainPosts {
    val queue = LinkedBlockingQueue<Runnable>()
    @Volatile var accept = true
}
class Looper private constructor() {
    companion object {
        private val main = Looper()
        private val mainThread = Thread.currentThread()
        fun getMainLooper() = main
        fun myLooper(): Looper? = if (Thread.currentThread() === mainThread) main else null
    }
}
class Handler(looper: Looper) {
    fun post(task: Runnable): Boolean = MainPosts.accept && MainPosts.queue.offer(task)
}
