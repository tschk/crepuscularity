#!/usr/bin/env python3
"""Exercise actual generated Kotlin queue and extracted StateStore/change helper.
Uses already-installed kotlinx JSON jars; Android/Compose/JNI collaborators are stubs.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[3]
FIX = Path(__file__).resolve().parent
source = (ROOT / 'crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt').read_text()
state = source[source.index('object CrepusStateStore {'):source.index('\nobject CrepusRustActions {')]
start = source.index('    internal fun dispatchQueuedChange(')
helper = source[start:source.index('    external fun lastResult()', start)]
imports = '''import kotlinx.serialization.json.*
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableLongStateOf
'''
ffi = '''object CrepusRustActions {
fun storeResultJson(raw: String) = Probe.store(raw)
fun evalText(expr: String, scopeName: String?, scope: String?) = Probe.scopedText(expr, scopeName, scope)
fun evalBool(expr: String, scopeName: String?, scope: String?) = Probe.bool(expr)
fun evalNumber(expr: String, scopeName: String?, scope: String?) = Probe.scopedNumber(expr, scopeName, scope)
fun evalItemsJson(expr: String, scopeName: String?, scope: String?) = "[]"
fun dispatchChangeJson(action: String, bind: String, valueJson: String) = Probe.dispatch(action, bind, valueJson)
'''
manifest = (ROOT / 'crates/crepuscularity-cli/templates/native/android/app/build.gradle.kts').read_text()
version = re.search(r'kotlinx-serialization-json:([^"\s]+)', manifest).group(1)
cp = os.environ.get('CREPUS_KOTLIN_SERIALIZATION_CLASSPATH')
if not cp:
    cache = Path(os.environ.get('GRADLE_USER_HOME', Path.home() / '.gradle')) / 'caches/modules-2/files-2.1/org.jetbrains.kotlinx'
    jars = []
    for name in ['kotlinx-serialization-core-jvm', 'kotlinx-serialization-json-jvm']:
        matches = sorted((cache / name / version).glob(f'*/{name}-{version}.jar'))
        if not matches:
            raise SystemExit(f'Missing installed {name}:{version}. Set CREPUS_KOTLIN_SERIALIZATION_CLASSPATH to existing core/json JVM jars; this probe never downloads dependencies.')
        jars.append(matches[0])
    cp = os.pathsep.join(map(str, jars))
for entry in cp.split(os.pathsep):
    if not Path(entry).is_file():
        raise SystemExit(f'Missing Kotlin serialization classpath file: {entry}')
print(f'Kotlin async probe: template serialization version {version}; classpath {cp}', flush=True)
generator = (ROOT / 'crates/crepuscularity-native/src/codegen/compose.rs').read_text()
match = re.search(r'let source = format!\(\s*("(?:[^"\\]|\\.)*")', generator)
generated = json.loads(match.group(1)).replace('{{', '{').replace('}}', '}')
snapshot = (ROOT / 'crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt').read_text()
with tempfile.TemporaryDirectory(prefix='crepus-kotlin-async-') as directory:
    out = Path(directory)
    for name, text in [('generator', generated), ('snapshot', snapshot)]:
        obj = text[text.index('object CrepusActions {'):]
        obj = obj[:obj.index('\n}\n') + 3]
        actual = out / 'Actual.kt'
        actual.write_text(imports + state + '\n' + ffi + helper + '\n}\n' + obj)
        jar = out / f'{name}.jar'
        subprocess.run(['kotlinc', str(actual), str(FIX/'AsyncCompose.kt'), str(FIX/'AsyncRuntime.kt'), str(FIX/'AsyncHarness.kt'), str(FIX/'Android.kt'), '-cp', cp, '-include-runtime', '-d', str(jar)], check=True)
        subprocess.run(['java', '-cp', str(jar)+':'+cp, 'AsyncHarnessKt'], check=True)
