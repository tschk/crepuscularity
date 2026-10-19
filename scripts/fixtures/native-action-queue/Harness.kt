import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import java.util.Collections

private fun nextPost(): Runnable =
    android.os.MainPosts.queue.poll(10, TimeUnit.SECONDS) ?: error("missing callback")

private fun waitForWorker() {
    val field = CrepusActions::class.java.getDeclaredField("executor")
    field.isAccessible = true
    val executor = field.get(null) as java.util.concurrent.ExecutorService
    executor.submit {}.get(10, TimeUnit.SECONDS)
}

fun main() {
    val main = Thread.currentThread()
    val started = CountDownLatch(1)
    val release = CountDownLatch(1)
    val active = AtomicInteger()
    val maxActive = AtomicInteger()
    val committed = AtomicInteger()
    val firstWorker = AtomicReference<Thread>()
    val workerFailure = AtomicReference<Throwable>()
    val failedPost = CountDownLatch(1)
    val priorHandler = Thread.getDefaultUncaughtExceptionHandler()
    Thread.setDefaultUncaughtExceptionHandler { thread, error ->
        if (thread.name == "crepus-actions") {
            workerFailure.set(error)
            failedPost.countDown()
        }
        else priorHandler?.uncaughtException(thread, error)
    }
    try {
        val calls = Collections.synchronizedList(mutableListOf<Int>())
        val workers = Collections.synchronizedSet(mutableSetOf<Long>())
        val results = mutableListOf<Int>()
        val count = 1001
        CrepusActions.dispatch = { action ->
            val worker = Thread.currentThread()
            check(worker !== main && worker.isDaemon && worker.name == "crepus-actions")
            workers.add(worker.id)
            val concurrent = active.incrementAndGet()
            maxActive.updateAndGet { maxOf(it, concurrent) }
            val n = action.toInt()
            check(committed.get() == n) { "dispatch $n ran before preceding sink completed" }
            if (n == 0) {
                firstWorker.set(worker)
                started.countDown()
                check(release.await(10, TimeUnit.SECONDS))
            }
            calls.add(n)
            active.decrementAndGet()
            action
        }
        CrepusActions.resultSink = { result ->
            check(Thread.currentThread() === main)
            val n = result.toInt()
            results.add(n)
            check(committed.incrementAndGet() == n + 1)
        }
        CrepusActions.perform("0")
        check(started.await(10, TimeUnit.SECONDS))
        for (n in 1 until count) CrepusActions.perform(n.toString())
        check(calls.isEmpty() && results.isEmpty())
        // Queued actions retain the dispatch and sink captured when submitted.
        CrepusActions.dispatch = { error("replacement dispatch used") }
        CrepusActions.resultSink = { error("replacement sink used") }
        release.countDown()
        val first = nextPost()
        // With 1000 tasks queued, this worker can wait only for sink delivery.
        // The previous asynchronous implementation instead advances and fails dispatch 1.
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (firstWorker.get().state != Thread.State.WAITING && workerFailure.get() == null) {
            check(System.nanoTime() < deadline) { "worker did not wait for first sink" }
            Thread.yield()
        }
        check(workerFailure.get() == null) { "worker failed: ${workerFailure.get()}" }
        check(calls == listOf(0) && results.isEmpty())
        first.run()
        repeat(count - 1) { nextPost().run() }
        waitForWorker()
        val expected = (0 until count).toList()
        check(calls == expected && results == expected)
        check(committed.get() == count)
        check(maxActive.get() == 1 && active.get() == 0 && workers.size == 1)
        check(android.os.MainPosts.queue.isEmpty() && workerFailure.get() == null)

        // Recoverable dispatch failures still complete on main before the next dispatch.
        val failureResults = mutableListOf<String>()
        CrepusActions.dispatch = { action ->
            if (action == "bad") throw IllegalStateException("dispatcher failed")
            check(failureResults == listOf("error:dispatcher failed"))
            action
        }
        CrepusActions.resultSink = { result ->
            check(Thread.currentThread() === main)
            failureResults.add(result)
        }
        CrepusActions.perform("bad")
        CrepusActions.perform("good")
        // Both queued jobs retain the hooks captured at submission.
        CrepusActions.dispatch = { error("replacement failure dispatch used") }
        CrepusActions.resultSink = { error("replacement failure sink used") }
        nextPost().run()
        nextPost().run()
        waitForWorker()
        check(failureResults == listOf("error:dispatcher failed", "good"))
        check(workerFailure.get() == null && android.os.MainPosts.queue.isEmpty())

        // Accepted main posts must finish even if dispatcher restores interruption,
        // or another thread interrupts the worker while delivery is pending.
        for (change in listOf(false, true)) {
            for (selfInterrupt in listOf(false, true)) {
                val interruptedWorker = AtomicReference<Thread>()
                val firstDispatched = CountDownLatch(1)
                val secondDispatched = CountDownLatch(1)
                val firstCommitted = java.util.concurrent.atomic.AtomicBoolean(false)
                val interruptedResults = mutableListOf<String>()
                val dispatch: (String) -> String = { action ->
                    if (action == "first") {
                        interruptedWorker.set(Thread.currentThread())
                        firstDispatched.countDown()
                        if (selfInterrupt) Thread.currentThread().interrupt()
                    } else {
                        check(firstCommitted.get()) { "interruption bypassed preceding sink" }
                        secondDispatched.countDown()
                    }
                    action
                }
                CrepusActions.dispatch = dispatch
                CrepusRustActions.queuedDispatch = { action, _, _ -> dispatch(action) }
                CrepusActions.resultSink = { result ->
                    check(Thread.currentThread() === main)
                    if (result == "first") firstCommitted.set(true)
                    interruptedResults.add(result)
                }
                fun submit(action: String) {
                    if (change) CrepusActions.performChange(action, "field", JsonElement())
                    else CrepusActions.perform(action)
                }
                submit("first")
                submit("second")
                check(firstDispatched.await(10, TimeUnit.SECONDS))
                val firstPost = nextPost()
                val worker = interruptedWorker.get()
                if (!selfInterrupt) worker.interrupt()
                val waitDeadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
                while (workerFailure.get() == null &&
                    (worker.state != Thread.State.WAITING || worker.isInterrupted)) {
                    check(System.nanoTime() < waitDeadline) { "interrupted worker did not resume delivery wait" }
                    Thread.yield()
                }
                check(workerFailure.get() == null) { "interrupted delivery escaped: ${workerFailure.get()}" }
                check(!firstCommitted.get() && secondDispatched.count == 1L)
                firstPost.run()
                nextPost().run()
                waitForWorker()
                check(interruptedResults == listOf("first", "second"))
                check(secondDispatched.count == 0L && workerFailure.get() == null)
            }
        }
        CrepusRustActions.queuedDispatch = { _, _, _ -> "{}" }
        // Observe restored status directly at the actual helper's return; the
        // ExecutorService may clear interruption before starting its next job.
        val awaitDelivery = CrepusActions::class.java.getDeclaredMethod(
            "awaitDelivery", CountDownLatch::class.java
        )
        awaitDelivery.isAccessible = true
        Thread.currentThread().interrupt()
        try {
            awaitDelivery.invoke(CrepusActions, CountDownLatch(0))
            check(Thread.currentThread().isInterrupted)
        } finally {
            Thread.interrupted()
        }

        // A custom sink still throws on main, while finally releases the worker.
        val sinkError = IllegalStateException("custom sink failure")
        val delivered = mutableListOf<String>()
        CrepusActions.dispatch = { it }
        CrepusActions.resultSink = { result ->
            check(Thread.currentThread() === main)
            if (result == "throw") throw sinkError
            delivered.add(result)
        }
        CrepusActions.perform("throw")
        CrepusActions.perform("after-throw")
        check(runCatching { nextPost().run() }.exceptionOrNull() === sinkError)
        nextPost().run()
        waitForWorker()
        check(delivered == listOf("after-throw"))

        // A rejected result post fails before awaiting, so a later action can run.
        android.os.MainPosts.accept = false
        CrepusActions.perform("rejected")
        check(failedPost.await(10, TimeUnit.SECONDS)) { "rejected result post stalled worker" }
        check(workerFailure.getAndSet(null)?.message == "main looper unavailable")
        android.os.MainPosts.accept = true
        CrepusActions.perform("after-rejection")
        nextPost().run()
        waitForWorker()
        check(delivered == listOf("after-throw", "after-rejection"))
        check(android.os.MainPosts.queue.isEmpty() && workerFailure.get() == null)
        println("PASS: $count actions FIFO/exactly-once; max active=1; named daemon worker; preceding sink committed before dispatch; dispatcher Exceptions deliver errors; interrupted normal/change deliveries preserve FIFO and restore status; main sink exceptions and rejected posts release worker; captured hooks preserved")
    } finally {
        Thread.setDefaultUncaughtExceptionHandler(priorHandler)
    }
}
