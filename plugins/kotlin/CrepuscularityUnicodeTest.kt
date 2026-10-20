package dev.crepuscularity.plugin

import java.nio.file.Files
import java.util.Comparator

/** Default: actual CLI Unicode acceptance. Optional transport mode uses a labelled byte fixture. */
private fun rejectMalformedUnicode(label: String, call: () -> Unit) {
    var rejected = false
    try { call() } catch (expected: Exception) { rejected = true }
    if (!rejected) throw AssertionError("Unpaired UTF-16 accepted: $label")
}

private fun transportResponse(mode: String, path: String, html: Boolean) {
    try {
        val output = if (html) CrepuscularityPlugin.renderHtml(path) else CrepuscularityPlugin.renderIr(path).json
        if (mode != "valid") throw AssertionError("Malformed CLI UTF-8 accepted: $mode html=$html")
        if (!output.contains("雪 🦋")) throw AssertionError("CLI Unicode changed")
    } catch (expected: java.nio.charset.CharacterCodingException) {
        if (mode != "ff" && mode != "sequence") throw expected
    } catch (expected: IllegalStateException) {
        if (mode != "failure" || expected.message != "雪 diagnostic\n") throw expected
    }
}

private fun largeTransport(mode: String, path: String) {
    try {
        val ir = CrepuscularityPlugin.renderIr(path, mapOf("blob" to "雪".repeat(400_000)))
        if (mode != "large" || !ir.json.contains("Exact payload 雪"))
            throw AssertionError("Incomplete write accepted or response changed: $mode")
    } catch (expected: java.io.IOException) {
        if (mode != "large_unread") throw expected
    } catch (expected: IllegalStateException) {
        if (mode != "large_failure" || expected.message != "雪 early diagnostic\n") throw expected
    }
}

fun main(args: Array<String>) {
    if (args.size == 4 && args[0] == "transport") {
        if (args[1].startsWith("large")) largeTransport(args[1], args[2])
        else transportResponse(args[1], args[2], args[3] == "html")
        return
    }
    val directory = Files.createTempDirectory("crepus-kotlin-unicode-")
    try {
        val source = directory.resolve("view.crepus")
        Files.writeString(source, "span \"{value}\"\n")
        val valid = "雪 🦋"
        val context = mapOf("value" to valid)
        if (!CrepuscularityPlugin.renderIr(source.toString(), context).json.contains(valid))
            throw AssertionError("Valid Unicode changed in IR")
        if (!CrepuscularityPlugin.renderHtml(source.toString(), context).contains(valid))
            throw AssertionError("Valid Unicode changed in HTML")
        val invalid = listOf("\uD800", "\uDC00", "\uD800x", "\uDC00\uD800")
        invalid.forEachIndexed { index, value ->
            val badValue = mapOf("value" to value)
            val badKey = mapOf(value to "ordinary")
            rejectMalformedUnicode("IR value $index") { CrepuscularityPlugin.renderIr(source.toString(), badValue) }
            rejectMalformedUnicode("HTML value $index") { CrepuscularityPlugin.renderHtml(source.toString(), badValue) }
            rejectMalformedUnicode("IR key $index") { CrepuscularityPlugin.renderIr(source.toString(), badKey) }
            rejectMalformedUnicode("HTML key $index") { CrepuscularityPlugin.renderHtml(source.toString(), badKey) }
        }
        println("Kotlin actual CLI Unicode preservation/rejection PASS")
    } finally {
        Files.walk(directory).use { paths ->
            paths.sorted(Comparator.reverseOrder()).forEach { Files.delete(it) }
        }
    }
}
