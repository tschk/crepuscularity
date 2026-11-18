import Darwin
import Foundation
import SwiftUI
#if canImport(UIKit)
import UIKit
import UniformTypeIdentifiers
#elseif canImport(AppKit)
import AppKit
#endif

@_silgen_name("crepus_mobile_dispatch")
private func crepusMobileDispatch(_ action: UnsafePointer<CChar>, _ length: UInt) -> Bool

@_silgen_name("crepus_mobile_dispatch_json")
private func crepusMobileDispatchJson(_ action: UnsafePointer<CChar>, _ length: UInt, _ output: UnsafeMutablePointer<CChar>, _ outputLength: UInt) -> UInt
@_silgen_name("crepus_mobile_dispatch_and_store_json")
private func crepusMobileDispatchAndStoreJson(_ action: UnsafePointer<CChar>, _ length: UInt, _ output: UnsafeMutablePointer<CChar>, _ outputLength: UInt) -> UInt
@_silgen_name("crepus_mobile_dispatch_change_json")
private func crepusMobileDispatchChangeJson(_ action: UnsafePointer<CChar>, _ actionLength: UInt, _ bind: UnsafePointer<CChar>, _ bindLength: UInt, _ value: UnsafePointer<CChar>, _ valueLength: UInt, _ output: UnsafeMutablePointer<CChar>, _ outputLength: UInt) -> UInt
@_silgen_name("crepus_mobile_last_result")
private func crepusMobileLastResult() -> UnsafeMutablePointer<CChar>?
@_silgen_name("crepus_mobile_last_error")
private func crepusMobileLastError() -> UnsafeMutablePointer<CChar>?
@_silgen_name("crepus_mobile_free_string")
private func crepusMobileFreeString(_ pointer: UnsafeMutablePointer<CChar>?)
@_silgen_name("crepus_mobile_store_result_json")
private func crepusMobileStoreResultJson(_ json: UnsafePointer<CChar>, _ length: UInt) -> Bool
@_silgen_name("crepus_mobile_eval_text")
private func crepusMobileEvalText(_ expr: UnsafePointer<CChar>, _ exprLength: UInt, _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt, _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt, _ output: UnsafeMutablePointer<CChar>, _ outputLength: UInt) -> UInt
@_silgen_name("crepus_mobile_eval_bool")
private func crepusMobileEvalBool(_ expr: UnsafePointer<CChar>, _ exprLength: UInt, _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt, _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt) -> Bool
@_silgen_name("crepus_mobile_eval_number")
private func crepusMobileEvalNumber(_ expr: UnsafePointer<CChar>, _ exprLength: UInt, _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt, _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt) -> Double
@_silgen_name("crepus_mobile_eval_items_json")
private func crepusMobileEvalItemsJson(_ expr: UnsafePointer<CChar>, _ exprLength: UInt, _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt, _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt, _ output: UnsafeMutablePointer<CChar>, _ outputLength: UInt) -> UInt

@MainActor
public enum CrepusRustActions {
    public static func install() {
        CrepusActions.dispatch = { action in
            if let host = dispatchHostAction(action) {
                return host
            }
            return action.withCString { pointer in
                let capacity = 4096
                let output = UnsafeMutablePointer<CChar>.allocate(capacity: capacity)
                defer { output.deallocate() }
                let written = crepusMobileDispatchJson(pointer, UInt(strlen(pointer)), output, UInt(capacity))
                if written >= capacity {
                    return oversizedResultJson(action: action)
                }
                return String(cString: output)
            }
        }
        CrepusActions.resultSink = { result in
            CrepusStateStore.shared.applyResult(result)
        }
    }

    public static func dispatchStored(_ action: String) -> String {
        action.withCString { pointer in
            let capacity = 4096
            let output = UnsafeMutablePointer<CChar>.allocate(capacity: capacity)
            defer { output.deallocate() }
            let written = crepusMobileDispatchAndStoreJson(pointer, UInt(strlen(pointer)), output, UInt(capacity))
            if written >= capacity {
                return oversizedResultJson(action: action)
            }
            return String(cString: output)
        }
    }

