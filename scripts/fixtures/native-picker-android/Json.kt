package org.json
// Transport collaborator only: assertions inspect the exact constructed object.
// Queue/lifecycle/provider-copy logic is extracted from production, not mocked.
class JSONObject {
 val fields = linkedMapOf<String, Any?>()
 fun put(key: String, value: Any?): JSONObject { fields[key] = value; return this }
 override fun toString(): String {
  val raw = fields.toString()
  synchronized(objects) { objects[raw] = this }
  return raw
 }
 companion object {
  private val objects = mutableMapOf<String, JSONObject>()
  fun lookup(raw: String): JSONObject = synchronized(objects) { objects[raw]!! }
 }
}
class JSONArray(val items: List<Any?>)
