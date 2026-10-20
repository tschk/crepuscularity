import kotlinx.serialization.json.*
// Labelled FFI model stub: real JSON parsing; no JNI, Rust, or Compose lifecycle.
object Probe {
    val model = mutableMapOf<String, JsonElement>()
    var dispatch: (String, String, String) -> String = { _, path, value ->
        JsonObject(mapOf("bindings" to JsonObject(mapOf(path to Json.parseToJsonElement(value))))).toString()
    }
    fun store(raw: String): Boolean {
        val obj = Json.parseToJsonElement(raw).jsonObject
        if ((obj["ok"] as? JsonPrimitive)?.booleanOrNull == false) return true
        for (key in listOf("value", "bindings")) (obj[key] as? JsonObject)?.forEach { (path, value) -> model[path] = value }
        return true
    }
    var scopedText: (String, String?, String?) -> String = { expr, _, _ -> text(expr) }
    fun text(path: String) = (model[path] as? JsonPrimitive)?.content ?: ""
    fun bool(path: String) = (model[path] as? JsonPrimitive)?.booleanOrNull ?: false
    var scopedNumber: (String, String?, String?) -> Double = { expr, _, _ -> number(expr) }
    fun number(path: String) = (model[path] as? JsonPrimitive)?.doubleOrNull ?: 0.0
}
