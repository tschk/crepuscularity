class JsonElement
object CrepusStateStore {
    fun applyResult(raw: String) {}
    fun snapshotDraft(raw: String) = JsonElement()
    fun beginDraft(path: String, value: JsonElement): Long = 0
    fun clearDraft(path: String, sequence: Long) {}
}
object CrepusRustActions {
    // shortcut: transport uses a marker; real JSON serialization has separate bridge probes.
    fun queuedChangeError(error: Throwable) = "error:${error.message}"
    var queuedDispatch: (String, String, String) -> String = { _, _, _ -> "{}" }
    fun dispatchQueuedChange(action: String, bind: String, value: String) = queuedDispatch(action, bind, value)
}
