import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

private fun waitForWorker() {
    val field = CrepusActions::class.java.getDeclaredField("executor")
    field.isAccessible = true
    val executor = field.get(null) as java.util.concurrent.ExecutorService
    executor.submit {}.get(10, TimeUnit.SECONDS)
}

fun main() {
    val main = Thread.currentThread()
    android.os.Looper.getMainLooper()
    val hosts = AtomicInteger()
    val rust = AtomicInteger()
    val results = mutableListOf<String>()
    RoutingSlice.host = { action ->
        check(Thread.currentThread() === main)
        hosts.incrementAndGet()
        if (action == "import_files") "host-result" else null
    }
    RoutingSlice.rust = { action ->
        check(Thread.currentThread() !== main)
        check(Thread.currentThread().name == "crepus-actions")
        rust.incrementAndGet()
        "rust-result:$action"
    }
    RoutingSlice.install()
    CrepusActions.resultSink = { result ->
        check(Thread.currentThread() === main)
        results.add(result)
    }
    CrepusActions.perform("import_files")
    CrepusActions.perform("sync")
    while (results.size < 2) {
        val post = android.os.MainPosts.queue.poll(10, TimeUnit.SECONDS) ?: error("missing main post")
        post.run()
    }
    waitForWorker()
    check(results == listOf("host-result", "rust-result:sync"))
    check(hosts.get() == 2 && rust.get() == 1)
    // The installed dispatch takes the direct fast path when invoked on main.
    check(CrepusActions.dispatch("import_files") == "host-result")
    check(hosts.get() == 3 && android.os.MainPosts.queue.isEmpty())
    // A rejected post must fail promptly instead of waiting on an unrun FutureTask.
    android.os.MainPosts.accept = false
    val failed = java.util.concurrent.FutureTask<Boolean> {
        runCatching { CrepusActions.dispatch("import_files") }
            .exceptionOrNull()?.message == "main looper unavailable"
    }
    Thread(failed, "routing-rejection-test").apply { isDaemon = true }.start()
    check(failed.get(10, TimeUnit.SECONDS))
    check(hosts.get() == 3 && rust.get() == 1)
    println("PASS: actual installed dispatch + routing helper; host on main, Rust on serial worker, main fastpath, rejected Handler post fails promptly")
}
