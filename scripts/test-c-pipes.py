"""Check C/C++ CLI adapters with quoted paths and fragmented output."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix="crepus-c-pipes-") as directory:
    root = Path(directory)
    fixture = root / "template's view.crepus"
    fixture.write_text('span\n  "Fragmented"\n')
    cli = root / "crepus"
    cli.write_text(
        f"#!{sys.executable}\n"
        "import json,sys,time,os\nfrom pathlib import Path\n"
        "html=sys.argv[1] == 'render'\n"
        "source=sys.argv[2] if html else sys.argv[3]\n"
        "assert Path(source).read_text().startswith('span')\n"
        "assert not html or sys.argv[3:] == ['--ctx', '/dev/null']\n"
        "content='x'*10000 if os.getenv('CREPUS_TEST_LARGE_OUTPUT') else 'Fragmented'\n"
        "text=('<div>'+content+'</div>') if html else json.dumps({'version':7,'root':[{'kind':'text','content':content}]})\n"
        "sys.stdout.write(text[:16]);sys.stdout.flush();time.sleep(0.1)\n"
        "sys.stdout.write(text[16:]);sys.stdout.flush()\n"
    )
    cli.chmod(0o700)
    for binary, supports_large in [(sys.argv[1], False), (sys.argv[2], True)]:
        for large in (False, True):
            result = subprocess.run(
                [str(Path(binary).resolve()), str(fixture)],
                env={**os.environ, "CREPUS_BIN": str(cli), "CREPUS_TEST_LARGE_OUTPUT": "1" if large else ""},
                capture_output=True, text=True, timeout=10,
            )
            expected = 1 if large and not supports_large else 0
            assert result.returncode == expected, f"{binary}: {result.stderr}"
            assert not result.stderr, result.stderr
print("C/C++ filename and fragmented pipe checks passed")
