#!/usr/bin/env python3
"""Run extracted production Swift async edits against mock native responses."""
from pathlib import Path
import subprocess
import sys
import tempfile

fixtures = Path(__file__).resolve().parent
root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='crepus-swift-async-') as directory:
    out = Path(directory)
    subprocess.run([sys.executable, str(fixtures / 'extract-swift-async.py'), str(root), directory], check=True)
    for variant in ['generated', 'snapshot']:
        subprocess.run(['swiftc', '-swift-version', '5', '-warnings-as-errors', '-parse-as-library',
                        str(out / f'{variant}-async.swift'), '-o', str(out / variant)], check=True)
        subprocess.run([str(out / variant)], check=True, timeout=15)
