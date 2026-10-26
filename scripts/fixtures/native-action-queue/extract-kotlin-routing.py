from pathlib import Path
import re
import sys
out = Path(sys.argv[2])
path = (Path(sys.argv[1]) / "crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt")
s = path.read_text()
start = s.index("    private fun dispatchHostActionOnMain(")
end = s.index("    private fun dispatchHostAction(", start)
helper = s[start:end]
installation = re.search(r"        CrepusActions.dispatch = .*", s).group(0)
(out / "RoutingSlice.kt").write_text("""object RoutingSlice {
    var host: (String) -> String? = { null }
    var rust: (String) -> String = { it }
    private fun dispatchHostAction(action: String) = host(action)
    private fun dispatchAndStoreJson(action: String) = rust(action)
    fun install() {
""" + installation + "\n    }\n" + helper + "}\n")
