# Native Shells

**Also:** [Documentation home](README.md) · [DSL](dsl.md) · [Components](components.md) · [CLI](cli.md) · [Polyglot plugins](polyglot.md)

Crepuscularity can target native mobile platforms through **View IR** (Intermediate Representation), enabling code sharing between iOS (SwiftUI) and Android (Jetpack Compose) applications.

## Overview

Instead of compiling directly to native UI frameworks, Crepuscularity generates a **JSON-based intermediate representation** that native shells can interpret. This approach allows:

- **Code sharing** between platforms
- **Runtime flexibility** - update UI without app store releases
- **Consistent styling** across platforms
- **Platform-specific optimizations**

## Architecture

```
.crepus Template → Rust Parser → View IR (JSON) → Native Shell
                                      ↓
                               SwiftUI / Jetpack Compose
```

The `crepuscularity-native` crate converts templates to a structured JSON format that native applications can consume.

Tabs (`tabs`, `tabview`, `page-switcher`) accept direct `tab` and `page` children
plus literal whitespace. Other direct content, declarations and structural
wrappers produce errors rather than disappearing. Panel bodies retain normal
layout, declarations and control flow.

For non-Rust hosts, the stable boundary is the same View IR JSON emitted by `crepus native ir`. See [Polyglot plugins](polyglot.md).

## Quick Start

Prefer the unified mobile workflow for new iOS + Android apps:

```bash
crepus mobile new my-mobile-app
cd my-mobile-app
crepus mobile dev --platform all
crepus mobile build --platform ios
crepus mobile run --platform ios
crepus mobile build --platform android
```

### Cross-platform scaffold (iOS + Android in one command)

```bash
# Scaffold an iOS XcodeGen app + Android Gradle module sharing a fixture.json
crepus native new my-mobile-app
cd my-mobile-app

# iOS — build, install, and launch the simulator app
crepus mobile build --platform ios --dir .
crepus mobile run --platform ios --dir .
# or:    crepus native build ios --dir .

# Android — generate the Gradle wrapper (one-time), then build
cd android && gradle wrapper --gradle-version 8.10
./gradlew :app:assembleDebug
# or, from the project root:
#    crepus native build android --dir <root>
```

Android builds apply `[android]` `namespace` and `application_id` overrides from
`crepus.toml` to simple assignments in `android/app/build.gradle.kts`, retaining
similarly named fields such as `applicationIdSuffix` and trailing `//` comments.

`crepus native run android --dir <root>` runs `gradle :app:installDebug`, then
launches the app on a connected device/emulator with `adb shell am start`.
The launch component uses the simple `applicationId` assignment in
`android/app/build.gradle.kts`. Trailing `//` comments are ignored outside quoted
values; comment markers inside a quoted value are preserved.

The default shell contains View IR, host glue, and the app-local Rust bridge only: no
vendored Bluetooth Java or Bluetooth permission. Add platform capabilities per app:

```bash
crepus native add sensors --dir .
crepus native add bluetooth --dir .
crepus native add image-picker --dir .
crepus native add photo-library --dir .
```

Sensors installs real gyro/accelerometer bridges. Bluetooth installs BLE central scan bridges on
Android and iOS, plus only the required platform declarations. It supports `status`,
`requestPermission`, `scan`, and `stopScan`; discoveries arrive as `bluetooth.device` results.

