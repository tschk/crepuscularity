#!/usr/bin/env python3
"""Inventory cached CE source without Cargo, network access or dependency builds.

This is a lexical source inventory, not a resolved rustdoc API or coverage score.
Cfg alternatives and generated methods must be reviewed separately.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tomllib

TRAITS = {
    "elements/div.rs": ["InteractiveElement", "StatefulInteractiveElement"],
    "styled.rs": ["Styled"],
    "elements/img.rs": ["StyledImage"],
    "input.rs": ["EntityInputHandler"],
    "platform.rs": ["InputHandler"],
    "element.rs": ["Element", "IntoElement", "ParentElement"],
    "elements/animation.rs": ["AnimationExt"],
}


def package(path):
    manifest = tomllib.loads((path / "Cargo.toml").read_text())
    return {
        "name": manifest["package"]["name"],
        "version": manifest["package"]["version"],
        "vcs": json.loads((path / ".cargo_vcs_info.json").read_text()),
        "features": manifest.get("features", {}),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--platform", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = {
        "schema_version": 1,
        "scope": "Lexical explicit trait methods, helper invocations and manifest features; not a whole-crate resolved API denominator",
        "core": package(args.core),
        "platform": package(args.platform),
        "native_access": "crepuscularity_gpui::gpui and ::gpui_platform preserve the original crate namespaces; availability follows dependency features and cfg",
        "validation": "Source inventory only; compilation and platform behavior not established by this file",
        "traits": {},
    }
    repository = Path(__file__).resolve().parent.parent
    macro_source = (repository / "crates/crepuscularity_macros/src/lib.rs").read_text()
    style_source = (repository / "crates/crepuscularity-runtime/src/styler.rs").read_text()
    input_source = (repository / "crates/crepuscularity-runtime/src/text_input.rs").read_text()
    input_impl = re.search(r"(?ms)^impl EntityInputHandler for TextInput \{(.*?)^\}", input_source).group(1)
    input_methods = set(re.findall(r"(?m)^    fn (\w+)", input_impl))
    runtime_element_source = (repository / "crates/crepuscularity-runtime/src/runtime_element.rs").read_text()
    runtime_metadata = set(re.findall(r"\.(aria_[a-z_]+|role)\s*\(", runtime_element_source))
    compiled_metadata = set(re.findall(r'"(aria_[a-z_]+|role)"', macro_source))
    macro_calls = set(re.findall(r"\.([a-z_][a-z_0-9]*)\s*\(", macro_source))
    style_calls = set(re.findall(r"\.([a-z_][a-z_0-9]*)\s*\(", style_source))
    common_events = {
        "on_click": "@click", "on_key_down": "@keydown", "on_key_up": "@keyup",
        "on_hover": "@hover", "on_mouse_down": "@mousedown (left button)",
        "on_mouse_up": "@mouseup (left button)", "on_mouse_move": "@mousemove",
        "on_mouse_exit": "@mouseexit", "on_scroll_wheel": "@scroll",
        "on_pinch": "@pinch", "on_modifiers_changed": "@modifierschanged",
    }
    mappings = []
    for filename, traits in TRAITS.items():
        text = (args.core / "src" / filename).read_text()
        for trait in traits:
            match = re.search(r"(?ms)^pub trait " + trait + r"\b[^\{]*\{(.*?)^\}", text)
            if not match:
                raise ValueError(f"Trait {trait} missing from {filename}")
            body = match.group(1)
            methods = re.findall(r"(?m)^    (?:async )?fn ([a-zA-Z_][a-zA-Z_0-9]*)", body)
            helpers = re.findall(r"(?m)^    ([a-zA-Z_][a-zA-Z_0-9:]*)!\s*\(", body)
            result["traits"][trait] = {
                "source": filename,
                "source_sha256": hashlib.sha256(text.encode()).hexdigest(),
                "declarations": len(methods),
                "unique_methods": sorted(set(methods)),
                "generated_helper_invocations": helpers,
                "native_access": "Original trait via gpui namespace; runtime factories may use typed APIs",
                "template_mapping": "See docs/gpui-ce-coverage.md; absence of shorthand is not implemented parity",
            }
            for method in sorted(set(methods)):
                compiled = runtime = "no declarative mapping identified; native implementation required"
                syntax = None
                if trait == "Styled":
                    if method in macro_calls:
                        compiled = "referenced by utility mapper; utility subset only, not general argument parity"
                    if method in style_calls:
                        runtime = "referenced by utility mapper; utility subset only, not general argument parity"
                elif trait == "EntityInputHandler":
                    if method in input_methods:
                        runtime = "implemented for built-in single-line input; behavior unvalidated"
                    compiled = "no built-in input lowering; retained entity/native expression required"
                elif trait == "InputHandler":
                    compiled = "native input entity and upstream bridge required; no built-in input lowering"
                    if method in input_methods:
                        runtime = "upstream ElementInputHandler delegates to built-in TextInput; platform delivery unverified"
                    elif method == "element_bounds":
                        runtime = "upstream ElementInputHandler returns registered InputText bounds; platform delivery unverified"
                    elif method == "prefers_ime_for_printable_keys":
                        runtime = "upstream ElementInputHandler delegates to accepts_text_input"
                    elif method == "apple_press_and_hold_enabled":
                        runtime = "inherits upstream true default; no template control or platform acceptance"
                elif trait == "ParentElement" and method in ("child", "children"):
                    compiled = runtime = "template node hierarchy lowered through this infrastructure; not arbitrary builder arguments"
                elif trait == "IntoElement":
                    compiled = runtime = "renderer/native-expression infrastructure; not a separately configurable template API"
                elif trait == "AnimationExt":
                    runtime = "runtime animate subset uses this method; scale rejected and easing/duration validation incomplete"
                    compiled = "animate attributes rejected; native builder/expression required"
                elif trait == "Element":
                    if method in ("a11y_role", "write_a11y_info", "id"):
                        compiled = "semantic wrapper/identity lowering for supported built-ins; custom implementation remains native"
                    if method in ("a11y_role", "a11y_synthetic_children", "write_a11y_info", "id", "request_layout", "prepaint", "paint"):
                        runtime = "built-in TextInput implements this lifecycle/semantics method; custom behavior remains native and unverified"
                elif trait in ("InteractiveElement", "StatefulInteractiveElement"):
                    if method in common_events:
                        compiled = runtime = "declared event shorthand; dispatch tests pending"
                        syntax = common_events[method]
                    elif method == "on_aux_click":
                        compiled, syntax = "declared event shorthand; dispatch tests pending", "@auxclick"
                    elif method == "on_drop":
                        runtime, syntax = "external file paths only; arbitrary typed drops require native code", "@drop"
                    elif method.startswith("aria_") or method == "role":
                        if method in compiled_metadata:
                            compiled = "typed metadata binding; see generate_binding"
                        if method in runtime_metadata:
                            runtime = "metadata binding (named role subset; checked maps toggled); see runtime_element"
                    elif method in ("id", "hover", "focus", "active", "focusable", "overflow_scroll", "overflow_x_scroll", "overflow_y_scroll"):
                        compiled = runtime = "declared binding/style subset; source-only validation"
                mappings.append({"trait": trait, "method": method, "compiled": compiled,
                                 "runtime": runtime, "syntax": syntax,
                                 "acceptance": "not executed; native access is not declarative parity"})
    result["method_mappings"] = mappings
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({name: len(item["unique_methods"]) for name, item in result["traits"].items()}))


if __name__ == "__main__":
    main()
