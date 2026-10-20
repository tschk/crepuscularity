package androidx.activity
import android.content.Context
import android.net.Uri
import androidx.activity.result.contract.ActivityResultContracts
class Launcher { fun launch(types: Array<String>) {} }
class ComponentActivity(val applicationContext: Context) {
 private var callback: ((List<Uri>) -> Unit)? = null
 fun registerForActivityResult(contract: ActivityResultContracts.OpenMultipleDocuments, callback: (List<Uri>) -> Unit): Launcher {
  this.callback = callback
  return Launcher()
 }
 fun deliver(uris: List<Uri>) { callback!!(uris) }
}
