import kotlinx.serialization.json.*
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

private fun next() = checkNotNull(android.os.MainPosts.queue.poll(5, TimeUnit.SECONDS)) { "main delivery timed out" }
private fun text(path: String) = CrepusStateStore.draftText(path) { CrepusStateStore.text(path) }
fun main() {
    val main = Thread.currentThread()
    val started = CountDownLatch(1)
    val release = CountDownLatch(1)
    val calls = mutableListOf<String>()
    Probe.model["items.0.title"] = JsonPrimitive("old")
    Probe.model["items.1.title"] = JsonPrimitive("other")
    CrepusActions.dispatch = {
        started.countDown()
        check(release.await(5, TimeUnit.SECONDS))
        "{}"
    }
    CrepusActions.resultSink = { check(Thread.currentThread() === main); CrepusStateStore.applyResult(it) }
    CrepusActions.perform("hold")
    check(started.await(5, TimeUnit.SECONDS))
    Probe.dispatch = { _, path, raw ->
        check(Thread.currentThread() !== main)
        calls.add(raw)
        """{"bindings":{"$path":"transformed"}}"""
    }
    try {
        for (value in listOf("a", "ab", "abc")) {
            CrepusActions.performChange("edit", "items.0.title", JsonPrimitive(value))
            check(text("items.0.title") == value)
        }
        check(Probe.text("items.0.title") == "old")
        check(text("items.1.title") == "other")
    } finally { release.countDown() }
    next().run() // held ordinary action
    next().run(); check(text("items.0.title") == "abc")
    next().run(); check(text("items.0.title") == "abc")
    next().run(); check(text("items.0.title") == "transformed")
    check(calls == listOf("\"a\"", "\"ab\"", "\"abc\""))

    Probe.dispatch = { _, _, _ -> """{"ok":false,"error":"rejected"}""" }
    CrepusActions.performChange("reject", "items.0.title", JsonPrimitive("bad"))
    check(text("items.0.title") == "bad")
    next().run(); check(text("items.0.title") == "transformed")

    // Bind-only must prepare an envelope, leaving model commits to captured sink.
    val custom = mutableListOf<String>()
    CrepusActions.resultSink = { custom.add(it) }
    CrepusActions.performChange(null, "enabled", JsonPrimitive(true))
    check(CrepusStateStore.draftBool("enabled") { false })
    val bindResult = next()
    check(!Probe.bool("enabled"))
    CrepusActions.resultSink = { error("replacement sink used") }
    bindResult.run()
    check(custom.size == 1 && !Probe.bool("enabled"))
    check(!CrepusStateStore.draftBool("enabled") { false })
    CrepusStateStore.applyResult(custom.single()); check(Probe.bool("enabled"))

    CrepusActions.resultSink = { CrepusStateStore.applyResult(it) }
    CrepusActions.performChange(null, "amount", JsonPrimitive(0.75f))
    check(CrepusStateStore.draftNumber("amount") { 0f } == 0.75f)
    next().run(); check(CrepusStateStore.number("amount") == 0.75f)

    for ((raw, expected) in listOf(
        "null" to false,
        "false" to false,
        "true" to true,
        "0" to false,
        "-2" to true,
        "0.5" to true,
        "\"\"" to false,
        "\"false\"" to true,
        "\"0\"" to true,
        "[]" to false,
        "[0]" to true,
        "{}" to false,
        "{\"a\":0}" to true
    )) {
        val sequence = CrepusStateStore.beginDraft("truthy", CrepusStateStore.snapshotDraft(raw))
        check(CrepusStateStore.draftBool("truthy") { !expected } == expected) { raw }
        CrepusStateStore.clearDraft("truthy", sequence)
        check(CrepusStateStore.draftBool("truthy") { !expected } == !expected)
    }

    var expectedScope: JsonElement? = null
    Probe.scopedText = { expr, name, scope ->
        check(expr == "__crepusDraftValue" && name == expr)
        val value = Json.parseToJsonElement(checkNotNull(scope))
        check(value == expectedScope)
        check(value !is JsonPrimitive || !value.isString) { "strings must stay local" }
        "formatted by Rust"
    }
    for (raw in listOf("null", "false", "true", "0", "-0.0", "1e20", "[]", "[0]", "{}")) {
        expectedScope = CrepusStateStore.snapshotDraft(raw)
        val sequence = CrepusStateStore.beginDraft("display", expectedScope)
        check(CrepusStateStore.draftText("display") { "STALE" } == "formatted by Rust") { raw }
        CrepusStateStore.clearDraft("display", sequence)
        check(CrepusStateStore.draftText("display") { "absent" } == "absent")
    }
    Probe.scopedText = { expr, _, _ -> Probe.text(expr) }

    for (raw in listOf("null", "false", "true", "[]", "[0]", "{}", "1e100")) {
        val sequence = CrepusStateStore.beginDraft("numeric-other", CrepusStateStore.snapshotDraft(raw))
        check(CrepusStateStore.draftNumber("numeric-other") { -999f } == 0f) { raw }
        CrepusStateStore.clearDraft("numeric-other", sequence)
        check(CrepusStateStore.draftNumber("numeric-other") { -999f } == -999f)
    }
    for ((raw, expected) in listOf("0.75" to 0.75, "-2.5" to -2.5, "1e2" to 100.0, "invalid" to 0.0, "NaN" to 0.0, "Infinity" to 0.0, "1e9999" to 0.0)) {
        Probe.scopedNumber = { expr, name, scope ->
            check(expr == "__crepusDraftValue" && name == expr)
            check(Json.parseToJsonElement(checkNotNull(scope)) == JsonPrimitive(raw))
            expected
        }
        val sequence = CrepusStateStore.beginDraft("numeric-text", JsonPrimitive(raw))
        check(CrepusStateStore.draftNumber("numeric-text") { -999f } == expected.toFloat())
        CrepusStateStore.clearDraft("numeric-text", sequence)
    }
    Probe.scopedNumber = { expr, _, _ -> Probe.number(expr) }

    var reentered = false
    CrepusActions.resultSink = { raw ->
        CrepusStateStore.applyResult(raw)
        if (!reentered) {
            reentered = true
            CrepusActions.performChange(null, "items.0.title", JsonPrimitive("newer"))
        }
    }
    CrepusActions.performChange(null, "items.0.title", JsonPrimitive("first"))
    next().run(); check(text("items.0.title") == "newer")
    next().run(); check(text("items.0.title") == "newer")

    // A throwing main sink still clears its draft and releases the action lane.
    val failure = IllegalStateException("sink failure")
    CrepusActions.resultSink = { throw failure }
    CrepusActions.performChange(null, "items.0.title", JsonPrimitive("throw"))
    CrepusActions.resultSink = { CrepusStateStore.applyResult(it) }
    CrepusActions.performChange(null, "items.1.title", JsonPrimitive("survives"))
    check(runCatching { next().run() }.exceptionOrNull() === failure)
    check(text("items.0.title") == "newer")
    next().run(); check(text("items.1.title") == "survives")
    // Caller-owned JsonObject backing maps cannot mutate the recorded snapshot.
    CrepusActions.resultSink = { CrepusStateStore.applyResult(it) }
    val backing = mutableMapOf<String, JsonElement>("name" to JsonPrimitive("captured"))
    val value = JsonObject(backing)
    val owned = CrepusStateStore.snapshotDraft(value.toString())
    val sequence = CrepusStateStore.beginDraft("owned", owned)
    backing["name"] = JsonPrimitive("mutated")
    check(owned.jsonObject["name"] == JsonPrimitive("captured"))
    CrepusStateStore.clearDraft("owned", sequence)
    CrepusActions.performChange(null, "owned", value)
    backing["name"] = JsonPrimitive("later")
    next().run()
    check(Probe.model["owned"]?.jsonObject?.get("name") == JsonPrimitive("mutated"))
    for (raw in listOf("01", "+1", "NaN", "Infinity", "{\"nested\":01}")) {
        check(runCatching { CrepusStateStore.snapshotDraft(raw) }.isFailure) { raw }
    }
    val invalidNumber = object : Number() {
        override fun toByte() = 1.toByte()
        override fun toShort() = 1.toShort()
        override fun toInt() = 1
        override fun toLong() = 1L
        override fun toFloat() = 1f
        override fun toDouble() = 1.0
        override fun toString() = "01"
    }
    val errors = mutableListOf<String>()
    CrepusActions.resultSink = { errors.add(it) }
    for (invalid in listOf(JsonPrimitive(invalidNumber), JsonPrimitive(Float.NaN), JsonPrimitive(Double.POSITIVE_INFINITY), JsonObject(mapOf("bad" to JsonPrimitive(Double.NEGATIVE_INFINITY))))) {
        CrepusActions.performChange(null, "invalid", invalid)
    }
    check(errors.size == 4 && errors.all { Json.parseToJsonElement(it).jsonObject["ok"] == JsonPrimitive(false) })
    check(CrepusStateStore.draftNumber("invalid") { 0.25f } == 0.25f)
    check(android.os.MainPosts.queue.isEmpty())
    println("PASS Kotlin async bound edits: actual queue/state/helper; labelled Android/Compose/FFI stubs; immediate drafts, sequential acknowledgments, transformed success/failure, canonical paths, captured sink, reentrancy, bind-only no early write")
}
