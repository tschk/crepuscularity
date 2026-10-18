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

@_silgen_name("crepus_mobile_dispatch_json_owned")
private func crepusMobileDispatchJsonOwned(_ action: UnsafePointer<CChar>, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("crepus_mobile_dispatch_change_json_owned")
private func crepusMobileDispatchChangeJsonOwned(_ action: UnsafePointer<CChar>, _ actionLength: UInt, _ bind: UnsafePointer<CChar>, _ bindLength: UInt, _ value: UnsafePointer<CChar>, _ valueLength: UInt) -> UnsafeMutablePointer<CChar>?
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
        CrepusActions.dispatch = defaultDispatch
        CrepusActions.resultSink = { result in
            CrepusStateStore.shared.applyResult(result)
        }
    }

    private nonisolated static func defaultDispatch(_ action: String) -> String {
        let host: String?
        if Thread.isMainThread {
            host = MainActor.assumeIsolated { dispatchHostAction(action) }
        } else {
            host = DispatchQueue.main.sync {
                MainActor.assumeIsolated { dispatchHostAction(action) }
            }
        }
        if let host { return host }
        return action.withCString { pointer in
            decodeOwnedResult(crepusMobileDispatchJsonOwned(pointer, UInt(action.utf8.count)))
        }
    }

    public static func dispatchStored(_ action: String) -> String {
        action.withCString { pointer in
            decodeOwnedResult(crepusMobileDispatchJsonOwned(pointer, UInt(action.utf8.count)))
        }
    }

    public static func dispatchChangeStored(_ action: String, bind: String, value: Any) -> String {
        guard let valueJson = encodeQueuedChangeValue(value) else {
            return "{\"ok\":false,\"error\":\"json encode failure\"}"
        }
        return dispatchChangeJson(action, bind: bind, valueJson: valueJson)
    }

    static func encodeQueuedChangeValue(_ value: Any) -> String? {
        if let number = value as? NSNumber, !number.doubleValue.isFinite { return nil }
        guard JSONSerialization.isValidJSONObject([value]),
              let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]),
              let json = String(data: data, encoding: .utf8) else { return nil }
        return json
    }

    nonisolated static func dispatchQueuedChange(_ action: String, bind: String, valueJson: String) -> String {
        if action.isEmpty {
            guard bind.split(separator: ".", maxSplits: 126, omittingEmptySubsequences: false).count <= 126 else {
                return "{\"ok\":false,\"error\":\"binding path exceeds 126 segments\"}"
            }
            guard let data = valueJson.data(using: .utf8),
                  let value = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]),
                  let result = try? JSONSerialization.data(withJSONObject: ["ok": true, "action": "bind", "bindings": [bind: value]]),
                  let json = String(data: result, encoding: .utf8) else {
                return "{\"ok\":false,\"error\":\"json encode failure\"}"
            }
            return json
        }
        return dispatchChangeJson(action, bind: bind, valueJson: valueJson)
    }

    private nonisolated static func dispatchChangeJson(_ action: String, bind: String, valueJson: String) -> String {
        return action.withCString { actionPointer in
            bind.withCString { bindPointer in
                valueJson.withCString { valuePointer in
                    decodeOwnedResult(crepusMobileDispatchChangeJsonOwned(
                        actionPointer, UInt(action.utf8.count),
                        bindPointer, UInt(bind.utf8.count),
                        valuePointer, UInt(valueJson.utf8.count)
                    ))
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

    private nonisolated static func decodeOwnedResult(_ pointer: UnsafeMutablePointer<CChar>?) -> String {
        guard let pointer else { return "{\"ok\":false,\"error\":\"action result allocation failed\"}" }
        defer { crepusMobileFreeString(pointer) }
        return String(cString: pointer)
    }

    private static func dispatchHostAction(_ action: String) -> String? {
        if let named = dispatchNamedHostAction(action) {
            return named
        }
        let root: [String: Any]
        if let data = action.data(using: .utf8),
           let request = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            root = request
        } else if let separator = action.firstIndex(of: ".") {
            root = [
                "kind": "plugin",
                "capability": String(action[..<separator]),
                "method": String(action[action.index(after: separator)...])
            ]
        } else {
            return nil
        }
        guard (root["kind"] as? String) == "plugin",
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

    fileprivate static func errorJson(action: String, error: String) -> String {
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

    fileprivate static func stringify(_ payload: [String: Any]) -> String {
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
            crepusMobileStoreResultJson(pointer, UInt(json.utf8.count))
        }
        if stored {
            revision &+= 1
        }
    }

    private struct Draft {
        let sequence: UInt64
        let value: Any
    }
    private var draftSequence: UInt64 = 0
    private var drafts: [String: Draft] = [:]

    func beginDraft(path: String, valueJson: String) -> UInt64? {
        guard let data = valueJson.data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else { return nil }
        draftSequence &+= 1
        drafts[path] = Draft(sequence: draftSequence, value: value)
        revision &+= 1
        return draftSequence
    }

    func finishDraft(path: String, sequence: UInt64) {
        if drafts[path]?.sequence == sequence {
            drafts.removeValue(forKey: path)
        }
        revision &+= 1
    }

    public func draftText(path: String, fallback: @autoclosure () -> String) -> String {
        _ = revision
        guard let value = drafts[path]?.value else { return fallback() }
        if let text = value as? String { return text }
        return text("__crepusDraftValue", scopeName: "__crepusDraftValue", scope: value)
    }

    public func draftBool(path: String, fallback: @autoclosure () -> Bool) -> Bool {
        _ = revision
        guard let value = drafts[path]?.value else { return fallback() }
        switch value {
        case is NSNull: return false
        case let number as NSNumber: return number.doubleValue != 0
        case let text as String: return !text.isEmpty
        case let items as [Any]: return !items.isEmpty
        case let object as [String: Any]: return !object.isEmpty
        default: return false
        }
    }

    public func draftNumber(path: String, fallback: @autoclosure () -> Double) -> Double {
        _ = revision
        guard let value = drafts[path]?.value else { return fallback() }
        if value is String {
            return number("__crepusDraftValue", scopeName: "__crepusDraftValue", scope: value)
        }
        guard let number = value as? NSNumber,
              CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite else { return 0 }
        return number.doubleValue
    }

    public func text(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> String {
        _ = revision
        return readString(expr: expr, scopeName: scopeName, scopeJson: scopeJson(scope), reader: crepusMobileEvalText)
    }

    public func bool(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Bool {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalBool(exprPointer, UInt(expr.utf8.count), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
            }
        }
    }

    public func number(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Double {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalNumber(exprPointer, UInt(expr.utf8.count), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
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
        guard let scope else { return nil }
        return CrepusRustActions.encodeQueuedChangeValue(scope)
    }

    private func readString(
        expr: String,
        scopeName: String?,
        scopeJson: String?,
        reader: (UnsafePointer<CChar>, UInt, UnsafePointer<CChar>?, UInt, UnsafePointer<CChar>?, UInt, UnsafeMutablePointer<CChar>, UInt) -> UInt
    ) -> String {
        expr.withCString { exprPointer in
            var capacity = 4096
            while true {
                let output = UnsafeMutablePointer<CChar>.allocate(capacity: capacity)
                defer { output.deallocate() }
                let written = callOptionalArgs(scopeName, scopeJson) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                    reader(exprPointer, UInt(expr.utf8.count), scopeNamePointer, scopeNameLength, scopePointer, scopeLength, output, UInt(capacity))
                }
                if written < UInt(capacity) {
                    return String(decoding: UnsafeRawBufferPointer(start: output, count: Int(written)), as: UTF8.self)
                }
                // Read-only evaluators report the full length even when the buffer is too small.
                guard written < UInt(Int.max) else { return "" }
                capacity = Int(written) + 1
            }
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
                        body(scopeNamePointer, UInt(scopeName.utf8.count), scopePointer, UInt(scopeJson.utf8.count))
                    }
                }
                return body(scopeNamePointer, UInt(scopeName.utf8.count), nil, 0)
            }
        }
        if let scopeJson {
            return scopeJson.withCString { scopePointer in
                body(nil, 0, scopePointer, UInt(scopeJson.utf8.count))
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
private let pickerImportQueue = DispatchQueue(label: "crepus.picker-import", qos: .userInitiated)

@MainActor
private final class FilePickerDelegate: NSObject, UIDocumentPickerDelegate {
    let action: String
    private var importing = false

    init(action: String) {
        self.action = action
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard !importing, !CrepusHostPicker.shared.importInFlight,
              CrepusHostPicker.shared.contains(delegate: self) else { return }
        importing = true
        CrepusHostPicker.shared.importInFlight = true
        let action = action
        pickerImportQueue.async { [weak self] in
            let files = urls.compactMap { pickedFileJson(url: $0) }
            let result = filePickerResultJson(action: action, files: files, failedCount: urls.count - files.count)
            let paths = files.compactMap { $0["filePath"] as? String }
            DispatchQueue.main.async {
                CrepusHostPicker.shared.importInFlight = false
                guard let self, CrepusHostPicker.shared.contains(delegate: self) else {
                    pickerImportQueue.async {
                        for path in paths { try? FileManager.default.removeItem(atPath: path) }
                    }
                    return
                }
                CrepusHostPicker.shared.clear(delegate: self)
                CrepusActions.resultSink(result)
            }
        }
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        guard CrepusHostPicker.shared.contains(delegate: self) else { return }
        CrepusHostPicker.shared.clear(delegate: self)
        CrepusActions.resultSink(filePickerResultJson(action: action, files: []))
    }
}

@MainActor
private final class CrepusHostPicker {
    static let shared = CrepusHostPicker()
    private var delegates: [NSObject] = []
    var importInFlight = false

    var isActive: Bool { importInFlight || !delegates.isEmpty }

    func retain(delegate: NSObject) {
        delegates.append(delegate)
    }

    func contains(delegate: NSObject) -> Bool {
        delegates.contains { $0 === delegate }
    }

    func clear(delegate: NSObject) {
        delegates.removeAll { $0 === delegate }
    }
}

private func presentFilePicker(action: String, contentTypes: [UTType], allowsMultiple: Bool) {
    Task { @MainActor in
        guard !CrepusHostPicker.shared.isActive else {
            CrepusRustActions.emit(CrepusRustActions.errorJson(action: action, error: "picker already active"))
            return
        }
        guard let root = topViewController() else {
            CrepusRustActions.emit(CrepusRustActions.errorJson(action: action, error: "missing root view controller"))
            return
        }
        let allowedTypes = contentTypes.isEmpty ? [UTType.item] : contentTypes
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: allowedTypes)
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
    let accessing = url.startAccessingSecurityScopedResource()
    defer {
        if accessing { url.stopAccessingSecurityScopedResource() }
    }
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
    do {
        try FileManager.default.copyItem(at: url, to: path)
        return path
    } catch {
        try? FileManager.default.removeItem(at: path)
        throw error
    }
}

private func filePickerResultJson(action: String, files: [[String: Any]], failedCount: Int = 0) -> String {
    var result: [String: Any] = [
        "ok": failedCount == 0,
        "action": action,
        "value": ["files": files],
    ]
    if failedCount > 0 {
        result["error"] = "could not import selected files"
        result["failedCount"] = failedCount
    }
    if let data = try? JSONSerialization.data(withJSONObject: result),
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
    return UTType(type)?.preferredMIMEType
        ?? UTType(filenameExtension: type)?.preferredMIMEType
        ?? "application/octet-stream"
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
