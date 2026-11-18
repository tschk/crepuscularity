# Aurorality (SwiftUI Engine)

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Native shells](native.md)

Aurorality is the SwiftUI-facing engine that consumes `.crepus` templates via `swiftgen` and renders them as native SwiftUI views in macOS/iOS host apps.

## What Aurorality Adds

- Native SwiftUI output from `.crepus` templates
- Semantic SwiftUI-style tags (`navigationstack`, `sidebar`, `item`, `menu`, `label`, `sf-*`, etc.)
- Utility-first layout fallback (`div flex`, `div flex-col`, spacing, sizing classes)
- Runtime bridge hooks (`eventSink`) for app state and transport actions
- Host-app integration with Rust/UniFFI backends

## Authoring Model

Aurorality supports a hybrid model:

- **Semantic components** for explicit native intent
- **Utility classes** for fast layout and spacing

```text
navigationsplitview
  sidebar
    section header="Conversations"
      button @click="newConversation"
        sf-plus
      for conv in {conversations}
        item @click="selectConversation:{conv.id}"
          span "{conv.title}"
  detail
    vstack gap-2 p-3
      textfield bind=draft placeholder="Message"
      button button-prominent @click="send"
        "Send"
```

## Supported Semantic Tags (swiftgen path)

- Navigation: `navigationstack`, `navigationsplitview`, `sidebar`, `detail`, `content`
- Layout: `vstack`, `hstack`, `zstack`, `group`, `form`, `section`, `list`, `item`, `scrollview`, `divider`, `spacer`
- Controls: `button`, `toggle`, `picker`, `textfield`, `input`, `securefield`, `menu`
- Content: `label`, `image`, `sf`, `sf-*`, `span`, `p`
- **macOS chrome:** root-level `menubar` emits a companion `<ViewName>Commands: Commands` type (menus, shortcuts). Root `dockbadge bind=<field>` syncs `NSApplication.shared.dockTile.badgeLabel`. `notification` fires `eventSink` on appear (event-only).
- **Messaging widgets:** `avatar bind=<seed>`, `badge` (inline text), `messagebubble bind=<msg>`, `typingindicator label="…"` (or `{typingLine}`).
- **Composer:** `textfield` supports utility classes including **`pill`** for capsule chrome.

See [DSL reference](dsl.md) for the living mapping details.

## Hot reload modes (`aurorality dev`)

The CLI can watch `.crepus` and optionally:

1. Push updated **IR** JSON over the WebSocket (default).
2. Re-run **`swiftgen`** on save (`--swiftgen-view`, `--swiftgen-out`, …) and broadcast a **`SwiftgenStatus`** envelope (ok/errors/output path).
3. Emit **`DevHello`** when a client connects (session id, watch roots, swiftgen config, whether IR is enabled).

Swift hosts wire [`HotReloadClient`](https://github.com/softwarecorporation/aurorality) → **`HotReloadBus`** → HUD / optional live IR preview (`AurorDevOverlay`). Pure-swiftgen workflows may pass **`--no-ir`**.

See also [IDE extensions](ide-extensions.md) for editor-facing notes.

## Integration Notes

- `swiftgen` emits a generated SwiftUI view struct (for example `HyperChatGeneratedView`).
- Host apps pass a typed context object plus `eventSink`.
- Rust plugins/backends can be wired through UniFFI and invoked from host-side state models.

## Related Projects

- Aurorality repo: [softwarecorporation/aurorality](https://github.com/softwarecorporation/aurorality)
- Native shell examples in this repo: [native.md](native.md)
