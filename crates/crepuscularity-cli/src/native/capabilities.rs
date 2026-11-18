use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crepuscularity_native::{
    ANDROID_ACCESSIBILITY_INFO, ANDROID_ACTION_SHEET, ANDROID_APP, ANDROID_APPEARANCE,
    ANDROID_APP_STATE, ANDROID_BATTERY, ANDROID_BIOMETRICS, ANDROID_BLUETOOTH,
    ANDROID_BLUETOOTH_BRIDGE, ANDROID_BROWSER, ANDROID_CALENDAR, ANDROID_CAMERA, ANDROID_CLIPBOARD,
    ANDROID_CONTACTS, ANDROID_DEEP_LINKS, ANDROID_DEVICE, ANDROID_DIALOG, ANDROID_DIMENSIONS,
    ANDROID_DOCUMENT_PICKER, ANDROID_GEOLOCATION, ANDROID_GEOLOCATION_BRIDGE, ANDROID_HAPTICS,
    ANDROID_IMAGE_PICKER, ANDROID_IN_APP_BROWSER, ANDROID_KEYBOARD, ANDROID_LOCAL_NOTIFICATIONS,
    ANDROID_MICROPHONE, ANDROID_NETWORK, ANDROID_PERMISSIONS, ANDROID_PHOTO_LIBRARY,
    ANDROID_PREFERENCES, ANDROID_PRIVACY_SCREEN, ANDROID_SCHEDULED_NOTIFICATION_RECEIVER,
    ANDROID_SCREEN_ORIENTATION, ANDROID_SECURE_STORAGE, ANDROID_SENSORS, ANDROID_SENSORS_BRIDGE,
    ANDROID_SETTINGS, ANDROID_SHARE, ANDROID_SYSTEM_BARS, ANDROID_TOAST, ANDROID_VIDEO,
    IOS_ACCESSIBILITY_INFO, IOS_ACTION_SHEET, IOS_ACTION_SHEET_BRIDGE, IOS_APP, IOS_APPEARANCE,
    IOS_APP_STATE, IOS_BATTERY, IOS_BIOMETRICS, IOS_BLUETOOTH, IOS_BLUETOOTH_BRIDGE, IOS_BROWSER,
    IOS_CALENDAR, IOS_CAMERA, IOS_CAMERA_BRIDGE, IOS_CLIPBOARD, IOS_CLIPBOARD_BRIDGE, IOS_CONTACTS,
    IOS_DEEP_LINKS, IOS_DEVICE, IOS_DIALOG, IOS_DIALOG_BRIDGE, IOS_DIMENSIONS, IOS_DOCUMENT_PICKER,
    IOS_GEOLOCATION, IOS_GEOLOCATION_BRIDGE, IOS_HAPTICS, IOS_IMAGE_PICKER,
    IOS_IMAGE_PICKER_BRIDGE, IOS_IN_APP_BROWSER, IOS_KEYBOARD, IOS_LOCAL_NOTIFICATIONS,
    IOS_MICROPHONE, IOS_NETWORK, IOS_PERMISSIONS, IOS_PHOTO_LIBRARY, IOS_PHOTO_LIBRARY_BRIDGE,
    IOS_PREFERENCES, IOS_PRIVACY_SCREEN, IOS_SCREEN_ORIENTATION, IOS_SECURE_STORAGE, IOS_SENSORS,
    IOS_SENSORS_BRIDGE, IOS_SETTINGS, IOS_SHARE, IOS_SYSTEM_BARS, IOS_TOAST, IOS_VIDEO,
    IOS_VIDEO_BRIDGE,
};

use crate::error::CrepusCliError;
use crate::ui;

pub struct CapabilitySpec {
    name: &'static str,
    aliases: &'static [&'static str],
    cargo_feature: &'static str,
    android_manifest: &'static str,
    ios_project: &'static str,
    host: fn(&Path) -> Result<(), CrepusCliError>,
}

fn no_op_host(_root: &Path) -> Result<(), CrepusCliError> {
    Ok(())
}

