#!/usr/bin/env python3
"""Typecheck actual CLI output for nested/shadowed native Swift loop scopes.

Requires an already rebuilt CLI; never invokes Cargo. Uses full real host template,
not scope-resolution or UI stubs. Runtime expression evaluation is a separate gate.
"""
import argparse
from pathlib import Path
import re
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--cli', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
fixtures = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix='crepus-swift-lexical-') as directory:
    out = Path(directory)
    subprocess.run([str(args.cli.resolve()), 'native', 'codegen', str(fixtures / 'SwiftLexicalScopes.crepus'),
                    '--platform', 'swiftui', '--out', str(out), '--view-name', 'LexicalScopeView'], check=True)
    source = out / 'LexicalScopeView.swift'
    text = source.read_text()
    variables = re.findall(r'ForEach\([^\n]+\) \{ ([^,\s]+), ([^\s]+) in', text)
    assert len(variables) == 3, text
    items = [variable for _, variable in variables]
    assert len(set(items)) == 3, f'shadowed loop variable reused: {items}'
    assert len({index for index, _ in variables}) == 3, variables
    assert '@crepus:lexical' in text, 'compound nested expression lacks lexical protocol'
    assert text.count('model.draftText(path:') == 4, text
    assert '`__crepusItem_67726f7570__crepusPath` + ".name"' in text, 'outer binding path absent'
    assert '`__crepusItem_6368696c64__crepusPath` + ".name"' in text, 'inner binding path absent'
    subprocess.run(['swiftc', '-swift-version', '5', '-warnings-as-errors', '-typecheck',
                    str(root / 'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift'),
                    str(source)], check=True)
    print('PASS actual CLI Swift nested/outer/shadow alias paths and full host typecheck')
