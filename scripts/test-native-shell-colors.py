"""Compile each iOS shell's actual hex decoder without a SwiftUI SDK."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
STUB = '''import Foundation
struct Color {
    enum Space { case sRGB }
    var r: Double; var g: Double; var b: Double; var a: Double
    init(_ space: Space, red: Double, green: Double, blue: Double, opacity: Double) {
        r = red; g = green; b = blue; a = opacity
    }
}
'''
CHECK = '''
guard let red = Color(hex: "#ff000080"), let opaque = Color(hex: "#ff0000") else {
    fatalError("valid color rejected")
}
assert(red.r == 1 && red.g == 0 && red.b == 0 && red.a == 128.0 / 255)
assert(opaque.r == 1 && opaque.g == 0 && opaque.b == 0 && opaque.a == 1)
for malformed in ["#ff000z", "#12345g", "#ff000080junk", "#+12345", "##ff0000"] {
    assert(Color(hex: malformed) == nil, "malformed color accepted: \\(malformed)")
}
'''
with tempfile.TemporaryDirectory(prefix="crepus-shell-colors-") as temporary:
    source = Path(temporary) / "check.swift"
    binary = Path(temporary) / "check"
    for relative in ["crates/crepuscularity-cli/assets/ios/ViewIrTreeView.swift",
                     "examples/native-shells/ios/Sources/NativeShell/ViewIrTreeView.swift"]:
        text = (ROOT / relative).read_text()
        decoder = text[text.index("extension Color {"):text.index("/// Loads bundled fixture")]
        source.write_text(STUB + decoder + CHECK)
        subprocess.run(["swiftc", "-warnings-as-errors", str(source), "-o", str(binary)], check=True)
        subprocess.run([str(binary)], check=True)
        print(f"{relative}: color decoder passed")
