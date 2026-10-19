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


def render_ir(path: str | Path, context: dict[str, Any] | None = None, allowed_dir: str | Path | None = None) -> ViewIr:
    template_path = Path(path)
    command_path = str(path)
    if context is not None or allowed_dir is not None:
        template_path = template_path.resolve()
        resolved_allowed = Path(allowed_dir).resolve() if allowed_dir is not None else Path.cwd().resolve()
        if not template_path.is_relative_to(resolved_allowed):
            raise ValueError("Path traversal detected")
        command_path = str(template_path)

    args = [_crepus_bin(), "native", "ir", command_path]
    input_data = None
    if context is not None:
        source = template_path.read_text()
        payload = {"template": source, "context": context, "baseDir": str(template_path.parent)}
        args = [_crepus_bin(), "native", "ir", "--stdin-json"]
        input_data = json.dumps(payload)
    proc = subprocess.run(args, input=input_data, text=True, capture_output=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip())
    data = json.loads(proc.stdout)
    return ViewIr(version=data["version"], root=data["root"])


def render_html(path: str | Path, context: dict[str, Any] | None = None, allowed_dir: str | Path | None = None) -> str:
    return "".join(_render_node(node) for node in render_ir(path, context, allowed_dir).root)


def _render_node(node: dict[str, Any]) -> str:
    kind = node.get("kind")
    if kind == "text":
        return escape(str(node.get("content", "")))
    if kind == "stack":
        axis = escape(str(node.get("axis", "column")))
        children = "".join(_render_node(child) for child in node.get("children", []))
        return f'<div data-crepus-kind="stack" data-axis="{axis}">{children}</div>'
    if kind == "scroll":
        axis = escape(str(node.get("axis", "column")))
        children = "".join(_render_node(child) for child in node.get("children", []))
        return f'<div data-crepus-kind="scroll" data-axis="{axis}">{children}</div>'
    if kind == "button":
        label = escape(str(node.get("label", "")))
        on_click = escape(str(node.get("onClick", "")))
        attr = f' data-onclick="{on_click}"' if on_click else ""
        return f"<button{attr}>{label}</button>"
    if kind == "image":
        src = escape(str(node.get("src", "")), quote=True)
        alt = escape(str(node.get("alt", "")), quote=True)
        return f'<img src="{src}" alt="{alt}">'
    if kind == "slotRotate":
        phrase = escape(str((node.get("phrases") or [""])[0]))
        return f'<span data-crepus-kind="slotRotate">{phrase}</span>'
    if kind == "input":
        placeholder = escape(str(node.get("placeholder", "")), quote=True)
        bind = escape(str(node.get("bind", "")), quote=True)
        if node.get("multiline"):
            return f'<textarea data-bind="{bind}" placeholder="{placeholder}"></textarea>'
        return f'<input data-bind="{bind}" placeholder="{placeholder}">'
    if kind == "picker":
        bind = escape(str(node.get("bind", "")), quote=True)
        options = "".join(
            f'<option value="{escape(str(opt.get("value", "")), quote=True)}">{escape(str(opt.get("label", "")))}</option>'
            for opt in node.get("options", [])
        )
        return f'<select data-bind="{bind}">{options}</select>'
    return ""
