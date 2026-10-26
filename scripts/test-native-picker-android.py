#!/usr/bin/env python3
"""Compile actual Android picker queue/lifecycle/copy helpers with JVM stubs.

No Cargo, Android device, network, new dependencies or JNI backend. JSON and
Android framework collaborators are test stubs; production methods are extracted.
"""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
fixtures = root / 'scripts/fixtures/native-picker-android'
source = (root / 'crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt').read_text()
fields = source[source.index('    private var installedActivity'):source.index('    external fun dispatchAction(')]
install = source[source.index('    fun install('):source.index('    private fun dispatchHostActionOnMain(')]
named = source[source.index('    private fun dispatchNamedHostAction('):source.index('    private fun emit(')]
helpers = source[source.index('    private fun pendingJson('):source.index('\n}\n\nobject CrepusActionState')]
actual = '''import android.content.Context
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import android.webkit.MimeTypeMap
import java.io.File
import org.json.JSONObject
import org.json.JSONArray
object CrepusActions {
 var dispatch: (String) -> String = { it }
 var resultSink: (String) -> Unit = {}
}
object CrepusStateStore { fun applyResult(raw: String) {} }
object PickerUnderTest {
''' + fields + install + named + helpers + '''
 // Unrelated JNI/action-router collaborators only; no picker algorithm stub.
 private fun initAndroid(context: Context) {}
 private fun dispatchHostActionOnMain(action: String): String? = null
 private fun dispatchAndStoreJson(action: String) = action
 fun request(action: String) = dispatchNamedHostAction(action)
}
'''
activity = (root / 'crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt').read_text()
assert 'if (CrepusRustActions.detach(this))' in activity
assert 'private lateinit var activity:' not in source
with tempfile.TemporaryDirectory(prefix='crepus-native-picker-android-') as directory:
    out = Path(directory)
    (out / 'Actual.kt').write_text(actual)
    jar = out / 'probe.jar'
    subprocess.run(['kotlinc', '-Werror', str(out / 'Actual.kt'), *map(str, fixtures.glob('*.kt')),
                    '-include-runtime', '-d', str(jar)], check=True)
    subprocess.run(['java', '-jar', str(jar)], check=True, timeout=20)