pub const CAPABILITIES: &[CapabilitySpec] = &[
    CapabilitySpec {
        name: "sensors",
        aliases: &["motion", "gyro", "accelerometer"],
        cargo_feature: "sensors",
        android_manifest: "    <uses-feature android:name=\"android.hardware.sensor.accelerometer\" android:required=\"false\" />\n    <uses-feature android:name=\"android.hardware.sensor.gyroscope\" android:required=\"false\" />\n",
        ios_project: "",
        host: add_sensors_host,
    },
    CapabilitySpec {
        name: "bluetooth",
        aliases: &["ble"],
        cargo_feature: "bluetooth",
        android_manifest: "    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" android:maxSdkVersion=\"30\" />\n    <uses-permission android:name=\"android.permission.BLUETOOTH_SCAN\" />\n    <uses-permission android:name=\"android.permission.BLUETOOTH_CONNECT\" />\n    <uses-feature android:name=\"android.hardware.bluetooth_le\" android:required=\"false\" />\n",
        ios_project: "        OTHER_LDFLAGS: \"-lcrepus_mobile_actions -framework CoreBluetooth\"\n        INFOPLIST_KEY_NSBluetoothAlwaysUsageDescription: \"$(PRODUCT_NAME) uses Bluetooth for nearby device setup.\"\n",
        host: add_bluetooth_host,
    },
    CapabilitySpec {
        name: "haptics",
        aliases: &["vibration"],
        cargo_feature: "haptics",
        android_manifest: "    <uses-permission android:name=\"android.permission.VIBRATE\" />\n",
        ios_project: "",
        host: add_haptics_host,
    },
    CapabilitySpec { name: "clipboard", aliases: &[], cargo_feature: "clipboard", android_manifest: "", ios_project: "", host: add_clipboard_host },
    CapabilitySpec { name: "toast", aliases: &[], cargo_feature: "toast", android_manifest: "", ios_project: "", host: add_toast_host },
    CapabilitySpec { name: "privacy-screen", aliases: &["privacyscreen"], cargo_feature: "privacy-screen", android_manifest: "", ios_project: "", host: add_privacy_screen_host },
    CapabilitySpec { name: "video", aliases: &[], cargo_feature: "video", android_manifest: "    <uses-permission android:name=\"android.permission.CAMERA\" />\n    <uses-permission android:name=\"android.permission.RECORD_AUDIO\" />\n    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n", ios_project: "        INFOPLIST_KEY_NSCameraUsageDescription: \"$(PRODUCT_NAME) records video when you ask it to.\"\n        INFOPLIST_KEY_NSMicrophoneUsageDescription: \"$(PRODUCT_NAME) records video audio when you ask it to.\"\n", host: add_video_host },
    CapabilitySpec { name: "browser", aliases: &["linking", "app-launcher", "applauncher", "phone", "sms"], cargo_feature: "browser", android_manifest: "", ios_project: "", host: add_browser_host },
    CapabilitySpec { name: "in-app-browser", aliases: &["inappbrowser", "web-browser"], cargo_feature: "in-app-browser", android_manifest: "", ios_project: "", host: add_in_app_browser_host },
    CapabilitySpec { name: "share", aliases: &[], cargo_feature: "share", android_manifest: "", ios_project: "", host: add_share_host },
    CapabilitySpec {
        name: "documentpicker",
        aliases: &["document-picker", "documents"],
        cargo_feature: "documentpicker",
        android_manifest: "",
        ios_project: "",
        host: add_document_picker_host,
    },
    CapabilitySpec {
        name: "image-picker",
        aliases: &["imagepicker", "media-picker"],
        cargo_feature: "image-picker",
        android_manifest: "",
        ios_project: "",
        host: add_image_picker_host,
    },
    CapabilitySpec {
        name: "photo-library",
        aliases: &["photolibrary", "media-library"],
        cargo_feature: "photo-library",
        android_manifest: "    <uses-permission android:name=\"android.permission.READ_MEDIA_IMAGES\" />\n    <uses-permission android:name=\"android.permission.READ_MEDIA_VIDEO\" />\n    <uses-permission android:name=\"android.permission.READ_EXTERNAL_STORAGE\" android:maxSdkVersion=\"32\" />\n",
        ios_project: "        INFOPLIST_KEY_NSPhotoLibraryUsageDescription: \"$(PRODUCT_NAME) accesses your media library when you ask it to.\"\n",
        host: add_photo_library_host,
    },
    CapabilitySpec {
        name: "camera",
        aliases: &[],
        cargo_feature: "camera",
        android_manifest: "    <uses-permission android:name=\"android.permission.CAMERA\" />\n    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n",
        ios_project: "        INFOPLIST_KEY_NSCameraUsageDescription: \"$(PRODUCT_NAME) uses the camera when you ask it to.\"\n",
        host: add_camera_host,
    },
    CapabilitySpec { name: "dimensions", aliases: &[], cargo_feature: "dimensions", android_manifest: "", ios_project: "", host: add_dimensions_host },
    CapabilitySpec { name: "dialog", aliases: &[], cargo_feature: "dialog", android_manifest: "", ios_project: "", host: add_dialog_host },
    CapabilitySpec { name: "action-sheet", aliases: &["actionsheet"], cargo_feature: "action-sheet", android_manifest: "", ios_project: "", host: add_action_sheet_host },
    CapabilitySpec { name: "app-state", aliases: &["appstate"], cargo_feature: "app-state", android_manifest: "", ios_project: "", host: add_app_state_host },
    CapabilitySpec { name: "app", aliases: &["app-info", "appinfo"], cargo_feature: "app", android_manifest: "", ios_project: "", host: add_app_host },
    CapabilitySpec { name: "screen-orientation", aliases: &["screenorientation"], cargo_feature: "screen-orientation", android_manifest: "", ios_project: "", host: add_screen_orientation_host },
    CapabilitySpec { name: "accessibility-info", aliases: &["accessibilityinfo", "screen-reader", "screenreader"], cargo_feature: "accessibility-info", android_manifest: "", ios_project: "", host: add_accessibility_info_host },
    CapabilitySpec { name: "device", aliases: &["device-info", "deviceinfo", "platform"], cargo_feature: "device", android_manifest: "", ios_project: "", host: add_device_host },
    CapabilitySpec { name: "preferences", aliases: &["storage", "async-storage"], cargo_feature: "preferences", android_manifest: "", ios_project: "", host: add_preferences_host },
    CapabilitySpec { name: "network", aliases: &["net-info", "netinfo"], cargo_feature: "network", android_manifest: "", ios_project: "", host: add_network_host },
    CapabilitySpec { name: "keyboard", aliases: &[], cargo_feature: "keyboard", android_manifest: "", ios_project: "", host: add_keyboard_host },
    CapabilitySpec { name: "settings", aliases: &["app-settings"], cargo_feature: "settings", android_manifest: "", ios_project: "", host: add_settings_host },
    CapabilitySpec { name: "local-notifications", aliases: &["localnotifications", "notifications"], cargo_feature: "local-notifications", android_manifest: "    <uses-permission android:name=\"android.permission.POST_NOTIFICATIONS\" />\n", ios_project: "", host: add_local_notifications_host },
    CapabilitySpec { name: "secure-storage", aliases: &["securestorage"], cargo_feature: "secure-storage", android_manifest: "", ios_project: "", host: add_secure_storage_host },
    CapabilitySpec { name: "biometrics", aliases: &["authentication"], cargo_feature: "biometrics", android_manifest: "", ios_project: "", host: add_biometrics_host },
    CapabilitySpec { name: "permissions", aliases: &["permission"], cargo_feature: "permissions", android_manifest: "", ios_project: "        OTHER_LDFLAGS: \"-lcrepus_mobile_actions -framework CoreBluetooth\"\n        INFOPLIST_KEY_NSBluetoothAlwaysUsageDescription: \"$(PRODUCT_NAME) uses Bluetooth when you ask it to.\"\n", host: add_permissions_host },
    CapabilitySpec { name: "microphone", aliases: &["audio"], cargo_feature: "microphone", android_manifest: "    <uses-permission android:name=\"android.permission.RECORD_AUDIO\" />\n", ios_project: "        INFOPLIST_KEY_NSMicrophoneUsageDescription: \"$(PRODUCT_NAME) uses the microphone when you ask it to.\"\n", host: add_microphone_host },
    CapabilitySpec { name: "calendar", aliases: &["calendars"], cargo_feature: "calendar", android_manifest: "    <uses-permission android:name=\"android.permission.READ_CALENDAR\" />\n    <uses-permission android:name=\"android.permission.WRITE_CALENDAR\" />\n", ios_project: "        INFOPLIST_KEY_NSCalendarsFullAccessUsageDescription: \"$(PRODUCT_NAME) accesses your calendar when you ask it to.\"\n", host: add_calendar_host },
    CapabilitySpec {
        name: "contacts",
        aliases: &[],
        cargo_feature: "contacts",
        android_manifest: "    <uses-permission android:name=\"android.permission.READ_CONTACTS\" />\n",
        ios_project: "        INFOPLIST_KEY_NSContactsUsageDescription: \"$(PRODUCT_NAME) accesses your contacts when you ask it to.\"\n",
        host: add_contacts_host,
    },
    CapabilitySpec {
        name: "filesystem",
        aliases: &["files"],
        cargo_feature: "filesystem",
        android_manifest: "",
        ios_project: "",
        host: no_op_host,
    },
    CapabilitySpec {
        name: "geolocation",
        aliases: &["location"],
        cargo_feature: "geolocation",
        android_manifest: "    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" />\n    <uses-permission android:name=\"android.permission.ACCESS_COARSE_LOCATION\" />\n",
        ios_project: "        INFOPLIST_KEY_NSLocationWhenInUseUsageDescription: \"$(PRODUCT_NAME) uses your location while the app is open.\"\n",
        host: add_geolocation_host,
    },
    CapabilitySpec { name: "battery", aliases: &[], cargo_feature: "battery", android_manifest: "", ios_project: "", host: add_battery_host },
    CapabilitySpec { name: "appearance", aliases: &[], cargo_feature: "appearance", android_manifest: "", ios_project: "", host: add_appearance_host },
    CapabilitySpec { name: "system-bars", aliases: &["systembars", "status-bar", "statusbar"], cargo_feature: "system-bars", android_manifest: "", ios_project: "", host: add_system_bars_host },
    CapabilitySpec { name: "deep-links", aliases: &["deeplinks", "deep-link", "url-events"], cargo_feature: "deep-links", android_manifest: "", ios_project: "", host: add_deep_links_host },
];

