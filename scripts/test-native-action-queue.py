#!/usr/bin/env python3
"""Compile actual action objects with small platform stubs; no Cargo/dependencies.

Swift removes only unrelated model/keyboard collaborators; queue/perform remain
extracted verbatim. Kotlin stubs Handler's main-post queue and unrelated JSON/FFI
collaborators, not the executor. This does not emulate full native UI lifecycles.
The edit probe also uses the template's already-installed kotlinx JSON jars.
"""
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'scripts/fixtures/native-action-queue'


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def kotlin_object(source):
    obj = source[source.index('object CrepusActions {'):]
    return obj[:obj.index('\n}\n') + 3]


def main():
    for tool in ['swiftc', 'kotlinc', 'java']:
        if shutil.which(tool) is None:
            sys.exit(f'Missing installed {tool}; install/configure it before this gate.')
    with tempfile.TemporaryDirectory(prefix='crepus-native-action-queue-') as directory:
        out = Path(directory)
        run(sys.executable, FIXTURES / 'extract-swift.py', ROOT, out)
        run(sys.executable, FIXTURES / 'extract-swift-routing.py', ROOT, out)
        for variant in ['generated', 'snapshot', 'route']:
            run('swiftc', '-swift-version', '5', '-warnings-as-errors',
                '-parse-as-library', out / f'{variant}.swift', '-o', out / variant)
            run(out / variant)
        run(sys.executable, FIXTURES / 'extract-kotlin-routing.py', ROOT, out)
        source = (ROOT / 'crates/crepuscularity-native/src/codegen/compose.rs').read_text()
        match = re.search(r'let source = format!\(\s*("(?:[^"\\]|\\.)*")', source)
        if match is None:
            raise RuntimeError('Cannot locate Compose generator source literal')
        generated = json.loads(match.group(1)).replace('{{', '{').replace('}}', '}')
        snapshot = (ROOT / 'crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt').read_text()
        for variant, text in [('generator', generated), ('snapshot', snapshot)]:
            actual = out / f'{variant}.kt'
            actual.write_text(kotlin_object(text))
            jar = out / f'{variant}.jar'
            run('kotlinc', actual, FIXTURES / 'Harness.kt', FIXTURES / 'Stubs.kt',
                FIXTURES / 'Android.kt', '-include-runtime', '-d', jar)
            run('java', '-jar', jar)
            routing = out / f'{variant}-routing.jar'
            run('kotlinc', actual, out / 'RoutingSlice.kt', FIXTURES / 'RoutingHarness.kt',
                FIXTURES / 'Stubs.kt', FIXTURES / 'RoutingAndroid.kt',
                '-include-runtime', '-d', routing)
            run('java', '-jar', routing)
    run(sys.executable, FIXTURES / 'test-swift-async.py')
    run(sys.executable, FIXTURES / 'test-swift-scope-json.py')
    run(sys.executable, FIXTURES / 'test-swift-picker.py')
    run(sys.executable, FIXTURES / 'test-kotlin-async.py')
    run(sys.executable, ROOT / 'scripts/test-native-picker-android.py')
    print('PASS native action queues: generated + snapshot Swift/Kotlin; host routing and async edits')


if __name__ == '__main__':
    main()