    public static func dispatchChangeStored(_ action: String, bind: String, value: Any) -> String {
        guard let valueJson = encodeJsonValue(value) else {
            return "{\"ok\":false,\"error\":\"json encode failure\"}"
        }
        return action.withCString { actionPointer in
            bind.withCString { bindPointer in
                valueJson.withCString { valuePointer in
                    let capacity = 4096
                    let output = UnsafeMutablePointer<CChar>.allocate(capacity: capacity)
                    defer { output.deallocate() }
                    let written = crepusMobileDispatchChangeJson(
                        actionPointer,
                        UInt(strlen(actionPointer)),
                        bindPointer,
                        UInt(strlen(bindPointer)),
                        valuePointer,
                        UInt(strlen(valuePointer)),
                        output,
                        UInt(capacity)
                    )
                    if written >= capacity {
                        return oversizedResultJson(action: action)
                    }
                    return String(cString: output)
                }
            }
        }
    }

    public static func lastError() -> String? {
        guard let pointer = crepusMobileLastError() else {
            return nil
        }
        defer { crepusMobileFreeString(pointer) }
        let value = String(cString: pointer)
        return value.isEmpty ? nil : value
    }

    public static func lastResult() -> String {
        guard let pointer = crepusMobileLastResult() else {
            return "{}"
        }
        defer { crepusMobileFreeString(pointer) }
        return String(cString: pointer)
    }

    private static func oversizedResultJson(action: String) -> String {
        let payload: [String: Any] = [
            "ok": false,
            "action": action,
            "error": "action result too large",
        ]
        if let data = try? JSONSerialization.data(withJSONObject: payload),
           let json = String(data: data, encoding: .utf8) {
            return json
        }
        return "{\"ok\":false,\"error\":\"action result too large\"}"
    }

    private static func encodeJsonValue(_ value: Any) -> String? {
        switch value {
        case let text as String:
            guard let data = try? JSONSerialization.data(withJSONObject: [text]),
                  let json = String(data: data, encoding: .utf8)
            else {
                return nil
            }
            return String(json.dropFirst().dropLast())
        case let number as NSNumber:
            return number.stringValue
        case is NSNull:
            return "null"
        default:
            guard JSONSerialization.isValidJSONObject(value),
                  let data = try? JSONSerialization.data(withJSONObject: value),
                  let json = String(data: data, encoding: .utf8)
            else {
                return nil
            }
            return json
        }
    }

    private static func dispatchHostAction(_ action: String) -> String? {
        if let named = dispatchNamedHostAction(action) {
            return named
        }
        guard let data = action.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              (root["kind"] as? String) == "plugin",
              let capability = root["capability"] as? String,
              let method = root["method"] as? String
        else {
            return nil
        }
        if capability == "app" || capability == "device" || capability == "preferences" {
            return nil
        }
        let payload = root["payload"] as? [String: Any]
        do {
            let value = try hostPluginValue(capability: capability, method: method, payload: payload)
            return successJson(action: "\(capability).\(method)", capability: capability, method: method, value: value)
        } catch {
            return errorJson(action: "\(capability).\(method)", error: error.localizedDescription)
        }
    }

    private static func dispatchNamedHostAction(_ action: String) -> String? {
        switch action {
        case "import_files":
            presentFilePicker(action: action, contentTypes: [], allowsMultiple: true)
            return pendingJson(action: action)
        default:
            return nil
        }
    }

    private static func hostPluginValue(capability: String, method: String, payload: [String: Any]?) throws -> Any {
        switch capability {
        default:
            throw HostActionError("unsupported host capability: \(capability)")
        }
    }

    private static func successJson(action: String, capability: String, method: String, value: Any) -> String {
        let payload: [String: Any] = [
            "ok": true,
            "action": action,
            "value": [
                "capability": capability,
                "method": method,
                "value": value,
            ],
        ]
        return stringify(payload)
    }

    private static func errorJson(action: String, error: String) -> String {
        stringify([
            "ok": false,
            "action": action,
            "error": error,
        ])
    }

    private static func pendingJson(action: String) -> String {
        stringify([
            "ok": true,
            "action": action,
            "pending": true,
        ])
    }

    fileprivate static func emit(_ result: String) {
        Task { @MainActor in
            CrepusActions.resultSink(result)
        }
    }

    private static func stringify(_ payload: [String: Any]) -> String {
        if let data = try? JSONSerialization.data(withJSONObject: payload),
           let json = String(data: data, encoding: .utf8) {
            return json
        }
        return "{\"ok\":false,\"error\":\"json encode failure\"}"
    }
}