pub fn add_capability(capability: &str, root: &Path) -> Result<(), CrepusCliError> {
    let capability = capability.to_ascii_lowercase();
    let spec = CAPABILITIES
        .iter()
        .find(|spec| capability == spec.name || spec.aliases.contains(&capability.as_str()))
        .ok_or_else(|| {
            format!(
                "unknown native capability '{capability}'; available: {}",
                CAPABILITIES
                    .iter()
                    .map(|spec| spec.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let cargo = root.join("rust/Cargo.toml");
    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    let project = root.join("ios/project.yml");
    if !cargo.is_file() || !manifest.is_file() || !project.is_file() {
        return Err(CrepusCliError::context(format!(
            "'{}' is not a native scaffold",
            root.display()
        )));
    }
    add_feature(&cargo, spec.cargo_feature)?;
    if spec.name == "filesystem" {
        enable_default_feature(&cargo, spec.cargo_feature)?;
    }
    add_once(
        &manifest,
        spec.android_manifest,
        "    <uses-permission android:name=\"android.permission.INTERNET\" />\n",
    )?;
    if !spec.ios_project.is_empty() {
        let mut source = fs::read_to_string(&project)
            .map_err(|e| CrepusCliError::io(e, project.to_path_buf()))?;
        if source.contains("INFOPLIST_FILE: App/Info.plist") {
            add_ios_info_plist_key(root, spec.ios_project)?;
        } else if (spec.name == "geolocation"
            && !source.contains("INFOPLIST_KEY_NSLocationWhenInUseUsageDescription"))
            || (spec.name == "photo-library"
                && !source.contains("INFOPLIST_KEY_NSPhotoLibraryUsageDescription"))
            || (spec.name == "camera" && !source.contains("INFOPLIST_KEY_NSCameraUsageDescription"))
            || (spec.name == "video"
                && !source.contains("INFOPLIST_KEY_NSMicrophoneUsageDescription"))
            || (spec.name == "microphone"
                && !source.contains("INFOPLIST_KEY_NSMicrophoneUsageDescription"))
            || (spec.name == "calendar"
                && !source.contains("INFOPLIST_KEY_NSCalendarsFullAccessUsageDescription"))
            || (spec.name == "contacts"
                && !source.contains("INFOPLIST_KEY_NSContactsUsageDescription"))
        {
            source = source.replace(
                "        INFOPLIST_KEY_UILaunchScreen_Generation: YES\n",
                &format!(
                    "        INFOPLIST_KEY_UILaunchScreen_Generation: YES\n{}",
                    spec.ios_project
                ),
            );
            fs::write(&project, source)
                .map_err(|e| CrepusCliError::io(e, project.to_path_buf()))?;
        } else if (spec.name == "bluetooth" || spec.name == "permissions")
            && !source.contains("CoreBluetooth")
        {
            source = source.replace(
                "        OTHER_LDFLAGS: \"-lcrepus_mobile_actions\"\n",
                spec.ios_project,
            );
            fs::write(&project, source)
                .map_err(|e| CrepusCliError::io(e, project.to_path_buf()))?;
        }
    }
    (spec.host)(root)?;
    dedupe_android_imports(root)?;
    ui::success(&format!(
        "added native capability '{}' to '{}'",
        spec.name,
        root.display()
    ));
    Ok(())
}

pub fn add_deep_links_host(root: &Path) -> Result<(), CrepusCliError> {
    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    let mut source =
        fs::read_to_string(&manifest).map_err(|e| CrepusCliError::io(e, manifest.to_path_buf()))?;
    if !source.contains("android.intent.action.VIEW") {
        source = source.replace(
            "        </activity>\n",
            "            <intent-filter>\n                <action android:name=\"android.intent.action.VIEW\" />\n                <category android:name=\"android.intent.category.DEFAULT\" />\n                <category android:name=\"android.intent.category.BROWSABLE\" />\n                <data android:scheme=\"crepus\" />\n            </intent-filter>\n        </activity>\n",
        );
        fs::write(&manifest, source).map_err(|e| CrepusCliError::io(e, manifest.to_path_buf()))?;
    }
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("deepLinksValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"deepLinks\" -> deepLinksValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_DEEP_LINKS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let activity =
        root.join("android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt");
    let mut source =
        fs::read_to_string(&activity).map_err(|e| CrepusCliError::io(e, activity.to_path_buf()))?;
    if !source.contains("receiveDeepLink(intent.data)") {
        source = source.replace(
            "import android.os.Bundle\n",
            "import android.content.Intent\nimport android.os.Bundle\n",
        );
        source = source.replace(
            "        CrepusRustActions.install(this)\n",
            "        CrepusRustActions.install(this)\n        CrepusRustActions.receiveDeepLink(intent.data)\n",
        );
        source = source.replace(
            "\n    override fun onDestroy()",
            "\n    override fun onNewIntent(intent: Intent) {\n        super.onNewIntent(intent)\n        setIntent(intent)\n        CrepusRustActions.receiveDeepLink(intent.data)\n    }\n\n    override fun onDestroy()",
        );
        fs::write(&activity, source).map_err(|e| CrepusCliError::io(e, activity.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("deepLinksValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"deepLinks\":\n            return try deepLinksValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_DEEP_LINKS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    let app = root.join("ios/App/CrepusMobileApp.swift");
    let mut source =
        fs::read_to_string(&app).map_err(|e| CrepusCliError::io(e, app.to_path_buf()))?;
    if !source.contains(".onOpenURL") {
        source = source.replace(
            "        WindowGroup {\n            ContentView()\n        }\n",
            "        WindowGroup {\n            ContentView()\n                .onOpenURL { CrepusRustActions.receiveDeepLink($0) }\n        }\n",
        );
        fs::write(&app, source).map_err(|e| CrepusCliError::io(e, app.to_path_buf()))?;
    }
    let project = root.join("ios/project.yml");
    let mut source =
        fs::read_to_string(&project).map_err(|e| CrepusCliError::io(e, project.to_path_buf()))?;
    if source.contains("GENERATE_INFOPLIST_FILE: YES") {
        source = source.replace(
            "        GENERATE_INFOPLIST_FILE: YES\n",
            "        GENERATE_INFOPLIST_FILE: NO\n        INFOPLIST_FILE: App/Info.plist\n",
        );
        fs::write(&project, &source).map_err(|e| CrepusCliError::io(e, project.to_path_buf()))?;
    }
    let info = root.join("ios/App/Info.plist");
    if !info.exists() {
        fs::write(
            &info,
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>CFBundleURLTypes</key>\n    <array>\n        <dict>\n            <key>CFBundleTypeRole</key>\n            <string>Editor</string>\n            <key>CFBundleURLName</key>\n            <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>\n            <key>CFBundleURLSchemes</key>\n            <array>\n                <string>crepus</string>\n            </array>\n        </dict>\n    </array>\n{}    <key>UILaunchScreen</key>\n    <dict/>\n</dict>\n</plist>\n", ios_info_plist_keys(&source)),
        )
        .map_err(|e| CrepusCliError::io(e, info.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_ios_info_plist_key(root: &Path, setting: &str) -> Result<(), CrepusCliError> {
    let keys = setting
        .lines()
        .filter_map(ios_info_plist_key)
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return Ok(());
    }
    let path = root.join("ios/App/Info.plist");
    let mut source =
        fs::read_to_string(&path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    let mut changed = false;
    for (key, value) in keys {
        if !source.contains(&format!("<key>{key}</key>")) {
            source = source.replace(
                "    <key>UILaunchScreen</key>",
                &format!("    <key>{key}</key>\n    <string>{value}</string>\n    <key>UILaunchScreen</key>"),
            );
            changed = true;
        }
    }
    if changed {
        fs::write(&path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    }
    Ok(())
}

pub fn ios_info_plist_keys(project: &str) -> String {
    project
        .lines()
        .filter_map(|line| ios_info_plist_key(line))
        .map(|(key, value)| format!("    <key>{key}</key>\n    <string>{value}</string>\n"))
        .collect()
}

pub fn ios_info_plist_key(setting: &str) -> Option<(String, &str)> {
    let setting = setting.trim();
    let (key, value) = setting.split_once(": ")?;
    let key = key.strip_prefix("INFOPLIST_KEY_NS")?;
    Some((format!("NS{key}"), value.trim_matches('"')))
}

pub fn add_system_bars_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("systemBarsValue") {
        source = source.replace("import android.content.ClipData\n", "import android.content.ClipData\nimport android.graphics.Color\nimport android.os.Build\nimport android.view.View\n");
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"systemBars\" -> systemBarsValue(method, payload)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_SYSTEM_BARS}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("systemBarsValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"systemBars\":\n            return try systemBarsValue(method: method, payload: payload)\n");
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_SYSTEM_BARS}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_appearance_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("appearanceValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.content.res.Configuration\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"appearance\" -> appearanceValue(method)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_APPEARANCE}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("appearanceValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"appearance\":\n            return try appearanceValue(method: method)\n");
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_APPEARANCE}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_battery_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("batteryValue") {
        source = source.replace("import android.content.Context\n", "import android.content.BroadcastReceiver\nimport android.content.Context\nimport android.content.IntentFilter\nimport android.os.BatteryManager\n");
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"battery\" -> batteryValue(method)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_BATTERY}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("batteryValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"battery\":\n            return try batteryValue(method: method)\n");
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_BATTERY}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_geolocation_ios_host(root: &Path) -> Result<(), CrepusCliError> {
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if source.contains("GeolocationBridge") {
        return Ok(());
    }
    source = source.replace(
        "import Foundation\n",
        "import Foundation\nimport CoreLocation\n",
    );
    source = source.replace(
        "        switch capability {\n",
        "        switch capability {\n        case \"geolocation\":\n            return try geolocationValue(method: method)\n",
    );
    source = source.replace(
        "\n    fileprivate static func emit",
        &format!("{IOS_GEOLOCATION}\n\n    fileprivate static func emit"),
    );
    source = source.replace(
        "    private static func emit(_ result: String)",
        "    static func emit(_ result: String)",
    );
    source = source.replace(
        "    private static func successJson(action: String, capability: String, method: String, value: Any) -> String",
        "    static func successJson(action: String, capability: String, method: String, value: Any) -> String",
    );
    source.push_str(IOS_GEOLOCATION_BRIDGE);
    fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.clone()))?;
    Ok(())
}

pub fn add_geolocation_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if source.contains("GeolocationBridge") {
        return Ok(());
    }
    source = source.replace(
        "import android.content.ClipData\n",
        "import android.content.ClipData\nimport android.Manifest\nimport android.content.pm.PackageManager\nimport android.location.Location\nimport android.location.LocationListener\nimport android.location.LocationManager\n",
    );
    source = source.replace(
        "        when (capability) {\n",
        "        when (capability) {\n            \"geolocation\" -> geolocationValue(method)\n",
    );
    source = source.replace(
        "\n}\n\nobject CrepusActionState",
        &format!("{ANDROID_GEOLOCATION}\n}}\n\nobject CrepusActionState"),
    );
    source.push_str(ANDROID_GEOLOCATION_BRIDGE);
    fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    add_geolocation_ios_host(root)
}

pub fn add_feature(path: &Path, feature: &str) -> Result<(), CrepusCliError> {
    let mut source =
        fs::read_to_string(path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    if source
        .lines()
        .any(|line| line.trim_start().starts_with(&format!("{feature} =")))
    {
        return Ok(());
    }
    if let Some((before, after)) = source.split_once("[features]\n") {
        let end = before.len() + "[features]\n".len() + after.find("\n[").unwrap_or(after.len());
        source.insert_str(end, &format!("{feature} = []\n"));
    } else {
        source.push_str(&format!("\n[features]\n{feature} = []\n"));
    }
    fs::write(path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    Ok(())
}

pub fn enable_default_feature(path: &Path, feature: &str) -> Result<(), CrepusCliError> {
    let mut source =
        fs::read_to_string(path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    let default = "default = []";
    if source.contains(&format!("\"{feature}\"")) {
        return Ok(());
    }
    if source.contains(default) {
        source = source.replacen(default, &format!("default = [\"{feature}\"]"), 1);
    } else if source.contains("default = [") {
        source = source.replacen("default = [", &format!("default = [\"{feature}\", "), 1);
    } else if source.contains("[features]\n") {
        source = source.replacen(
            "[features]\n",
            &format!("[features]\ndefault = [\"{feature}\"]\n"),
            1,
        );
    } else {
        return Err(CrepusCliError::context(format!(
            "missing feature table in '{}'",
            path.display()
        )));
    }
    fs::write(path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    Ok(())
}

pub fn add_bluetooth_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if source.contains("BluetoothBridge") {
        return Ok(());
    }
    source = source.replace(
        "import android.content.Context\n",
        "import android.Manifest\nimport android.bluetooth.BluetoothAdapter\nimport android.bluetooth.le.ScanCallback\nimport android.bluetooth.le.ScanResult\nimport android.content.Context\nimport android.content.pm.PackageManager\n",
    );
    source = source.replace(
        "        when (capability) {\n",
        "        when (capability) {\n            \"bluetooth\" -> bluetoothValue(method)\n",
    );
    source = source.replace(
        "\n}\n\nobject CrepusActionState",
        &format!("{ANDROID_BLUETOOTH}\n}}\n\nobject CrepusActionState"),
    );
    source = source.replace(
        "    private fun emit(result: String)",
        "    internal fun emit(result: String)",
    );
    source.push_str(ANDROID_BLUETOOTH_BRIDGE);
    fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    add_bluetooth_ios_host(root)
}

pub fn add_bluetooth_ios_host(root: &Path) -> Result<(), CrepusCliError> {
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if source.contains("BluetoothBridge") {
        return Ok(());
    }
    source = source.replace(
        "import Foundation\n",
        "import Foundation\nimport CoreBluetooth\n",
    );
    source = source.replace(
        "        switch capability {\n",
        "        switch capability {\n        case \"bluetooth\":\n            return try bluetoothValue(method: method)\n",
    );
    source = source.replace(
        "    private static func successJson",
        "    fileprivate static func successJson",
    );
    source = source.replace(
        "\n    fileprivate static func emit",
        &format!("{IOS_BLUETOOTH}\n\n    fileprivate static func emit"),
    );
    source.push_str(IOS_BLUETOOTH_BRIDGE);
    fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.clone()))?;
    Ok(())
}

pub fn add_haptics_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("hapticsValue") {
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.os.Build\nimport android.os.VibrationEffect\nimport android.os.Vibrator\nimport android.os.VibratorManager\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"haptics\", \"vibration\" -> hapticsValue(method, payload)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_HAPTICS}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("hapticsValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"haptics\", \"vibration\":\n            return try hapticsValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_HAPTICS}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_clipboard_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("clipboardValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.ClipData\nimport android.content.ClipboardManager\nimport android.content.Context\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"clipboard\" -> clipboardValue(method, payload)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_CLIPBOARD}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("clipboardValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"clipboard\":\n            return try clipboardValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_CLIPBOARD}\n\n    fileprivate static func emit"),
        );
        source.push_str(IOS_CLIPBOARD_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_toast_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("toastValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.widget.Toast\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"toast\" -> toastValue(method, payload)\n",
        );
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_TOAST}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("toastValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"toast\":\n            return try toastValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_TOAST}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_video_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("videoValue") {
        source = source.replace("import android.content.Context\n", "import android.app.Activity\nimport android.content.Context\nimport android.provider.MediaStore\n");
        source = source.replace("    private var openDocuments: (() -> Unit)? = null\n", "    private var openDocuments: (() -> Unit)? = null\n    private var pendingVideoAction: String? = null\n    private var captureVideo: (() -> Unit)? = null\n");
        source = source.replace("        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n", "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n        val video = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->\n            val action = pendingVideoAction ?: return@registerForActivityResult\n            pendingVideoAction = null\n            val uri = result.data?.data\n            if (result.resultCode != Activity.RESULT_OK || uri == null) emit(videoCancelledJson(action)) else emit(videoResultJson(action, uri))\n        }\n        captureVideo = { video.launch(Intent(MediaStore.ACTION_VIDEO_CAPTURE)) }\n");
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"video\" -> videoValue(method)\n",
        );
        source = source.replace("\n    private fun dispatchHostAction", &format!("{ANDROID_VIDEO}\n    private fun videoCancelledJson(action: String): String = JSONObject().put(\"ok\", true).put(\"action\", action).put(\"value\", JSONObject().put(\"files\", JSONArray())).toString()\n\n    private fun dispatchHostAction"));
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("videoValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"video\":\n            return try videoValue(method: method)\n");
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_VIDEO}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_VIDEO_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_privacy_screen_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("privacyScreenValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.view.WindowManager\n",
        );
        source = source.replace("        when (capability) {\n", "        when (capability) {\n            \"privacyScreen\" -> privacyScreenValue(method)\n");
        source = source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_PRIVACY_SCREEN}\n}}\n\nobject CrepusActionState"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("privacyScreenValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"privacyScreen\":\n            return try privacyScreenValue(method: method)\n");
        source = source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_PRIVACY_SCREEN}\n\n    fileprivate static func emit"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_browser_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("openUrlValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"browser\", \"linking\", \"appLauncher\", \"phone\", \"sms\" -> openUrlValue(capability, method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_BROWSER}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("openUrlValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"browser\", \"linking\", \"appLauncher\", \"phone\", \"sms\":\n            return try openUrlValue(capability: capability, method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_BROWSER}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_in_app_browser_host(root: &Path) -> Result<(), CrepusCliError> {
    let gradle = root.join("android/app/build.gradle.kts");
    add_once(
        &gradle,
        "    implementation(\"androidx.browser:browser:1.8.0\")\n",
        "dependencies {\n",
    )?;
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("inAppBrowserValue") {
        source = source.replace(
            "import androidx.activity.ComponentActivity\n",
            "import androidx.activity.ComponentActivity\nimport androidx.browser.customtabs.CustomTabsIntent\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"inAppBrowser\" -> inAppBrowserValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_IN_APP_BROWSER}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("inAppBrowserValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport SafariServices\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"inAppBrowser\":\n            return try inAppBrowserValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_IN_APP_BROWSER}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_share_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("shareValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"share\" -> shareValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_SHARE}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("shareValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"share\":\n            return try shareValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_SHARE}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_document_picker_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("documentPickerValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"documentPicker\" -> documentPickerValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_DOCUMENT_PICKER}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("documentPickerValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"documentPicker\":\n            return try documentPickerValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_DOCUMENT_PICKER}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_image_picker_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("imagePickerValue") {
        source = source.replace(
            "    private var openDocuments: (() -> Unit)? = null\n",
            "    private var openDocuments: (() -> Unit)? = null\n    private var openMedia: (() -> Unit)? = null\n",
        );
        source = source.replace(
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n",
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n        openMedia = { filePicker.launch(arrayOf(\"image/*\", \"video/*\")) }\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"imagePicker\" -> imagePickerValue(method)\n",
        );
        source = source.replace(
            "        when (action) {\n            \"import_files\" -> {\n",
            "        when (action) {\n            \"pick_media\" -> {\n                pendingPickerAction = action\n                openMedia?.invoke() ?: emit(errorJson(action, \"media picker unavailable\"))\n                pendingJson(action)\n            }\n            \"import_files\" -> {\n",
        );
        source = source.replace(
            "    private fun dispatchHostAction",
            &format!("{ANDROID_IMAGE_PICKER}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("imagePickerValue") {
        source = source.replace("import UIKit\n", "import PhotosUI\nimport UIKit\n");
        source = source.replace(
            "        switch action {\n        case \"import_files\":\n",
            "        switch action {\n        case \"pick_media\":\n            presentMediaPicker(action: action)\n            return pendingJson(action: action)\n        case \"import_files\":\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"imagePicker\":\n            return try imagePickerValue(method: method)\n",
        );
        source = source.replace(
            "    private static func dispatchHostAction",
            &format!("{IOS_IMAGE_PICKER}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_IMAGE_PICKER_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_photo_library_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("photoLibraryValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.content.pm.PackageManager\nimport android.os.Build\n",
        );
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.provider.MediaStore\n",
        );
        source = source.replace(
            "import java.io.File\n",
            "import java.io.File\nimport java.time.Instant\n",
        );
        source = source.replace(
            "    private var openDocuments: (() -> Unit)? = null\n",
            "    private var openDocuments: (() -> Unit)? = null\n    private var pendingPhotoAction: String? = null\n    private var requestPhotoAccess: ((String) -> Unit)? = null\n",
        );
        source = source.replace(
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n",
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n        val photoPermissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) arrayOf(android.Manifest.permission.READ_MEDIA_IMAGES, android.Manifest.permission.READ_MEDIA_VIDEO) else arrayOf(android.Manifest.permission.READ_EXTERNAL_STORAGE)\n        val photoAccess = activity.registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { grants ->\n            val action = pendingPhotoAction ?: return@registerForActivityResult\n            pendingPhotoAction = null\n            if (grants.values.any { it }) scanPhotoLibrary(action) else emit(errorJson(action, \"photo access denied\"))\n        }\n        requestPhotoAccess = { action ->\n            if (photoPermissions.any { activity.checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }) scanPhotoLibrary(action)\n            else {\n                pendingPhotoAction = action\n                photoAccess.launch(photoPermissions)\n            }\n        }\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"photoLibrary\" -> photoLibraryValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_PHOTO_LIBRARY}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("photoLibraryValue") {
        source = source.replace(
            "import SwiftUI\n#if canImport(UIKit)\n",
            "import SwiftUI\n#if canImport(UIKit)\nimport Photos\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"photoLibrary\":\n            return try photoLibraryValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_PHOTO_LIBRARY}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_PHOTO_LIBRARY_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_camera_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("cameraValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.graphics.Bitmap\n",
        );
        source = source.replace(
            "    private var openDocuments: (() -> Unit)? = null\n",
            "    private var openDocuments: (() -> Unit)? = null\n    private var pendingCameraAction: String? = null\n    private var captureCameraPhoto: (() -> Unit)? = null\n",
        );
        source = source.replace(
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n",
            "        openDocuments = { filePicker.launch(arrayOf(\"*/*\")) }\n        val camera = activity.registerForActivityResult(ActivityResultContracts.TakePicturePreview()) { bitmap ->\n            val action = pendingCameraAction ?: return@registerForActivityResult\n            pendingCameraAction = null\n            if (bitmap == null) emit(cameraCancelledJson(action)) else emit(cameraResultJson(action, bitmap))\n        }\n        captureCameraPhoto = { camera.launch(null) }\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"camera\" -> cameraValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_CAMERA}\n    private fun cameraCancelledJson(action: String): String = JSONObject().put(\"ok\", true).put(\"action\", action).put(\"value\", JSONObject().put(\"files\", JSONArray())).toString()\n\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("cameraValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"camera\":\n            return try cameraValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_CAMERA}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_CAMERA_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_dimensions_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("dimensionsValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"dimensions\" -> dimensionsValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_DIMENSIONS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("dimensionsValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"dimensions\":\n            return try dimensionsValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_DIMENSIONS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_dialog_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("dialogValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.app.AlertDialog\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"dialog\" -> dialogValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_DIALOG}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("dialogValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"dialog\":\n            return try dialogValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "    private static func emit(_ result: String)",
            "    fileprivate static func emit(_ result: String)",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_DIALOG}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_DIALOG_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_action_sheet_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("actionSheetValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.app.AlertDialog\n",
        );
        source = source.replace("        when (capability) {\n", "        when (capability) {\n            \"actionSheet\" -> actionSheetValue(method, payload)\n");
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_ACTION_SHEET}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("actionSheetValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"actionSheet\":\n            return try actionSheetValue(method: method, payload: payload)\n");
        source = source.replace(
            "    private static func emit(_ result: String)",
            "    fileprivate static func emit(_ result: String)",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_ACTION_SHEET}\n\n    private static func dispatchHostAction"),
        );
        source.push_str(IOS_ACTION_SHEET_BRIDGE);
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_app_state_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("appStateValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"appState\" -> appStateValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_APP_STATE}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("appStateValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"appState\":\n            return try appStateValue(method: method)\n");
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_APP_STATE}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_app_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("appValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"app\" -> appValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_APP}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("appValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"app\":\n            return try appValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_APP}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_screen_orientation_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("screenOrientationValue") {
        source = source.replace("        when (capability) {\n", "        when (capability) {\n            \"screenOrientation\" -> screenOrientationValue(method, payload)\n");
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_SCREEN_ORIENTATION}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("screenOrientationValue") {
        source = source.replace("        switch capability {\n", "        switch capability {\n        case \"screenOrientation\":\n            return try screenOrientationValue(method: method, payload: payload)\n");
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_SCREEN_ORIENTATION}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_accessibility_info_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("accessibilityInfoValue") {
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.provider.Settings\nimport android.view.accessibility.AccessibilityManager\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"accessibilityInfo\", \"screenReader\" -> accessibilityInfoValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_ACCESSIBILITY_INFO}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("accessibilityInfoValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"accessibilityInfo\", \"screenReader\":\n            return try accessibilityInfoValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_ACCESSIBILITY_INFO}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_device_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("deviceValue") {
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.os.Build\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"device\", \"platform\" -> deviceValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_DEVICE}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("deviceValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"device\", \"platform\":\n            return try deviceValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_DEVICE}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_preferences_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("preferencesValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"preferences\" -> preferencesValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_PREFERENCES}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("preferencesValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"preferences\":\n            return try preferencesValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_PREFERENCES}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_network_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("networkValue") {
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.net.ConnectivityManager\nimport android.net.Network\nimport android.net.NetworkCapabilities\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"network\" -> networkValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_NETWORK}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("networkValue") {
        source = source.replace("import Foundation\n", "import Foundation\nimport Network\n");
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"network\":\n            return try networkValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_NETWORK}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_keyboard_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("keyboardValue") {
        source = source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.view.inputmethod.InputMethodManager\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"keyboard\" -> keyboardValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_KEYBOARD}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("keyboardValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"keyboard\":\n            return try keyboardValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_KEYBOARD}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_settings_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("settingsValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"settings\" -> settingsValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_SETTINGS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("settingsValue") {
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"settings\":\n            return try settingsValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_SETTINGS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_local_notifications_host(root: &Path) -> Result<(), CrepusCliError> {
    let receiver = root.join(
        "android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusNotificationReceiver.kt",
    );
    if !receiver.exists() {
        fs::write(&receiver, ANDROID_SCHEDULED_NOTIFICATION_RECEIVER)
            .map_err(|e| CrepusCliError::io(e, receiver.to_path_buf()))?;
    }
    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    add_once(
        &manifest,
        "        <receiver android:name=\".CrepusNotificationReceiver\" android:exported=\"false\" />\n",
        "        </activity>\n",
    )?;
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("localNotificationsValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.Manifest\nimport android.app.AlarmManager\nimport android.app.Notification\nimport android.app.NotificationChannel\nimport android.app.NotificationManager\nimport android.app.PendingIntent\nimport android.content.Context\nimport android.content.pm.PackageManager\nimport android.os.Build\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"localNotifications\" -> localNotificationsValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_LOCAL_NOTIFICATIONS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("localNotificationsValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport UserNotifications\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"localNotifications\":\n            return try localNotificationsValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_LOCAL_NOTIFICATIONS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_secure_storage_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("secureStorageValue") {
        source = source.replace(
            "import android.content.Context\n",
            "import android.content.Context\nimport android.security.keystore.KeyGenParameterSpec\nimport android.security.keystore.KeyProperties\nimport android.util.Base64\nimport java.security.KeyStore\nimport javax.crypto.Cipher\nimport javax.crypto.KeyGenerator\nimport javax.crypto.SecretKey\nimport javax.crypto.spec.GCMParameterSpec\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"secureStorage\" -> secureStorageValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_SECURE_STORAGE}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("secureStorageValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport Security\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"secureStorage\":\n            return try secureStorageValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_SECURE_STORAGE}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_biometrics_host(root: &Path) -> Result<(), CrepusCliError> {
    let gradle = root.join("android/app/build.gradle.kts");
    add_once(
        &gradle,
        "    implementation(\"androidx.biometric:biometric:1.1.0\")\n",
        "dependencies {\n",
    )?;
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("biometricsValue") {
        source = source.replace(
            "import androidx.activity.ComponentActivity\n",
            "import androidx.activity.ComponentActivity\nimport androidx.biometric.BiometricManager\nimport androidx.biometric.BiometricPrompt\nimport androidx.core.content.ContextCompat\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"biometrics\", \"authentication\" -> biometricsValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_BIOMETRICS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
        let activity =
            root.join("android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt");
        let source = fs::read_to_string(&activity)
            .map_err(|e| CrepusCliError::io(e, activity.to_path_buf()))?
            .replace(
                "import androidx.activity.ComponentActivity\n",
                "import androidx.fragment.app.FragmentActivity\n",
            )
            .replace(
                "class MainActivity : ComponentActivity()",
                "class MainActivity : FragmentActivity()",
            );
        fs::write(&activity, source).map_err(|e| CrepusCliError::io(e, activity.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("biometricsValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport LocalAuthentication\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"biometrics\", \"authentication\":\n            return try biometricsValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_BIOMETRICS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_permissions_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("permissionsValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"permissions\" -> permissionsValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_PERMISSIONS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("permissionsValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport AVFoundation\nimport Contacts\nimport CoreBluetooth\nimport CoreLocation\nimport Photos\nimport UserNotifications\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"permissions\":\n            return try permissionsValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_PERMISSIONS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_microphone_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("microphoneValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"microphone\" -> microphoneValue(method)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_MICROPHONE}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("microphoneValue") {
        if !source.contains("import AVFoundation\n") {
            source = source.replace(
                "import Foundation\n",
                "import Foundation\nimport AVFoundation\n",
            );
        }
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"microphone\":\n            return try microphoneValue(method: method)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_MICROPHONE}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_contacts_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("contactsValue") {
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"contacts\" -> contactsValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_CONTACTS}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("contactsValue") {
        if !source.contains("import Contacts\n") {
            source = source.replace(
                "import Foundation\n",
                "import Foundation\nimport Contacts\n",
            );
        }
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"contacts\":\n            return try contactsValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_CONTACTS}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_calendar_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let mut source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !source.contains("calendarValue") {
        source = source.replace(
            "import android.content.ClipData\n",
            "import android.content.ClipData\nimport android.content.ContentValues\nimport android.provider.CalendarContract\nimport java.util.TimeZone\n",
        );
        source = source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"calendar\", \"calendars\" -> calendarValue(method, payload)\n",
        );
        source = source.replace(
            "\n    private fun dispatchHostAction",
            &format!("{ANDROID_CALENDAR}\n    private fun dispatchHostAction"),
        );
        fs::write(&android, source).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !source.contains("calendarValue") {
        source = source.replace(
            "import Foundation\n",
            "import Foundation\nimport EventKit\n",
        );
        source = source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"calendar\", \"calendars\":\n            return try calendarValue(method: method, payload: payload)\n",
        );
        source = source.replace(
            "\n    private static func dispatchHostAction",
            &format!("{IOS_CALENDAR}\n\n    private static func dispatchHostAction"),
        );
        fs::write(&ios, source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_sensors_host(root: &Path) -> Result<(), CrepusCliError> {
    let android = android_actions_path(root)?;
    let ios = root.join("ios/Sources/NativeShell/CrepusRustActions.swift");
    let mut android_source =
        fs::read_to_string(&android).map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    if !android_source.contains("SensorBridge") {
        android_source = android_source.replace(
            "import android.net.Uri\n",
            "import android.net.Uri\nimport android.hardware.Sensor\nimport android.hardware.SensorEvent\nimport android.hardware.SensorEventListener\nimport android.hardware.SensorManager\n",
        );
        android_source = android_source.replace(
            "        when (capability) {\n",
            "        when (capability) {\n            \"sensors\", \"motion\" -> sensorsValue(method)\n",
        );
        android_source = android_source.replace(
            "\n}\n\nobject CrepusActionState",
            &format!("{ANDROID_SENSORS}\n}}\n\nobject CrepusActionState"),
        );
        android_source.push_str(ANDROID_SENSORS_BRIDGE);
        fs::write(&android, android_source)
            .map_err(|e| CrepusCliError::io(e, android.to_path_buf()))?;
    }
    let mut ios_source =
        fs::read_to_string(&ios).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    if !ios_source.contains("SensorBridge") {
        ios_source = ios_source.replace(
            "import Foundation\n",
            "import Foundation\nimport CoreMotion\n",
        );
        ios_source = ios_source.replace(
            "        switch capability {\n",
            "        switch capability {\n        case \"sensors\", \"motion\":\n            return try sensorsValue(method: method)\n",
        );
        ios_source = ios_source.replace(
            "\n    fileprivate static func emit",
            &format!("{IOS_SENSORS}\n\n    fileprivate static func emit"),
        );
        ios_source.push_str(IOS_SENSORS_BRIDGE);
        fs::write(&ios, ios_source).map_err(|e| CrepusCliError::io(e, ios.to_path_buf()))?;
    }
    Ok(())
}

pub fn android_actions_path(root: &Path) -> Result<PathBuf, CrepusCliError> {
    let packages = root.join("android/app/src/main/java/dev/crepuscularity");
    fs::read_dir(&packages)
        .map_err(|e| CrepusCliError::io(e, packages.to_path_buf()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("CrepusRustActions.kt"))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            CrepusCliError::context(format!(
                "Android Rust actions not found under '{}'",
                packages.display()
            ))
        })
}

pub fn dedupe_android_imports(root: &Path) -> Result<(), CrepusCliError> {
    let path = android_actions_path(root)?;
    let source =
        fs::read_to_string(&path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    let mut imports = HashSet::new();
    let deduped = source
        .lines()
        .filter(|line| !line.starts_with("import ") || imports.insert(*line))
        .collect::<Vec<_>>()
        .join("\n");
    if deduped != source.trim_end() {
        fs::write(&path, format!("{deduped}\n"))
            .map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    }
    Ok(())
}

pub fn add_once(path: &Path, addition: &str, anchor: &str) -> Result<(), CrepusCliError> {
    let mut source =
        fs::read_to_string(path).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    if addition.starts_with("    <uses-") {
        let mut missing = String::new();
        for line in addition.lines() {
            let Some(name) = line
                .split("android:name=\"")
                .nth(1)
                .and_then(|part| part.split('"').next())
            else {
                missing.push_str(line);
                missing.push('\n');
                continue;
            };
            let identity = format!("android:name=\"{name}\"");
            let existing = source
                .lines()
                .find(|candidate| candidate.contains(&identity))
                .map(str::to_owned);
            match existing {
                Some(existing)
                    if existing.contains("android:maxSdkVersion")
                        && !line.contains("android:maxSdkVersion") =>
                {
                    source = source.replacen(&existing, line, 1);
                }
                Some(_) => {}
                None => {
                    missing.push_str(line);
                    missing.push('\n');
                }
            }
        }
        if missing.is_empty() {
            return fs::write(path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()));
        }
        source = source.replacen(anchor, &format!("{anchor}{missing}"), 1);
        return fs::write(path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()));
    }
    if source.contains(addition.lines().next().unwrap_or_default()) {
        return Ok(());
    }
    if addition.starts_with("[features]") {
        source.push('\n');
        source.push_str(addition);
    } else {
        source = source.replacen(anchor, &format!("{anchor}{addition}"), 1);
    }
    fs::write(path, source).map_err(|e| CrepusCliError::io(e, path.to_path_buf()))?;
    Ok(())
}
