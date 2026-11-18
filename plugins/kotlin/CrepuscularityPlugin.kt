package dev.crepuscularity.plugin

import java.io.ByteArrayOutputStream
import java.io.OutputStreamWriter
import java.nio.file.Files
import java.nio.file.Path

data class ViewIr(val version: Int, val json: String)
data class Event(val handler: String, val payload: Any? = null)

class ViewSession(val path: String, context: Map<String, Any?> = emptyMap()) {
    val context: MutableMap<String, Any?> = LinkedHashMap(context)
    private val handlers = LinkedHashMap<String, (Event, ViewSession) -> Unit>()

    fun on(handler: String, callback: (Event, ViewSession) -> Unit): ViewSession {
        handlers[handler] = callback
        return this
    }

    fun renderIr(): ViewIr = CrepuscularityPlugin.renderIr(path, context)

    fun renderHtml(): String = CrepuscularityPlugin.renderHtml(path, context)

    fun dispatch(handler: String): ViewIr = dispatch(Event(handler))

    fun dispatch(event: Event): ViewIr {
        applyBind(event.handler)
        handlers[event.handler]?.invoke(event, this)
        return renderIr()
    }

    private fun applyBind(handler: String) {
        if (!handler.startsWith("bind:")) return
        val rest = handler.removePrefix("bind:")
        val colon = rest.indexOf(':')
        if (colon <= 0) return
        context[rest.substring(0, colon)] = rest.substring(colon + 1)
    }
}

object CrepuscularityPlugin {
    fun renderIr(path: String, context: Map<String, Any?> = emptyMap()): ViewIr {
        val bin = System.getenv("CREPUS_BIN") ?: "crepus"
        val process = ProcessBuilder(bin, "native", "ir", "--stdin-json").redirectErrorStream(true).start()
        OutputStreamWriter(process.outputStream, Charsets.UTF_8).use {
            it.write(toJson(mapOf("template" to Files.readString(Path.of(path)), "context" to context)))
        }
        val stdout = ByteArrayOutputStream()
        process.inputStream.transferTo(stdout)
        val code = process.waitFor()
        val json = stdout.toString(Charsets.UTF_8)
        if (code != 0) error(json)
        val version = if (json.contains("\"version\":5") || json.contains("\"version\": 5")) 5 else -1
        return ViewIr(version, json)
    }

    fun renderHtml(path: String, context: Map<String, Any?> = emptyMap()): String {
        val content = renderIr(path, context).json.substringAfter("\"content\":\"", "").substringBefore("\"")
        return "<div data-crepus-kind=\"stack\" data-axis=\"column\">${escapeHtml(content)}</div>"
    }

    private fun escapeHtml(value: String): String =
        value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

    private fun toJson(value: Any?): String = when (value) {
        null -> "null"
        is String -> quoteJson(value)
        is Number, is Boolean -> value.toString()
        is Map<*, *> -> value.entries.joinToString(prefix = "{", postfix = "}") {
            "${quoteJson(it.key.toString())}:${toJson(it.value)}"
        }
        is Iterable<*> -> value.joinToString(prefix = "[", postfix = "]") { toJson(it) }
        is Array<*> -> value.joinToString(prefix = "[", postfix = "]") { toJson(it) }
        else -> quoteJson(value.toString())
    }

    private fun quoteJson(value: String): String = buildString {
        append('"')
        value.forEach { ch ->
            when (ch) {
                '"' -> append("\\\"")
                '\\' -> append("\\\\")
                '\b' -> append("\\b")
                '\u000C' -> append("\\f")
                '\n' -> append("\\n")
                '\r' -> append("\\r")
                '\t' -> append("\\t")
                else -> if (ch < ' ') append("\\u%04x".format(ch.code)) else append(ch)
            }
        }
        append('"')
    }
}
