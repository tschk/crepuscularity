import Foundation
import SwiftUI

/// Reactive state store backed by Rust FFI eval functions.
/// Bumps `revision` on every `applyResult` call, triggering SwiftUI re-renders.
@MainActor
public final class CrepusStateStore: ObservableObject {
    public static let shared = CrepusStateStore()

    @Published public private(set) var revision: UInt64 = 0

    public func refresh() {
        withAnimation(.easeInOut(duration: 0.22)) {
            revision &+= 1
        }
    }

    /// Store a JSON result in Rust and bump revision to trigger re-render.
    public func applyResult(_ json: String) {
        let stored = json.withCString { pointer in
            crepusMobileStoreResultJson(pointer, UInt(strlen(pointer)))
        }
        if stored {
            if Thread.isMainThread {
                refresh()
            } else {
                DispatchQueue.main.async {
                    self.refresh()
                }
            }
        }
    }

    /// Read a text value from Rust state.
    public func text(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> String {
        _ = revision
        return readString(expr: expr, scopeName: scopeName, scopeJson: scopeJson(scope), reader: crepusMobileEvalText)
    }

    /// Read a boolean value from Rust state.
    public func bool(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Bool {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalBool(exprPointer, UInt(strlen(exprPointer)), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
            }
        }
    }

    /// Read a numeric value from Rust state.
    public func number(_ expr: String, scopeName: String? = nil, scope: Any? = nil) -> Double {
        _ = revision
        return expr.withCString { exprPointer in
            callOptionalArgs(scopeName, scopeJson(scope)) { scopeNamePointer, scopeNameLength, scopePointer, scopeLength in
                crepusMobileEvalNumber(exprPointer, UInt(strlen(exprPointer)), scopeNamePointer, scopeNameLength, scopePointer, scopeLength)
            }
        }
    }

    /// Read a list of items from Rust state (returns array of dictionaries).
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

    // MARK: - Private helpers

    private func scopeJson(_ scope: Any?) -> String? {
        guard let scope else { return nil }
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
            else { return nil }
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

    private func callOptionalArgs<T>(_ scopeName: String?, _ scopeJson: String?, body: (UnsafePointer<CChar>?, UInt, UnsafePointer<CChar>?, UInt) -> T) -> T {
        if let scopeName {
            return scopeName.withCString { scopeNamePointer in
                if let scopeJson {
                    return scopeJson.withCString { pointer in
                        body(scopeNamePointer, UInt(strlen(scopeNamePointer)), pointer, UInt(strlen(pointer)))
                    }
                }
                return body(scopeNamePointer, UInt(strlen(scopeNamePointer)), nil, 0)
            }
        }
        guard let scopeJson else {
            return body(nil, 0, nil, 0)
        }
        return scopeJson.withCString { pointer in
            body(nil, 0, pointer, UInt(strlen(pointer)))
        }
    }
}
