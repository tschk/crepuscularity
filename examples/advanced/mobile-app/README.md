# Mobile App Example

A cross-platform task tracker built with `.crepus` templates, **codegen'd native views**, and a **Rust action dispatch backend**.

## How it works

```
main.crepus  ──→  crepus native codegen  ──→  SwiftUI / Compose views
                        ↕
               Rust action dispatch (staticlib)
               state management + FFI eval
                        ↕
             iOS: @_silgen_name C ABI
             Android: JNI external functions
```

**User action → Rust → JSON → state bump → UI re-renders.**

## Developer workflow

The checked-in adapters in this example use an older bridge. Before regenerating
with the current CLI, migrate their state-store and queued-change helpers to the
[current native host contract](../../../docs/native.md#template-to-ir-conversion).

```bash
# 1. Edit the template
vim views/main.crepus

# 2. Regenerate native views (one command, no patching)
crepus native codegen views/main.crepus --platform swiftui --out ios/Sources/MobileApp --view-name TaskTrackerView
crepus native codegen views/main.crepus --platform compose --out android/app/src/main/java/dev/crepuscularity/mobileapp --view-name TaskTrackerView

# 3. Build Rust backend
cd rust && cargo build --target aarch64-apple-ios --release   # iOS
cd rust && cargo build --target aarch64-linux-android --release  # Android

# 4. Build native apps
cd ios && swift build           # or open in Xcode
cd android && ./gradlew :app:assembleDebug  # or open in Android Studio
```

## Structure

```
mobile-app/
  views/main.crepus          — app UI (3 tabs: Tasks, Notes, Settings)
  fixture.json               — View IR for reference
  rust/                      — Rust action dispatch backend
    src/lib.rs               — state, dispatch, C ABI + JNI exports
  ios/
    Package.swift            — SwiftPM package
    App/TaskTrackerApp.swift — @main, calls CrepusRustActions.install()
    Sources/MobileApp/
      CrepusStateStore.swift — ObservableObject wrapping Rust eval FFI
      CrepusRustActions.swift— @_silgen_name FFI bridge
      TaskTrackerView.swift  — codegen'd from main.crepus
  android/
    app/build.gradle.kts     — Compose + kotlinx-serialization
    java/.../MainActivity.kt — entry, calls CrepusRustActions.install()
    java/.../CrepusStateStore.kt — Compose state wrapping Rust JNI evals
    java/.../CrepusRustActions.kt— JNI external bridge
    java/.../TaskTrackerView.kt  — codegen'd from main.crepus
```

## Template syntax

```crepus
div flex flex-col w-full h-full bg-[#0a0a0a]
  tabs bind=current_tab
    tab value="tasks" label="Tasks"
      for task in {tasks}
        toggle bind=task.done @change="tasks.toggle"
        div bind=task.title          ← renders as Text automatically
        if {task.due != ""}
          div bind=task.due          ← renders as Text automatically
      button @click="tasks.add"      ← dispatches "tasks.add" to Rust
        "Add"
```

Key features: `tabs`/`tab`, `if`/`for` conditionals, `toggle`/`slider` controls,
`bind=` data binding (renders as Text on any element), `@click`/`@change` event handlers.

## Adding a new action

1. Add handler in `rust/src/lib.rs` `dispatch_action_inner()`
2. Add initial state in `initial_view_state()`
3. Call from template: `button @click="my.action"`
4. Regenerate codegen views

## Adding a new screen

1. Add a new `tab` in `views/main.crepus`
2. Regenerate codegen views
3. Add any new state variables in `initial_view_state()`
