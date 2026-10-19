package dev.crepuscularity.nativeshell

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.webkit.MimeTypeMap
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.floatOrNull
import kotlinx.serialization.json.jsonArray
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

object CrepusStateStore {
    private val json = Json { ignoreUnknownKeys = true }
    private var revision by mutableLongStateOf(0L)

    private data class Draft(val sequence: Long, val value: JsonElement)
    private val drafts = mutableMapOf<String, Draft>()
    private var nextSequence = 0L

    private val jsonNumber = Regex("""-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?""")

    fun snapshotDraft(raw: String): JsonElement {
        val value = json.parseToJsonElement(raw)
        fun validate(value: JsonElement) {
            when (value) {
                is JsonObject -> value.values.forEach(::validate)
                is JsonArray -> value.forEach(::validate)
                is JsonPrimitive -> if (!value.isString && value.content !in listOf("null", "true", "false")) {
                    require(jsonNumber.matches(value.content) && value.content.toDoubleOrNull()?.isFinite() == true) { "invalid or non-finite change number" }
                }
            }
        }
        validate(value)
        return value
    }

    fun beginDraft(path: String, value: JsonElement): Long {
        val sequence = ++nextSequence
        drafts[path] = Draft(sequence, value)
        revision += 1
        return sequence
    }

    fun clearDraft(path: String, sequence: Long) {
        if (drafts[path]?.sequence == sequence) {
            drafts.remove(path)
            revision += 1
        }
    }

    fun draftText(path: String, fallback: () -> String): String {
        revision
        val value = drafts[path]?.value ?: return fallback()
        if (value is JsonPrimitive && value.isString) return value.content
        return text("__crepusDraftValue", "__crepusDraftValue", value)
    }

    fun draftBool(path: String, fallback: () -> Boolean): Boolean {
        revision
        return when (val value = drafts[path]?.value ?: return fallback()) {
            JsonNull -> false
            is JsonPrimitive -> if (value.isString) value.content.isNotEmpty()
                else value.booleanOrNull ?: (value.content.toDoubleOrNull()?.let { it != 0.0 } ?: false)
            is JsonArray -> value.isNotEmpty()
            is JsonObject -> value.isNotEmpty()
        }
    }

    fun draftNumber(path: String, fallback: () -> Float): Float {
        revision
        val draft = drafts[path]?.value ?: return fallback()
        if (draft is JsonPrimitive && draft.isString) {
            return number("__crepusDraftValue", "__crepusDraftValue", draft)
        }
        val value = (draft as? JsonPrimitive)?.floatOrNull
        return if (value != null && value.isFinite()) value else 0f
    }

    fun applyResult(raw: String) {
        if (CrepusRustActions.storeResultJson(raw)) {
            revision += 1
        }
    }

    fun text(expr: String, scopeName: String? = null, scope: JsonElement? = null): String {
        revision
        return CrepusRustActions.evalText(expr, scopeName, scope?.toString())
    }

    fun bool(expr: String, scopeName: String? = null, scope: JsonElement? = null): Boolean {
        revision
        return CrepusRustActions.evalBool(expr, scopeName, scope?.toString())
    }

    fun number(expr: String, scopeName: String? = null, scope: JsonElement? = null): Float {
        revision
        val value = CrepusRustActions.evalNumber(expr, scopeName, scope?.toString()).toFloat()
        return if (value.isFinite()) value else 0f
    }

    fun items(expr: String, scopeName: String? = null, scope: JsonElement? = null): List<JsonElement> {
        revision
        val payload = runCatching { json.parseToJsonElement(CrepusRustActions.evalItemsJson(expr, scopeName, scope?.toString())) as? JsonArray }
            .getOrNull()
        return payload?.jsonArray?.toList() ?: emptyList()
    }
}

object CrepusRustActions {
    init {
        System.loadLibrary("crepus_mobile_actions")
    }

    private var installedActivity = java.lang.ref.WeakReference<ComponentActivity>(null)
    @Volatile private var pickerGeneration = 0L
    private val pickerWorker = java.util.concurrent.ThreadPoolExecutor(
        1, 1, 0L, java.util.concurrent.TimeUnit.MILLISECONDS,
        java.util.concurrent.ArrayBlockingQueue<Runnable>(4),
        { task -> Thread(task, "crepus-picker-io").apply { isDaemon = true } },
        java.util.concurrent.ThreadPoolExecutor.AbortPolicy(),
    )
    private var pendingPickerAction: String? = null
    private var openDocuments: (() -> Unit)? = null