| Capability | Methods | Boundary |
| --- | --- | --- |
| `sensors` | `status`, `latest`, `start`, `stop` | Android accelerometer/gyro; iOS Core Motion |
| `bluetooth` | `status`, `requestPermission`, `scan`, `stopScan` | Android and iOS central scanning; discoveries are streamed as results |
| `geolocation` | `status`, `requestPermission`, `getCurrentPosition`, `startWatch`, `stopWatch` | Android GPS/network watch and iOS CLLocationManager updates; locations stream as `geolocation.update` |
| `battery` | `status`, `startWatch`, `stopWatch` | Android battery broadcast and iOS battery notifications; watch changes stream as `battery.change` |
| `appearance` | `status`, `startWatch`, `stopWatch` | Current light/dark scheme on Android and iOS; watch changes stream as `appearance.change` |
| `haptics` | `impact`, `selection`, `notification` | Android vibrator; iOS UIKit feedback generators |
| `clipboard` | `get`, `set`, `clear`, `startWatch`, `stopWatch` | Android clipboard manager and iOS system pasteboard; watch changes stream as `clipboard.change` |
| `toast` | `show` | Android system toast or a transient UIKit overlay; payload requires `text` and accepts `duration: "short"` or `"long"` |
| `privacy-screen` (`privacyscreen`) | `status`, `enable`, `disable` | Blocks Android screenshots/recording or obscures the app while iOS is inactive |
| `browser` (`linking`, `app-launcher`, `phone`, `sms`) | `open` | Android `ACTION_VIEW`; iOS `UIApplication.open`, including custom app, `tel:`, and `sms:` schemes |
| `share` | `share` | Android chooser; iOS activity sheet; accepts `text`, `url`, and optional `title` |
| `image-picker` (`imagepicker`, `media-picker`) | `pick` | Android system picker; iOS PHPicker; results stream as `imagePicker.pick`. The named `pick_media` action is installed with this capability. |
| `documentPicker` (`documents`) | `pick` | Android/iOS multiple-document picker; emits imported cached-file metadata |
| `photo-library` (`photolibrary`, `media-library`) | `scan`, `getRecentMedia` | Android MediaStore and iOS PhotoKit, each requesting media access only when called; emits one cached-file result per image/video asset, with `mediaKind` from the library asset type; `getRecentMedia` visits newest first, `scan` oldest first (`getRecentMedia` uses a combined image/video query on Android, breaking equal-date ties by descending media ID) |
| `camera` | `takePhoto` | Android system camera preview and iOS camera picker; emits one cached JPEG result |
| `video` | `capture` | Android system video camera or iOS camera movie picker; emits cached video metadata |
| `dimensions` | `get`, `getWindow` | Current Android window metrics or iOS screen bounds, expressed in display points |
| `dialog` | `show` | Android alert dialog and iOS alert controller; accepts optional `title`, `message`, and `button`, then streams `ok` or `cancel` |
| `action-sheet` (`actionsheet`) | `show` | Android choices dialog and iOS action sheet; accepts `title` and `options`, then streams the selected label and index |
| `app-state` (`appstate`) | `get`, `startWatch`, `stopWatch` | Current Android lifecycle or iOS application foreground/background state; watch changes stream as `appState.change` |
| `app` (`app-info`, `appinfo`) | `getInfo` | App name, identifier, version, and build metadata from Android package or iOS bundle |
| `screen-orientation` (`screenorientation`) | `get`, `lock`, `unlock` | Current orientation plus portrait/landscape lock or unlock through Android activity policy and iOS scene geometry |
| `accessibility-info` (`accessibilityinfo`, `screen-reader`) | `get`, `status`, `startWatch`, `stopWatch` | Reduce-motion and screen-reader state from Android accessibility settings or iOS UIAccessibility; watch changes stream as `accessibilityInfo.change` |
| `device` (`device-info`, `deviceinfo`, `platform`) | `get`, `info` | Platform, manufacturer, model, and operating-system version from Android Build or iOS UIDevice |
| `preferences` (`storage`, `async-storage`) | `get`, `set`, `remove`, `clear` | String values in Android SharedPreferences or iOS UserDefaults; `key` is required except for `clear` |
| `network` (`net-info`, `netinfo`) | `status`, `startWatch`, `stopWatch` | Validated Android network or iOS NWPathMonitor status, including active transport; watch changes stream as `network.change` |
| `keyboard` | `dismiss` | Hides Android's active input method or resigns iOS's current first responder |
| `settings` (`app-settings`) | `open` | Opens the app's Android or iOS settings page |
| `local-notifications` (`notifications`) | `status`, `requestPermission`, `post`, `schedule`, `cancel`, `list` | Android AlarmManager receiver and iOS UserNotifications; schedule with `id` plus future Unix-millisecond `at` or `seconds`; iOS `list` reports asynchronously |
| `secure-storage` | `get`, `set`, `remove`, `clear` | Android Keystore-encrypted values and iOS Keychain values |
| `biometrics` (`authentication`) | `status`, `authenticate` | AndroidX BiometricPrompt or iOS LocalAuthentication, streaming the authentication result |
| `permissions` (`permission`) | `status`, `check`, `request` | Checks or requests `camera`, `microphone`, `location`, `photoLibrary`/`photos`, `contacts`, `notifications`, and `bluetooth`; notification checks report current iOS OS authorization asynchronously |
| `microphone` (`audio`) | `status`, `requestPermission` | Android `RECORD_AUDIO` or iOS `AVAudioSession` recording authorization |
| `contacts` | `status`, `requestPermission`, `list` | Android ContactsContract or iOS CNContactStore; access is declared only when installed |
| `calendar` (`calendars`) | `status`, `check`, `request`, `list`, `create` | Android CalendarContract and iOS EventKit; `create` accepts `title`, optional Unix-millisecond `start`, `end`, `calendarId`, and `notes` |
| `in-app-browser` (`inappbrowser`, `web-browser`) | `open` | Android Custom Tabs or iOS Safari View Controller |
| `system-bars` (`systembars`, `status-bar`) | `status`, `set` | Android status/navigation colors and light-icon flags; iOS supported window appearance style |
| `deep-links` (`deeplinks`, `deep-link`, `url-events`) | `status`, `getInitialUrl`, `open` | Receives `crepus://` URLs as `deepLinks.openUrl` events and opens valid URLs through the platform |

