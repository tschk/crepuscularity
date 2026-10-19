from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable
from html import escape


@dataclass
class ViewIr:
    version: int
    root: list[dict[str, Any]]


EventPayload = dict[str, Any]
EventHandler = Callable[[EventPayload, "ViewSession"], None]


class ViewSession:
    def __init__(self, path: str | Path, context: dict[str, Any] | None = None, allowed_dir: str | Path | None = None) -> None:
        self.path = Path(path)
        self.context = dict(context or {})
        self.allowed_dir = allowed_dir
        self._handlers: dict[str, EventHandler] = {}

    def on(self, handler: str, callback: EventHandler) -> "ViewSession":
        self._handlers[handler] = callback
        return self

    def render_ir(self) -> ViewIr:
        return render_ir(self.path, self.context, self.allowed_dir)

    def render_html(self) -> str:
        return "".join(_render_node(node) for node in self.render_ir().root)

    def dispatch(self, event: str | EventPayload) -> ViewIr:
        payload = {"handler": event} if isinstance(event, str) else event
        handler = str(payload.get("handler", ""))
        if handler.startswith("bind:"):
            parts = handler.removeprefix("bind:").split(":", 1)
            # ponytail: block security-sensitive keys only
            if len(parts) == 2 and parts[0] not in _BIND_BLOCKLIST:
                self.context[parts[0]] = parts[1]
        callback = self._handlers.get(handler)
        if callback is not None:
            callback(payload, self)
        return self.render_ir()


def _crepus_bin() -> str:
    bin_path = os.environ.get("CREPUS_BIN", "crepus")
    name = os.path.basename(bin_path)
    if name not in ("crepus", "crepus.exe"):
        raise ValueError(f"Invalid CREPUS_BIN: binary name must be 'crepus' or 'crepus.exe', got {name!r}")
    if bin_path != name and not os.path.isabs(bin_path):
        raise ValueError("Invalid CREPUS_BIN: must be an absolute path or a simple binary name")
    return bin_path


_BIND_BLOCKLIST = frozenset({"baseDir", "_"})  # ponytail: block security-sensitive keys only
_HTML_INPUT_TYPES = frozenset({
    "text", "search", "tel", "url", "email", "password", "number", "date",
    "datetime-local", "month", "time", "week", "color", "hidden",
})


def render_ir(path: str | Path, context: dict[str, Any] | None = None, allowed_dir: str | Path | None = None) -> ViewIr:
    args = [_crepus_bin(), "native", "ir", str(path)]
    input_data = None
    if context is not None or allowed_dir is not None:
        resolved_path = Path(path).resolve()
        resolved_allowed = Path(allowed_dir).resolve() if allowed_dir is not None else Path.cwd().resolve()
        if not resolved_path.is_relative_to(resolved_allowed):
            raise ValueError("Path traversal detected")
    if context is not None:
        source = resolved_path.read_bytes().decode("utf-8")
        payload = {"template": source, "context": context, "filename": str(path), "baseDir": str(Path(path).parent)}
        args = [_crepus_bin(), "native", "ir", "--stdin-json"]
        input_data = json.dumps(payload).encode("utf-8")
    proc = subprocess.run(args, input=input_data, capture_output=True, check=False)
    stderr = proc.stderr.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
    if proc.returncode != 0:
        raise RuntimeError(stderr.strip())
    data = json.loads(proc.stdout.decode("utf-8"))
    return ViewIr(version=data["version"], root=data["root"])


def render_html(path: str | Path, context: dict[str, Any] | None = None, allowed_dir: str | Path | None = None) -> str:
    return "".join(_render_node(node) for node in render_ir(path, context, allowed_dir).root)


def _safe_link_href(value: str) -> bool:
    # Match browser scheme parsing; entity literals are escaped by the caller.
    value = value.lstrip("".join(chr(i) for i in range(33)))
    scheme, colon, _ = value.translate(str.maketrans("", "", "\t\r\n")).partition(":")
    scheme = scheme.translate(str.maketrans(
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz"
    ))
    return not colon or scheme not in ("javascript", "vbscript", "data")


def _escape_attr(value: str) -> str:
    return escape(value, quote=True).replace("\r", "&#13;")


def _render_node(node: dict[str, Any]) -> str:
    kind = node.get("kind")
    if kind == "text":
        return escape(str(node.get("content", "")))
    if kind == "stack":
        axis = _escape_attr(str(node.get("axis", "column")))
        children = "".join(_render_node(child) for child in node.get("children", []))
        return f'<div data-crepus-kind="stack" data-axis="{axis}">{children}</div>'
    if kind == "scroll":
        axis = _escape_attr(str(node.get("axis", "column")))
        children = "".join(_render_node(child) for child in node.get("children", []))
        return f'<div data-crepus-kind="scroll" data-axis="{axis}">{children}</div>'
    if kind == "button":
        label = escape(str(node.get("label", "")))
        on_click = _escape_attr(str(node.get("onClick", "")))
        attr = f' data-onclick="{on_click}"' if on_click else ""
        return f"<button{attr}>{label}</button>"
    if kind == "link":
        href = str(node.get("href", ""))
        attrs = f' href="{_escape_attr(href)}"' if _safe_link_href(href) else ""
        for name in ("target", "rel"):
            if node.get(name) is not None:
                attrs += f' {name}="{_escape_attr(str(node[name]))}"'
        children = "".join(_render_node(child) for child in node.get("children", []))
        return f"<a{attrs}>{children}</a>"
    if kind == "image":
        src = _escape_attr(str(node.get("src", "")))
        alt = _escape_attr(str(node.get("alt", "")))
        return f'<img src="{src}" alt="{alt}">'
    if kind == "slotRotate":
        phrase = escape(str((node.get("phrases") or [""])[0]))
        return f'<span data-crepus-kind="slotRotate">{phrase}</span>'
    if kind == "input":
        placeholder = _escape_attr(str(node.get("placeholder", "")))
        bind = _escape_attr(str(node.get("bind", "")))
        style = node.get("style") or {}
        value = style.get("formValue") if not node.get("bind") else None
        metadata = ""
        if isinstance(style.get("formName"), str):
            metadata += f' name="{_escape_attr(style["formName"])}"'
        if style.get("required") is True:
            metadata += " required"
        if node.get("multiline"):
            text = escape(str(value), quote=True).replace("\r", "&#13;") if value is not None else ""
            # HTML discards one initial LF in textarea content; CR must avoid newline normalization.
            if text.startswith("\n"):
                text = "\n" + text
            return f'<textarea data-bind="{bind}" placeholder="{placeholder}"{metadata}>{text}</textarea>'
        input_type = "password" if node.get("secure") else style.get("inputType")
        type_attr = f' type="{input_type}"' if isinstance(input_type, str) and input_type in _HTML_INPUT_TYPES else ""
        initial = f' value="{_escape_attr(str(value))}"' if value is not None else ""
        return f'<input{type_attr} data-bind="{bind}" placeholder="{placeholder}"{metadata}{initial}>'
    if kind == "picker":
        bind = _escape_attr(str(node.get("bind", "")))
        options = "".join(
            f'<option value="{_escape_attr(str(opt.get("value", "")))}">{escape(str(opt.get("label", "")))}</option>'
            for opt in node.get("options", [])
        )
        return f'<select data-bind="{bind}">{options}</select>'
    return ""