@MainActor
public final class CrepusStateStore: ObservableObject {
    public static let shared = CrepusStateStore()

    @Published public private(set) var revision: UInt64 = 0

    public func applyResult(_ json: String) {
        let stored = json.withCString { pointer in
            crepusMobileStoreResultJson(pointer, UInt(strlen(pointer)))
        }
        if stored {
            revision &+= 1
        }
    }

    public func text(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> String {
        _ = revision
        return readString(expr: expr, scopeName: scopeName, scopeJson: scopeJson(scope), reader: crepusMobileEvalText)
    }

    public func bool(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Bool {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalBool(exprPointer, UInt(strlen(exprPointer)), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
            }
        }
    }

    public func number(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Double {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalNumber(exprPointer, UInt(strlen(exprPointer)), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
            }
        }
    }

    public func items(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> [Any] {
        _ = revision
        let json = readString(expr: expr, scopeName: scopeName, scopeJson: scopeJson(scope), reader: crepusMobileEvalItemsJson)
        guard let data = json.data(using: .utf8),
              let array = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else {
            return []
        }
        return array
    }

    private func scopeJson(_ scope: Any?) -> String? {
        guard let scope else {
            return nil
        }
        switch scope {
        case let text as String:
            return "\"\(text.replacingOccurrences(of: "\"", with: "\\\""))\""
        case let number as NSNumber:
            return number.stringValue
        case is NSNull:
            return "null"
        default:
            guard JSONSerialization.isValidJSONObject(scope),
                  let data = try? JSONSerialization.data(withJSONObject: scope),
                  let json = String(data: data, encoding: .utf8)
            else {
                return nil
            }
            return json
        }
    }

    private func readString(
        expr: String,
        scopeName: String?,
        scopeJson: String?,
        reader: (UnsafePointer<CChar>, UInt, UnsafePointer<CChar>?, UInt, UnsafePointer<CChar>?, UInt, UnsafeMutablePointer<CChar>, UInt) -> UInt
    ) -> String {
        expr.withCString { exprPointer in
            let capacity = 4096
            let output = UnsafeMutablePointer<CChar>.allocate(capacity: capacity)
            defer { output.deallocate() }
            let written = callOptionalArgs(scopeName, scopeJson) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                reader(exprPointer, UInt(strlen(exprPointer)), scopeNamePointer, scopeNameLength, scopePointer, scopeLength, output, UInt(capacity))
            }
            if written >= capacity {
                return ""
            }
            return String(cString: output)
        }
    }

    private func callOptionalArgs<T>(
        _ scopeName: String?,
        _ scopeJson: String?,
        body: (UnsafePointer<CChar>?, UInt, UnsafePointer<CChar>?, UInt) -> T
    ) -> T {
        if let scopeName {
            return scopeName.withCString { scopeNamePointer in
                if let scopeJson {
                    return scopeJson.withCString { scopePointer in
                        body(scopeNamePointer, UInt(strlen(scopeNamePointer)), scopePointer, UInt(strlen(scopePointer)))
                    }
                }
                return body(scopeNamePointer, UInt(strlen(scopeNamePointer)), nil, 0)
            }
        }
        if let scopeJson {
            return scopeJson.withCString { scopePointer in
                body(nil, 0, scopePointer, UInt(strlen(scopePointer)))
            }
        }
        return body(nil, 0, nil, 0)
    }
}

private func currentArchitecture() -> String {
    #if arch(arm64)
    return "arm64"
    #elseif arch(x86_64)
    return "x86_64"
    #else
    return "unknown"
    #endif
}

private struct HostActionError: LocalizedError {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    var errorDescription: String? { message }
}

#if canImport(UIKit)
private final class FilePickerDelegate: NSObject, UIDocumentPickerDelegate {
    let action: String

    init(action: String) {
        self.action = action
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        let files = urls.compactMap { pickedFileJson(url: $0) }
        CrepusRustActions.emit(filePickerResultJson(action: action, files: files))
        CrepusHostPicker.shared.clear(delegate: self)
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        CrepusRustActions.emit(filePickerResultJson(action: action, files: []))
        CrepusHostPicker.shared.clear(delegate: self)
    }
}

private final class CrepusHostPicker {
    static let shared = CrepusHostPicker()
    private var delegates: [FilePickerDelegate] = []