Document-picker metadata reads and file copies run on serial workers; final results
return on the main thread. Android queues at most four waiting imports and rejects
further submissions explicitly; Activity teardown invalidates pending delivery.
iOS admits one import at a time, retaining that slot until a canceled copy finishes.
Cancellation or teardown suppresses stale publication and removes completed stale
copies, but does not interrupt an ongoing provider read. iOS security-scoped access
covers metadata and copying. Real provider grants and device responsiveness still
require platform testing.

`filesystem` enables its scoped Rust backend only after `crepus native add filesystem`; the
default app shell and share-extension builds exclude it. It supports `readText`, `writeText`,
`delete`, `mkdir`, `list`, and `stat`, and rejects absolute paths and parent-directory segments.

### Service and entitlement boundaries

Capabilities installed by `crepus native add` stay local to the app and add only their required
platform declarations. Existing iOS permission usage descriptions are preserved;
overlapping capabilities add only missing usage-description keys. Push delivery needs an APNs/FCM project and server credentials; payments,
wallets, Google Maps, cloud sync, and authentication need the app's own provider configuration.
Health, NFC, background execution, widgets, and media services also require
their platform-specific entitlement or user permission before an app can use them. They are not
included in the default scaffold or represented as successful no-op bridges.

The scaffold includes `views/main.crepus` as the Crepus-authored UI source.
When the template changes, sync the shared View IR fixture into the native
containers:

```bash
crepus native sync views/main.crepus --dir . --out desktop/share/dashboard.view-ir.json --var name=Ada --pretty
```

Sync validates all destinations before updating any fixture. Default destinations
must stay inside the scaffold directory. Explicit `--out` paths may select an
external directory; destination file links must resolve within that selected
parent. Existing internal links remain supported. This validation assumes stable
filesystem links; later I/O failures can still leave some fixtures updated.

### iOS-only scaffold with XcodeGen + a real `.xcodeproj`

For iOS-only apps, use the iOS-specific scaffolder:

```bash
wax install xcodegen
crepus ios new my-ios-app
cd my-ios-app
crepus ios generate
open *.xcodeproj
```

### Android (Jetpack Compose)

