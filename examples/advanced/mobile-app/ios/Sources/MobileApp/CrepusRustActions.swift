import Foundation

// ── Rust FFI declarations ─────────────────────────────────────────────────────

@_silgen_name("crepus_mobile_dispatch_and_store_nul")
private func crepusMobileDispatchAndStoreNul(_ action: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?
@_silgen_name("crepus_mobile_dispatch_change_and_store_nul")
private func crepusMobileDispatchChangeAndStoreNul(_ action: UnsafePointer<CChar>, _ bind: UnsafePointer<CChar>, _ value: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?
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

// ── Action bridge ─────────────────────────────────────────────────────────────

private let actionQueue = DispatchQueue(label: "dev.crepuscularity.mobile-app.actions", qos: .userInitiated)

@MainActor
public enum CrepusRustActions {
    /// Wire `CrepusActions.dispatch` to the Rust FFI and apply results.
    public static func install() {
        CrepusActions.dispatch = { action in
            actionQueue.sync {
                dispatchStored(action)
            }
        }
        CrepusActions.resultSink = { result in
            CrepusActions.applyResult(result)
        }
    }

    /// Dispatch an action string to Rust and return the JSON result.
    public static func dispatchStored(_ action: String) -> String {
        action.withCString { pointer in
            guard let output = crepusMobileDispatchAndStoreNul(pointer) else {
                return "{}"
            }
            defer { crepusMobileFreeString(output) }
            return String(cString: output)
        }
    }

    /// Dispatch a bind change to Rust and return the JSON result.
    public static func dispatchChangeStored(_ action: String, bind: String, value: Any) -> String {
        guard let valueJson = encodeJsonValue(value) else {
            return "{\"ok\":false,\"error\":\"json encode failure\"}"
        }
        return action.withCString { actionPointer in
            bind.withCString { bindPointer in
                valueJson.withCString { valuePointer in
                    guard let output = crepusMobileDispatchChangeAndStoreNul(actionPointer, bindPointer, valuePointer) else {
                        return "{}"
                    }
                    defer { crepusMobileFreeString(output) }
                    return String(cString: output)
                }
            }
        }
    }

    private static func encodeJsonValue(_ value: Any) -> String? {
        if let number = value as? NSNumber, !number.doubleValue.isFinite { return nil }
        guard JSONSerialization.isValidJSONObject([value]),
              let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]),
              let json = String(data: data, encoding: .utf8) else { return nil }
        return json
    }
}
