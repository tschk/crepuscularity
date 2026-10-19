package android.os
import java.util.concurrent.LinkedBlockingQueue
object MainPosts {
    val queue = LinkedBlockingQueue<Runnable>()
    @Volatile var accept = true
}
class Looper { companion object { fun getMainLooper() = Looper() } }
class Handler(looper: Looper) { fun post(task: Runnable): Boolean = MainPosts.accept && MainPosts.queue.offer(task) }