    external fun dispatchAction(action: String): Boolean
    external fun dispatchActionJson(action: String): String
    external fun dispatchAndStoreJson(action: String): String
    external fun dispatchChangeJson(action: String, bind: String, valueJson: String): String
    internal fun dispatchQueuedChange(action: String, bind: String, valueJson: String): String =
        runCatching {
            if (action.isNotEmpty()) {
                dispatchChangeJson(action, bind, valueJson)
            } else {
                require(bind.split('.', limit = 128).size <= 126) { "binding path exceeds 126 segments" }
                val value = CrepusStateStore.snapshotDraft(valueJson)
                JsonObject(mapOf(
                    "ok" to JsonPrimitive(true),
                    "action" to JsonPrimitive("bind"),
                    "bindings" to JsonObject(mapOf(bind to value)),
                )).toString()
            }
        }.getOrElse(::queuedChangeError)

    internal fun queuedChangeError(error: Throwable): String = JsonObject(mapOf(
        "ok" to JsonPrimitive(false),
        "error" to JsonPrimitive(error.message ?: error.toString()),
    )).toString()

    external fun lastResult(): String
    external fun storeResultJson(json: String): Boolean
    external fun evalText(expr: String, scopeName: String?, scopeJson: String?): String
    external fun evalBool(expr: String, scopeName: String?, scopeJson: String?): Boolean
    external fun evalNumber(expr: String, scopeName: String?, scopeJson: String?): Double
    external fun evalItemsJson(expr: String, scopeName: String?, scopeJson: String?): String
    external fun initAndroid(context: Context)
    external fun lastError(): String
    external fun shutdownAndroid()

