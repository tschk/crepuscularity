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
import kotlinx.serialization.json.jsonArray
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

object CrepusStateStore {
    private val json = Json { ignoreUnknownKeys = true }
    private var revision by mutableLongStateOf(0L)

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
        return CrepusRustActions.evalNumber(expr, scopeName, scope?.toString()).toFloat()
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

    private lateinit var appContext: Context
    private lateinit var activity: ComponentActivity
    private var pendingPickerAction: String? = null
    private var openDocuments: (() -> Unit)? = null

    external fun dispatchAction(action: String): Boolean
    external fun dispatchActionJson(action: String): String
    external fun dispatchAndStoreJson(action: String): String
    external fun dispatchChangeJson(action: String, bind: String, valueJson: String): String
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
        this.activity = activity
        appContext = activity.applicationContext
        initAndroid(appContext)
        val filePicker =
            activity.registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
                val action = pendingPickerAction ?: return@registerForActivityResult
                pendingPickerAction = null
                emit(filePickerResultJson(action, uris))
            }
        openDocuments = { filePicker.launch(arrayOf("*/*")) }
        CrepusActions.dispatch = { action -> dispatchHostAction(action) ?: dispatchAndStoreJson(action) }
        CrepusActions.resultSink = { result -> CrepusStateStore.applyResult(result) }
    }

    private fun dispatchHostAction(action: String): String? {
        dispatchNamedHostAction(action)?.let { return it }
        val request = runCatching { JSONObject(action) }.getOrNull() ?: return null
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
            "import_files" -> {
                pendingPickerAction = action
                openDocuments?.invoke() ?: emit(errorJson(action, "file picker unavailable"))
                pendingJson(action)
            }
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

    private fun filePickerResultJson(action: String, uris: List<Uri>): String {
        val files =
            uris.mapNotNull { uri ->
                runCatching {
                    val name = queryDisplayName(uri)
                    val mime = appContext.contentResolver.getType(uri) ?: "application/octet-stream"
                    val file = copyToCache(uri, name, mime)
                    JSONObject()
                        .put("name", name)
                        .put("mimeType", mime)
                        .put("bytes", file.length())
                        .put("filePath", file.absolutePath)
                        .put("importSource", "android-document-picker")
                }.getOrNull()
            }
        return JSONObject()
            .put("ok", true)
            .put("action", action)
            .put("value", JSONObject().put("files", files))
            .toString()
    }

    private fun copyToCache(uri: Uri, name: String, mime: String): File {
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime)
            ?: name.substringAfterLast('.', "bin")
        val file = File.createTempFile("crepus-media-", ".$ext", appContext.cacheDir)
        appContext.contentResolver.openInputStream(uri)?.use { input ->
            file.outputStream().use { output -> input.copyTo(output) }
        } ?: error("media item unavailable")
        return file
    }

    private fun queryDisplayName(uri: Uri): String {
        appContext.contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)
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