`crepus native new` lays out a minimal Compose app under `<dir>/android/`.
For a deeper reference, see `examples/native-shells/android/`.

## Template to IR Conversion

Convert `.crepus` templates to View IR JSON:

```bash
crepus native ir views/main.crepus --ctx context.json --pretty
crepus native sync views/main.crepus --dir my-mobile-app --out desktop/share/dashboard.view-ir.json --var name=Ada --pretty
crepus native codegen views/main.crepus --platform swiftui --out Generated --view-name DashboardView --var name=Ada
crepus native codegen views/main.crepus --platform compose --out android/app/src/main/java/dev/example/generated --view-name DashboardView --var name=Ada
```

Generated bound controls require the async edit helpers in the current native
host templates. When upgrading an existing host, migrate its `CrepusStateStore`
draft helpers and `CrepusRustActions` queued-change bridge together before
regenerating views. Regeneration alone does not update custom host adapters.
The explicit synchronous change APIs remain available for existing callers.
The Swift bridge returns complete per-call action JSON through
`crepus_mobile_dispatch_json_owned` and `crepus_mobile_dispatch_change_json_owned`,
then releases each result with `crepus_mobile_free_string`. Large results do not
require replaying the action. Rebuild the app-local Rust library when adopting
this bridge; older libraries lack these symbols. Existing buffer-based C exports
remain available with their required-length/truncation contract.
Generated Android `CrepusActions.perform` delivers recoverable dispatcher
exceptions as `{"ok":false,"error":"..."}` results through its captured main-thread
sink. The serial queue waits for this delivery before dispatching the next action.
After an accepted main-thread post, both action and bound-change jobs wait through
worker interruption until delivery finishes, then restore interrupt status.

```rust
use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_template_to_ir, to_json_pretty};

let mut ctx = TemplateContext::new();
ctx.set("name", "Ada");

let ir = render_template_to_ir(
    r#"div flex flex-col gap-4
  span
    "Hello {name}""#,
    &ctx,
)?;

let json = to_json_pretty(&ir)?;
println!("{}", json);
```

When the IR is no longer needed, `crepuscularity_native::to_json_owned(ir)`
consumes it and produces compact JSON with iterative child traversal. It preserves
Serde's field order, escaping, and omission of optional fields, while avoiding
recursive serialization of deeply nested containers. The WASM bindings and C ABI
use this path. `to_json(&ir)` and `to_json_pretty(&ir)` remain available for callers
that retain the IR.

This produces JSON that native shells can parse:

```json
{
  "version": 7,
  "root": [
    {
      "kind": "stack",
      "axis": "column",
      "spacing": 16.0,
      "style": {
        "flexDirection": "column"
      },
      "children": [
        {
          "kind": "text",
          "content": "Hello Ada"
        }
      ]
    }
  ]
}
```

## iOS Integration

### Using the Swift Package

Add the local Swift package to your Xcode project:

1. In Xcode: **File → Add Package Dependencies**
2. Select **Add Local...** and choose the `ios/` directory
3. Import `NativeShell` in your Swift code

```swift
import NativeShell
import SwiftUI

struct ContentView: View {
    var body: some View {
        // Use the fixture
        FixtureRootView()

        // Or load custom IR
        if let ir = loadViewIR() {
            ViewIrRootView(ir: ir)
        }
    }
}
```

### CLI-Scaffolded Apps

The `crepus ios` commands create a complete Xcode project:

```bash
crepus ios new MyApp
cd MyApp
crepus ios generate  # Creates .xcodeproj
crepus ios build    # Builds for simulator
```

## Android Integration

### Jetpack Compose Setup

`crepus native new <name>` lays out a Compose app under `<name>/android/` —
the same source as `examples/native-shells/android/`:

```kotlin
// MainActivity.kt
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            // Load and render View IR
            val ir = loadViewIR(assets)
            ViewIrRootView(ir)
        }
    }
}
```

## Supported Features

### Layout