    fun install(activity: ComponentActivity) {
        pickerGeneration += 1
        val generation = pickerGeneration
        val context = activity.applicationContext
        installedActivity = java.lang.ref.WeakReference(activity)
        pendingPickerAction = null
        openDocuments = null
        initAndroid(context)
        val filePicker =
            activity.registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
                if (pickerGeneration != generation || installedActivity.get() == null) return@registerForActivityResult
                val action = synchronized(this) {
                    val pending = pendingPickerAction
                    pendingPickerAction = null
                    pending
                } ?: return@registerForActivityResult
                importPickedFiles(action, uris.toList(), context, generation, CrepusActions.resultSink)
            }
        openDocuments = { filePicker.launch(arrayOf("*/*")) }
        CrepusActions.dispatch = { action -> dispatchHostActionOnMain(action) ?: dispatchAndStoreJson(action) }
        CrepusActions.resultSink = { result -> CrepusStateStore.applyResult(result) }
    }

    fun detach(activity: ComponentActivity): Boolean {
        if (installedActivity.get() !== activity) return false
        pickerGeneration += 1
        installedActivity.clear()
        openDocuments = null
        pendingPickerAction = null
        return true
    }

    private fun importPickedFiles(
        action: String,
        uris: List<Uri>,
        context: Context,
        generation: Long,
        sink: (String) -> Unit,
    ) {
        try {
            pickerWorker.execute {
                if (pickerGeneration != generation) return@execute
                val copied = mutableListOf<File>()
                try {
                    val result = filePickerResultJson(action, uris, context, copied)
                    val published = java.util.concurrent.atomic.AtomicBoolean(false)
                    val delivery = java.util.concurrent.FutureTask<Unit> {
                        if (pickerGeneration == generation && installedActivity.get() != null) {
                            published.set(true)
                            sink(result)
                        }
                    }
                    if (android.os.Handler(android.os.Looper.getMainLooper()).post(delivery)) {
                        // Only the I/O worker waits; Activity callbacks never await this queue.
                        runCatching { delivery.get() }
                    }
                    if (!published.get()) copied.forEach { it.delete() }
                } catch (_: Exception) {
                    copied.forEach { it.delete() }
                    val delivery = Runnable {
                        if (pickerGeneration == generation && installedActivity.get() != null) {
                            sink(errorJson(action, "could not import selected files"))
                        }
                    }
                    android.os.Handler(android.os.Looper.getMainLooper()).post(delivery)
                }
            }
        } catch (_: java.util.concurrent.RejectedExecutionException) {
            if (pickerGeneration == generation && installedActivity.get() != null) {
                sink(errorJson(action, "file import queue is full"))
            }
        }
    }

    private fun dispatchHostActionOnMain(action: String): String? {
        if (android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            return dispatchHostAction(action)
        }
        val task = java.util.concurrent.FutureTask<String?> { dispatchHostAction(action) }
        check(android.os.Handler(android.os.Looper.getMainLooper()).post(task)) {
            "main looper unavailable"
        }
        return task.get()
    }

    private fun dispatchHostAction(action: String): String? {
        dispatchNamedHostAction(action)?.let { return it }
        val request = runCatching { JSONObject(action) }.getOrElse {
            val separator = action.indexOf('.')
            if (separator < 0) return null
            JSONObject()
                .put("kind", "plugin")
                .put("capability", action.substring(0, separator))
                .put("method", action.substring(separator + 1))
        }
        if (request.optString("kind") != "plugin") return null
        val capability = request.optString("capability")
        if (capability == "app" || capability == "device" || capability == "preferences") return null
        val method = request.optString("method")
        val actionName = "$capability.$method"
        return runCatching {
            val value = hostPluginValue(capability, method, request.optJSONObject("payload"))
            JSONObject()
                .put("ok", true)
                .put("action", actionName)
                .put(
                    "value",
                    JSONObject()
                        .put("capability", capability)
                        .put("method", method)
                        .put("value", value),
                ).toString()
        }.getOrElse { error ->
            JSONObject()
                .put("ok", false)
                .put("action", actionName)
                .put("error", error.message ?: error.toString())
                .toString()
        }
    }

    private fun hostPluginValue(capability: String, method: String, payload: JSONObject?): Any =
        when (capability) {
            else -> error("unsupported host capability: $capability")
        }

    private fun dispatchNamedHostAction(action: String): String? =
        when (action) {
            "import_files" -> synchronized(this) { runCatching {
                val open = openDocuments ?: error("file picker unavailable")
                check(pendingPickerAction == null) { "picker already active" }
                pendingPickerAction = action
                try {
                    open()
                    pendingJson(action)
                } catch (error: Exception) {
                    pendingPickerAction = null
                    throw error
                }
            }.getOrElse { error -> errorJson(action, error.message ?: error.toString()) } }
            else -> null
        }

    private fun emit(result: String) {
        CrepusActions.resultSink(result)
    }

    private fun pendingJson(action: String): String =
        JSONObject()
            .put("ok", true)
            .put("action", action)
            .put("pending", true)
            .toString()

    private fun errorJson(action: String, error: String): String =
        JSONObject()
            .put("ok", false)
            .put("action", action)
            .put("error", error)
            .toString()

    private fun filePickerResultJson(action: String, uris: List<Uri>, context: Context, copied: MutableList<File>): String {
        val files =
            uris.mapNotNull { uri ->
                runCatching {
                    val name = queryDisplayName(context, uri)
                    val mime = context.contentResolver.getType(uri) ?: "application/octet-stream"
                    val file = copyToCache(context, uri, name, mime)
                    copied.add(file)
                    JSONObject()
                        .put("name", name)
                        .put("mimeType", mime)
                        .put("bytes", file.length())
                        .put("filePath", file.absolutePath)
                        .put("importSource", "android-document-picker")
                }.getOrNull()
            }
        val failedCount = uris.size - files.size
        val result = JSONObject()
            .put("ok", failedCount == 0)
            .put("action", action)
            .put("value", JSONObject().put("files", JSONArray(files)))
        if (failedCount > 0) {
            result.put("error", "could not import selected files")
            result.put("failedCount", failedCount)
        }
        return result.toString()
    }

    private fun copyToCache(context: Context, uri: Uri, name: String, mime: String): File {
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime)
            ?: name.substringAfterLast('.', "bin")
        val file = File.createTempFile("crepus-media-", ".$ext", context.cacheDir)
        try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                file.outputStream().use { output -> input.copyTo(output) }
            } ?: error("media item unavailable")
            return file
        } catch (error: Exception) {
            file.delete()
            throw error
        }
    }

    private fun queryDisplayName(context: Context, uri: Uri): String {
        context.contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val index = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                    if (index >= 0) return cursor.getString(index)
                }
            }
        return uri.lastPathSegment ?: "file"
    }
}

object CrepusActionState {
    val lastResult = mutableStateOf("{}")
    val lastError = mutableStateOf<String?>(null)

    fun dispatch(action: String) {
        record(CrepusActions.dispatch(action))
    }

    fun record(result: String) {
        lastResult.value = CrepusRustActions.lastResult()
        lastError.value = CrepusRustActions.lastError().ifBlank { null }
    }
}
