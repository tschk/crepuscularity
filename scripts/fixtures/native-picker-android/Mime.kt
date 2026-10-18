package android.webkit
class MimeTypeMap {
 fun getExtensionFromMimeType(mime: String): String? = "bin"
 companion object { fun getSingleton() = MimeTypeMap() }
}
