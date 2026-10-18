package dev.crepuscularity.plugin

import java.nio.file.Files
import java.nio.file.Path

fun main(args: Array<String>) {
    if (args.size > 1 && args[1] == "failure") {
        try {
            CrepuscularityPlugin.renderIr(args[0], mapOf("name" to "Ada"))
            throw AssertionError("expected CLI failure")
        } catch (error: IllegalStateException) {
            check(error.message?.contains("diagnostic") == true) { "failure diagnostic missing: $error" }
        }
        return
    }
    val ir = CrepuscularityPlugin.renderIr(args[0], mapOf("name" to "Ada"))
    check(ir.version == 7 && ir.json.startsWith("{") && ir.json.contains("Hello Ada")) { "unexpected IR: $ir" }
    val frontend = CrepuscularityPlugin.renderIr(Path.of(args[0]).parent.resolve("frontend.svelte").toString(), mapOf("name" to "Ada"))
    check(frontend.json.contains("Frontend content") && !frontend.json.contains("ignored")) { "frontend output: $frontend" }
    val directory = Files.createTempDirectory("crepus-kotlin-include-")
    val source = directory.resolve("index.crepus")
    val part = directory.resolve("part.crepus")
    try {
        Files.writeString(source, "include part.crepus name={name}\n")
        Files.writeString(part, "span\n  \"Included {name}\"\n")
        val included = CrepuscularityPlugin.renderIr(source.toString(), mapOf("name" to "Ada"))
        check(included.json.contains("Included Ada")) { "include output: $included" }
    } finally {
        Files.deleteIfExists(source)
        Files.deleteIfExists(part)
        Files.deleteIfExists(directory)
    }
}
