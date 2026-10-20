package dev.crepuscularity.plugin

import java.io.IOException
import java.io.OutputStreamWriter
import java.nio.ByteBuffer
import java.nio.file.Files
import java.nio.file.Path
import java.util.concurrent.FutureTask

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
        val templatePath = Path.of(path).toAbsolutePath()
        val template = Files.readString(templatePath)
        val json = runCli(listOf("native", "ir", "--stdin-json"),
            toJson(mapOf("template" to template, "context" to context, "filename" to path,
                "baseDir" to templatePath.parent.toString())))
        val version = Regex("\"version\"\\s*:\\s*(\\d+)").find(json)
            ?.groupValues?.get(1)?.toIntOrNull() ?: error("IR version missing or invalid")
        return ViewIr(version, json)
    }

    fun renderHtml(path: String, context: Map<String, Any?> = emptyMap()): String {
        val contextFile = Files.createTempFile("crepus-kotlin-context-", ".json")
        try {
            Files.writeString(contextFile, toJson(context), Charsets.UTF_8)
            return runCli(listOf("render", Path.of(path).toAbsolutePath().toString(),
                "--ctx", contextFile.toString()), "")
        } finally {
            Files.deleteIfExists(contextFile)
        }
    }

    private fun runCli(arguments: List<String>, input: String): String {
        val bin = System.getenv("CREPUS_BIN") ?: "crepus"
        val process = ProcessBuilder(listOf(bin) + arguments).start()
        val stderr = FutureTask { process.errorStream.readBytes().toString(Charsets.UTF_8) }
        Thread(stderr, "crepus-kotlin-stderr").apply { isDaemon = true }.start()
        // Start draining stdout before writing a potentially large request.
        val stdout = FutureTask { process.inputStream.readBytes() }
        Thread(stdout, "crepus-kotlin-stdout").apply { isDaemon = true }.start()
        try {
            var writeError: IOException? = null
            try {
                OutputStreamWriter(process.outputStream, Charsets.UTF_8).use { it.write(input) }
            } catch (error: IOException) {
                writeError = error
            }
            val output = stdout.get()
            val code = process.waitFor()
            val diagnostic = stderr.get()
            if (code != 0) error(diagnostic)
            if (writeError != null) throw writeError
            return Charsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(output)).toString()
        } finally {
            process.destroy()
        }
    }

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
                else -> if (ch < ' ' || Character.isSurrogate(ch)) append("\\u%04x".format(ch.code)) else append(ch)
            }
        }
        append('"')
    }
}
