"""Offline palette generation and cross-target parity checks."""
import re
import runpy
import subprocess
import tempfile
from pathlib import Path

script = Path(__file__).with_name("gen-tailwind-colors.py")
module = runpy.run_path(str(script), run_name="palette_test")
convert = module["oklch_to_srgb8"]
assert convert(0, 0, 0) == (0, 0, 0)
assert convert(1, 0, 0) == (255, 255, 255)
assert convert(0.5, 0, 0) == (99, 99, 99)
assert convert(0.6279553606, 0.2576833077, 29.233885) == (255, 0, 0)
assert module["OUT"] == "crates/crepuscularity-colors/src/palette.rs"
with tempfile.TemporaryDirectory(prefix="crepus-palette-test-") as directory:
    theme = Path(directory) / "theme.css"
    output = Path(directory) / "palette.rs"
    theme.write_text("--color-black: #000;\n--color-white: #fff;\n--color-test-500: oklch(50% 0 0);\n")
    subprocess.run(["python3", str(script), "--theme", str(theme), str(output)], check=True)
    generated = output.read_text()
    assert '"test-500" => "#636363"' in generated
    assert '"black" => "#000"' in generated
    assert '"white" => "#fff"' in generated
    assert "lookup_color_u32" not in generated
    harness = Path(directory) / "check.rs"
    harness.write_text('#[path = "palette.rs"] mod palette;\nfn main() { assert_eq!(palette::lookup_named_color("test-500"), Some("#636363")); assert_eq!(palette::lookup_named_color("unknown"), None); }\n')
    binary = Path(directory) / "check"
    subprocess.run(["rustc", "--edition", "2021", "-D", "warnings", str(harness), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)

with tempfile.TemporaryDirectory(prefix="crepus-palette-invalid-") as directory:
    theme = Path(directory) / "theme.css"
    output = Path(directory) / "palette.rs"
    for css, message in [
        ("", "theme contains no supported colors"),
        ("upstream unavailable", "theme contains no supported colors"),
        ("--color-white: #fff;\n--color-white: #000;", "duplicate palette color"),
        ("--color-test-500: oklch(50% 0 0);\n--color-test-500: oklch(100% 0 0);", "duplicate palette color"),
        ("--color-white: #fff;\n--color-invalid-500: oklch(nan 0 0);", "invalid palette OKLCH color"),
        ("--color-white: #fff;\n--color-invalid-500: oklch(50% inf 0);", "invalid palette OKLCH color"),
        ("--color-white: #fff;\n--color-invalid-500: oklch(50% 0 0 extra);", "invalid palette OKLCH color"),
        ("--color-white: #fff;\n--color-invalid-500: oklch(50% 0);", "invalid palette OKLCH color"),
        ("--color-white: #fff;\n--color-invalid-500: oklch();", "invalid palette OKLCH color"),
        ("--color-white: #fff;\n--color-invalid: #;", "unsupported palette hex color"),
        ("--color-white: #fff;\n--color-invalid: #1234;", "unsupported palette hex color"),
        ("--color-white: #fff;\n--color-invalid: #12345678;", "unsupported palette hex color"),
    ]:
        theme.write_text(css)
        output.write_text("existing palette\n")
        result = subprocess.run(["python3", str(script), "--theme", str(theme), str(output)], capture_output=True, text=True)
        assert result.returncode != 0
        assert message in result.stderr
        assert output.read_text() == "existing palette\n"
with tempfile.TemporaryDirectory(prefix="crepus-palette-arguments-") as directory:
    output = Path(directory) / "palette.rs"
    output.write_text("existing palette\n")
    for args in [["--theme"], ["--theme", "--typo"], ["--typo"]]:
        result = subprocess.run(["python3", str(script), str(output), *args], capture_output=True, text=True)
        assert result.returncode == 2
        assert "error:" in result.stderr
        assert output.read_text() == "existing palette\n"
# Compare the Flutter parser's supported subset with the generated shared table.
repository = script.parent.parent
shared = dict(re.findall(
    r'"([^"\n]+)" => "(#[0-9a-f]+)"',
    (repository / "crates/crepuscularity-colors/src/palette.rs").read_text(encoding="utf-8"),
))
flutter_source = (repository / "plugins/crepuscularity-flutter/lib/src/crepus_parser.dart").read_text(encoding="utf-8")
flutter_table = flutter_source.split("const Map<String, String> _palette = {", 1)[1].split("};", 1)[0]
flutter = dict(re.findall(r"'([^'\n]+)': '(#[0-9a-f]+)'", flutter_table))
common_names = shared.keys() & flutter.keys()
assert "red-500" in common_names, "shared/Flutter palette entries were not found"
for name in common_names:
    expected = shared[name][1:]
    if len(expected) == 3:
        expected = "".join(channel * 2 for channel in expected)
    assert int(flutter[name][1:], 16) == int(expected, 16), f"Flutter palette drift: {name}"
print("PASS: palette conversion, offline generation, and Flutter parity")