    func retain(delegate: FilePickerDelegate) {
        delegates.append(delegate)
    }

    func clear(delegate: FilePickerDelegate) {
        delegates.removeAll { $0 === delegate }
    }
}

private func presentFilePicker(action: String, contentTypes: [UTType], allowsMultiple: Bool) {
    Task { @MainActor in
        guard let root = topViewController() else {
            CrepusRustActions.emit("{\"ok\":false,\"action\":\"\(action)\",\"error\":\"missing root view controller\"}")
            return
        }
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: contentTypes)
        let delegate = FilePickerDelegate(action: action)
        picker.delegate = delegate
        picker.allowsMultipleSelection = allowsMultiple
        CrepusHostPicker.shared.retain(delegate: delegate)
        root.present(picker, animated: true)
    }
}

private func topViewController() -> UIViewController? {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    let root = scenes
        .flatMap(\.windows)
        .first(where: \.isKeyWindow)?
        .rootViewController
    var current = root
    while let presented = current?.presentedViewController {
        current = presented
    }
    return current
}

private func pickedFileJson(url: URL) -> [String: Any]? {
    let values = try? url.resourceValues(forKeys: [.nameKey, .contentTypeKey, .fileSizeKey])
    let name = values?.name ?? url.lastPathComponent
    guard let path = try? copyToCache(from: url, name: name) else { return nil }
    return [
        "name": name,
        "mimeType": values?.contentType?.preferredMIMEType ?? "application/octet-stream",
        "bytes": values?.fileSize ?? (try? path.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0,
        "filePath": path.path,
        "importSource": "ios-document-picker",
    ]
}

private func copyToCache(from url: URL, name: String) throws -> URL {
    let ext = (name as NSString).pathExtension
    let path = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString)
        .appendingPathExtension(ext.isEmpty ? url.pathExtension : ext)
    let accessing = url.startAccessingSecurityScopedResource()
    defer {
        if accessing {
            url.stopAccessingSecurityScopedResource()
        }
    }
    try FileManager.default.copyItem(at: url, to: path)
    return path
}

private func filePickerResultJson(action: String, files: [[String: Any]]) -> String {
    if let data = try? JSONSerialization.data(withJSONObject: [
        "ok": true,
        "action": action,
        "value": [
            "files": files,
        ],
    ]),
       let json = String(data: data, encoding: .utf8) {
        return json
    }
    return "{\"ok\":false,\"action\":\"\(action)\",\"error\":\"json encode failure\"}"
}

private func mediaResultJson(action: String, files: [[String: Any]]) -> String {
    if let data = try? JSONSerialization.data(withJSONObject: [
        "ok": true,
        "action": action,
        "value": [
            "files": files,
        ],
    ]),
       let json = String(data: data, encoding: .utf8) {
        return json
    }
    return "{\"ok\":false,\"action\":\"\(action)\",\"error\":\"json encode failure\"}"
}

private func fileExtension(_ type: String) -> String {
    let type = type.lowercased()
    if type.contains("png") { return "png" }
    if type.contains("heic") || type.contains("heif") { return "heic" }
    if type.contains("movie") || type.contains("video") || type.contains("mpeg-4") { return "mp4" }
    return "jpg"
}

private func mimeType(_ type: String) -> String {
    let type = type.lowercased()
    if type.contains("png") { return "image/png" }
    if type.contains("heic") || type.contains("heif") { return "image/heic" }
    if type.contains("movie") || type.contains("video") || type.contains("mp4") || type.contains("mpeg-4") { return "video/mp4" }
    return "image/jpeg"
}

#elseif canImport(AppKit)
private func presentFilePicker(action: String, contentTypes: [Any], allowsMultiple: Bool) {
    Task { @MainActor in
        CrepusRustActions.emit("{\"ok\":false,\"action\":\"\(action)\",\"error\":\"file picker unavailable on AppKit shell\"}")
    }
}

#else
#endif

@MainActor
public final class CrepusActionStore: ObservableObject {
    public static let shared = CrepusActionStore()

    @Published public private(set) var lastResult: String = "{}"
    @Published public private(set) var lastError: String?

    public func dispatch(_ action: String) {
        record(CrepusRustActions.dispatchStored(action))
    }

    public func record(_ result: String) {
        lastResult = CrepusRustActions.lastResult()
        lastError = CrepusRustActions.lastError()
    }
}