- **Flexbox**: `flex`, `flex-row`, `flex-col`, `justify-center`, `items-center`
- **Spacing**: `gap-4`, `p-4`, `m-4`
- **Scroll containers**: `scroll`, `scroll-x`, `scroll-y`

### Styling

- **Colors**: `bg-blue-500`, `text-white`, `border-gray-300`
- **Typography**: `text-lg`, `font-bold`, `leading-relaxed`
- **Borders**: `rounded`, `border`, `border-2`
- **Shadows**: `shadow`, `shadow-lg`

### Components

- **Basic elements**: `div`, `span`, `button`, `img`
- **Interactive**: `button @click` emits an action string for the native shell to handle

### Advanced Widgets

- **Slot rotate**: Emits phrase data; the example shells currently display the first phrase
- **Custom components**: Via `include`
- **Control flow**: `if`, `for`, and `match`
- **Slots**: Component slots with caller context

The example SwiftUI and Jetpack Compose shells are intentionally small interpreters for the IR. They decode and render the core nodes, while app-specific image loading, navigation, action routing, accessibility, and animation behavior belong in the host shell.

## Generated Source

For production mobile shells that should compile native source instead of interpreting JSON at runtime, generate SwiftUI or Jetpack Compose from the same View IR:

```bash
crepus native codegen views/main.crepus --platform swiftui --out Generated --view-name DashboardView --var name=Ada
crepus native codegen views/main.crepus --platform compose --out android/app/src/main/java/dev/example/generated --view-name DashboardView --var name=Ada
```

The generator keeps View IR as the stable compiler boundary, then writes `DashboardView.swift` or `DashboardView.kt`. The generated source covers current IR nodes with native SwiftUI/Compose controls and conservative textual fallbacks for host-specific surfaces like image loading and drop handling.

## Runtime Updates

Since View IR is JSON, you can update UI at runtime:

```swift
// iOS
func updateUI(with newIR: ViewIR) {
    // Parse new IR and update view hierarchy
    viewIrRootView.updateIR(newIR)
}
```

```kotlin
// Android
fun updateUI(newIR: ViewIR) {
    // Recompose with new IR
    setContent {
        ViewIrRootView(newIR)
    }
}
```

## Development Workflow

1. **Design templates** in `.crepus` files
2. **Generate IR** using `crepuscularity-native`
3. **Test in native shells** using the examples
4. **Integrate** into your app's native code

### Hot Reload

For development, you can set up file watching:

```bash
# Watch templates and regenerate IR
cargo watch -x 'run --bin generate-ir'
```

## Platform-Specific Customization

While the core layout is shared, each platform can customize:

### iOS (SwiftUI)

- **Custom fonts** and **SF Symbols**
- **iOS-specific animations** and transitions
- **Accessibility** features
- **Dark mode** integration

### Android (Jetpack Compose)

- **Material Design 3** components
- **Platform animations**
- **System UI integration**
- **Dynamic color** themes

## Performance Considerations

- **IR parsing** happens at runtime - keep templates reasonably sized
- **Includes** are expanded before lowering to IR; file-backed includes reject absolute paths and `..` traversal
- **Image loading** should be optimized per platform
- **Animations** should be implemented in the host shell until animation nodes are added to the IR
- **Memory usage** scales with template complexity

## Examples

See `examples/native-shells/` for complete working examples (the same source
that `crepus native new` scaffolds):

- **iOS**: SwiftUI app with Swift Package integration
- **Android**: Jetpack Compose app with Gradle setup
- **Shared fixture**: Common View IR for testing

`crepus native new <name>` writes that tree (minus the binary Gradle wrapper
jar — generate it with `gradle wrapper --gradle-version 8.10` or open the
project in Android Studio).

## Extending the IR

To add new features:

1. **Update the Rust parser** in `crepuscularity-native`
2. **Extend the JSON schema** (run `export-view-ir-schema`)
3. **Update native shells** to handle new IR nodes
4. **Test across platforms**

The IR format is designed to be extensible while maintaining compatibility.
