package dev.crepuscularity.plugin

import java.nio.file.Files
import java.nio.file.Path
import java.util.Comparator

/** Actual CLI HTML acceptance checks, separate from the IR-only smoke fixture. */
fun main(args: Array<String>) {
    if (args.size == 2 && args[0] == "failure") {
        try {
            CrepuscularityPlugin.renderHtml(args[1])
            error("expected CLI failure")
        } catch (failure: IllegalStateException) {
            check(failure.message?.contains("html diagnostic") == true) { failure }
        }
        val context = Path.of(Files.readString(Path.of(System.getenv("CREPUS_HTML_CONTEXT_LOG"))).trim())
        check(!Files.exists(context)) { "failed CLI leaked context file: $context" }
        return
    }
    val dir = Files.createTempDirectory("crepus-kotlin-html-test-")
    try {
        val source = dir.resolve("index.crepus")
        Files.writeString(source, "span\n  \"First {name}\"\nbutton @click=\"save\"\n  \"Second\"\n")
        var html = CrepuscularityPlugin.renderHtml(source.toString(), mapOf("name" to "雪 & \"quoted\"\nnext"))
        check(html.contains("First 雪 &amp; &quot;quoted&quot;\nnext")) { html }
        check(html.contains("<button data-onclick=\"save\">Second</button>")) { html }
        check(html.indexOf("First") < html.indexOf("Second")) { html }

        Files.writeString(dir.resolve("part.crepus"), "span\n  \"Included {name}\"\n")
        Files.writeString(source, "include part.crepus name={name}\n")
        check(CrepuscularityPlugin.renderHtml(source.toString(), mapOf("name" to "Ada")).contains("Included Ada"))

        val frontend = dir.resolve("frontend.svelte")
        Files.writeString(frontend, "<p>Frontend {name}</p><button>Second</button>")
        html = CrepuscularityPlugin.renderHtml(frontend.toString(), mapOf("name" to "Ada"))
        check(html.contains("Frontend Ada") && html.contains("Second")) { html }

        Files.writeString(source, "span\n  \"Value {name}\"\n")
        Files.writeString(dir.resolve("context.toml"), "name = \"AUTOLOAD-MUST-NOT-APPEAR\"\n")
        check(!CrepuscularityPlugin.renderHtml(source.toString()).contains("AUTOLOAD-MUST-NOT-APPEAR"))
        val session = ViewSession(source.toString(), mapOf("name" to "Session"))
        check(session.renderHtml().contains("Value Session"))

        try {
            CrepuscularityPlugin.renderHtml(source.toString(), mapOf("nested" to mapOf("name" to "Ada")))
            error("unsupported nested context silently accepted")
        } catch (failure: IllegalStateException) {
            check(failure.message?.contains("context object values") == true) { failure }
        }
    } finally {
        Files.walk(dir).use { paths ->
            paths.sorted(Comparator.reverseOrder()).forEach { Files.deleteIfExists(it) }
        }
    }
}
