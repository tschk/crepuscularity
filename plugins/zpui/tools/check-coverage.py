#!/usr/bin/env python3
"""Keep the finish goal synchronized with the authoritative View IR inventory."""
import json
from pathlib import Path
import re


def camel(name):
    first, *rest = name.split("_")
    return first + "".join(word.title() for word in rest)


def main():
    plugin = Path(__file__).resolve().parents[1]
    source_path = plugin.parents[1] / "crates/crepuscularity-native/src/ir.rs"
    snapshot = plugin / "tests/native-ir-v7.rs"
    if source_path.exists() and source_path.read_bytes() != snapshot.read_bytes():
        raise SystemExit("Bundled IR snapshot differs from authoritative repository source")
    source = (source_path if source_path.exists() else snapshot).read_text()
    goal = json.loads((plugin / "coverage.json").read_text())
    style = source.split("pub struct ViewStyle {", 1)[1].split("\n}", 1)[0]
    node = source.split("pub enum ViewNode {", 1)[1].split("\n}", 1)[0]
    styles = {camel(name) for name in re.findall(r"\bpub (\w+):", style)}
    nodes = set(re.findall(r'#\[serde\(rename = "([^"]+)"\)\]', node))
    for category, expected in (("styles", styles), ("nodes", nodes)):
        recorded = set(goal[category])
        if recorded != expected:
            raise SystemExit(f"{category} inventory mismatch: missing={sorted(expected-recorded)}, extra={sorted(recorded-expected)}")
        if goal["completion"][category] != len(expected):
            raise SystemExit(f"{category} completion target changed: {len(expected)}")
    if goal["status"] == "complete":
        gaps = [f"{category}.{name}" for category in ("nodes", "styles") for name, entry in goal[category].items() if entry["status"] != "implemented"]
        gaps += [f"{category}.{name}" for category in ("events", "runtime", "validation") for name, status in goal[category].items() if status != "implemented"]
        if gaps or goal["remaining"]:
            raise SystemExit(f"Cannot mark coverage complete with remaining work: {gaps}")
    counts = {category: sum(entry["status"] == "implemented" for entry in goal[category].values()) for category in ("nodes", "styles")}
    print(f"Goal inventory verified: {len(nodes)} nodes, {len(styles)} styles; implementations={counts}; status={goal['status']}")


if __name__ == "__main__":
    main()
