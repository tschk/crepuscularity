// Mock native backend only. Production draft store, queue and JSON bridge are extracted.
final class NativeProbe {
    let lock = NSLock()
    let entered = DispatchSemaphore(value: 0)
    let release = DispatchSemaphore(value: 0)
    var values: [String: Any] = [:]
    var calls = 0
    var frees = 0
    var textScope: Any?
    var numberResult = 0.0
    var numberScope = ""
}
let nativeProbe = NativeProbe()
func crepusMobileDispatchChangeJsonOwned(
    _ action: UnsafePointer<CChar>, _ actionLength: UInt,
    _ bind: UnsafePointer<CChar>, _ bindLength: UInt,
    _ value: UnsafePointer<CChar>, _ valueLength: UInt
) -> UnsafeMutablePointer<CChar>? {
    precondition(!Thread.isMainThread)
    let action = String(cString: action)
    let path = String(cString: bind)
    let value = try! JSONSerialization.jsonObject(with: Data(String(cString: value).utf8), options: [.fragmentsAllowed])
    nativeProbe.lock.lock()
    if path == "items.0.name", let text = value as? String {
        if text == "ab" { precondition(nativeProbe.values[path] as? String == "a") }
        if action == "upper" { precondition(nativeProbe.values[path] as? String == "ab") }
    }
    nativeProbe.calls += 1
    nativeProbe.lock.unlock()
    if action == "blocked" {
        nativeProbe.entered.signal()
        precondition(nativeProbe.release.wait(timeout: .now() + 3) == .success)
    }
    let response: [String: Any] = action == "fail"
        ? ["ok": false, "error": "rejected"]
        : ["ok": true, "bindings": [path: action == "upper" ? (value as! String).uppercased() : value]]
    let data = try! JSONSerialization.data(withJSONObject: response)
    return strdup(String(decoding: data, as: UTF8.self))
}
func crepusMobileFreeString(_ pointer: UnsafeMutablePointer<CChar>?) {
    nativeProbe.lock.lock(); nativeProbe.frees += 1; nativeProbe.lock.unlock()
    free(pointer)
}

func crepusMobileStoreResultJson(_ pointer: UnsafePointer<CChar>, _ length: UInt) -> Bool {
    precondition(Thread.isMainThread)
    let result = try! JSONSerialization.jsonObject(with: Data(String(cString: pointer).utf8)) as! [String: Any]
    if result["ok"] as? Bool != false, let fields = result["bindings"] as? [String: Any] {
        nativeProbe.lock.lock(); defer { nativeProbe.lock.unlock() }
        nativeProbe.values.merge(fields) { _, new in new }
    }
    return true
}

// Formatting itself is checked against real Rust separately; this stub verifies forwarding.
func crepusMobileEvalText(
    _ expr: UnsafePointer<CChar>, _ exprLength: UInt,
    _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt,
    _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt,
    _ output: UnsafeMutablePointer<CChar>, _ capacity: UInt
) -> UInt {
    precondition(String(cString: expr) == "__crepusDraftValue")
    precondition(scopeName != nil && String(cString: scopeName!) == "__crepusDraftValue")
    precondition(scope != nil)
    let value = try! JSONSerialization.jsonObject(with: Data(bytes: scope!, count: Int(scopeLength)), options: [.fragmentsAllowed])
    precondition(!(value is String), "strings must stay on the local fast path")
    precondition((value as! NSObject).isEqual(nativeProbe.textScope!))
    let result = Array("formatted by Rust".utf8)
    for (index, byte) in result.prefix(Int(capacity) - 1).enumerated() { output[index] = CChar(bitPattern: byte) }
    output[min(result.count, Int(capacity) - 1)] = 0
    return UInt(result.count)
}

func crepusMobileEvalNumber(
    _ expr: UnsafePointer<CChar>, _ exprLength: UInt,
    _ scopeName: UnsafePointer<CChar>?, _ scopeNameLength: UInt,
    _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt
) -> Double {
    precondition(String(cString: expr) == "__crepusDraftValue")
    precondition(scopeName != nil && String(cString: scopeName!) == "__crepusDraftValue")
    precondition(scope != nil)
    let value = try! JSONSerialization.jsonObject(with: Data(bytes: scope!, count: Int(scopeLength)), options: [.fragmentsAllowed])
    precondition(value is String, "numeric drafts must stay local")
    precondition(value as? String == nativeProbe.numberScope)
    return nativeProbe.numberResult
}
