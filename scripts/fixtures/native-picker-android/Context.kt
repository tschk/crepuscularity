package android.content
import android.net.Uri
import java.io.File
import java.io.InputStream
open class Context(val contentResolver: ContentResolver, val cacheDir: File) {
 val applicationContext: Context get() = this
}
abstract class ContentResolver {
 abstract fun getType(uri: Uri): String?
 abstract fun openInputStream(uri: Uri): InputStream?
 abstract fun query(uri: Uri, columns: Array<String>, selection: String?, args: Array<String>?, order: String?): Cursor?
}
class Cursor(private val name: String): AutoCloseable {
 fun moveToFirst() = true
 fun getColumnIndex(column: String) = 0
 fun getString(index: Int) = name
 override fun close() {}
}
