#!/usr/bin/env python3
"""Run actual picker delegate and file-copy code with UIKit delegate stubs on macOS."""
from pathlib import Path
import subprocess
import tempfile

fixtures = Path(__file__).resolve().parent
root = fixtures.parents[2]
source = (root / 'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift').read_text()
delegate = source[source.index('private let pickerImportQueue'):source.index('private func presentFilePicker')]
helpers = source[source.index('private func pickedFileJson'):source.index('private func mediaResultJson')]
helpers = helpers.replace('private func pickedFileJson(', 'private func actualPickedFileJson(', 1)
with tempfile.TemporaryDirectory(prefix='crepus-swift-picker-') as directory:
    out = Path(directory)
    actual = out / 'Actual.swift'
    actual.write_text('import Foundation\nimport UniformTypeIdentifiers\n' + delegate + helpers + (fixtures / 'PickerHarness.swift').read_text())
    binary = out / 'probe'
    subprocess.run(['swiftc', '-swift-version', '5', '-warnings-as-errors', '-parse-as-library', str(actual), '-o', str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=15)
