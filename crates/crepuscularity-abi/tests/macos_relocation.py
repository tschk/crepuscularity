"""Run with an already-built ABI dylib path; requires macOS and cc."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if sys.platform != "darwin":
    raise SystemExit("This relocation check requires macOS")

crate = Path(__file__).resolve().parents[1]
library = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="crepus-abi-relocation-") as directory:
    destination = Path(directory).resolve()
    copied = destination / "libcrepuscularity_abi.dylib"
    shutil.copy2(library, copied)
    host = destination / "smoke"
    subprocess.run([
        "cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
        "-I", str(crate / "include"), str(crate / "tests/smoke.c"),
        "-L", str(destination), "-lcrepuscularity_abi",
        "-Wl,-rpath,@executable_path", "-o", str(host),
    ], check=True)
    linked = subprocess.check_output(["otool", "-L", str(host)], text=True)
    assert "@rpath/libcrepuscularity_abi.dylib" in linked, linked
    loaded = subprocess.run(
        [str(host)], env={**os.environ, "DYLD_PRINT_LIBRARIES": "1"},
        capture_output=True, text=True, check=True,
    )
    assert str(copied) in loaded.stderr, loaded.stderr
    assert str(library) not in loaded.stderr, loaded.stderr
print("macOS ABI relocation smoke passed")
