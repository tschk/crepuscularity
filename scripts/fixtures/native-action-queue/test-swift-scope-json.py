#!/usr/bin/env python3
"""Exercise actual Swift state and dispatch transport against labelled FFI stubs.

No Cargo or Rust fixture required. Readers validate decoded JSON and byte lengths;
this gate proves Swift transport, not Rust expression evaluation. --host supports
running exactly the same regression against a preserved pre-fix source.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host', type=Path, default=root / 'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift')
args = parser.parse_args()
host = args.host.read_text()
start = host.index('@MainActor\npublic final class CrepusStateStore')
model = host[start:host.index('\nprivate func currentArchitecture()', start)]
start = host.index('    public static func dispatchStored')
codec = host[start:host.index('    public static func lastError', start)]
start = host.index('    private nonisolated static func decodeOwnedResult')
codec += host[start:host.index('    private static func dispatchHostAction', start)]
source = 'import Foundation\nimport SwiftUI\nimport Darwin\n@MainActor enum CrepusRustActions {\n' + codec + '}\n' + model
example = (root / 'examples/advanced/mobile-app/ios/Sources/MobileApp/CrepusRustActions.swift').read_text()
start = example.index('    private static func encodeJsonValue')
codec = example[start:example.rindex('\n}')].replace('private static func', 'static func', 1)
source += '@MainActor enum ExampleCodec {\n' + codec + '}\n'
example_reader = (root / 'examples/advanced/mobile-app/ios/Sources/MobileApp/CrepusStateStore.swift').read_text()
start = example_reader.index('    private func readString(')
reader_methods = example_reader[start:example_reader.rindex('\n}')].replace('private func ', 'static func ')
source += '@MainActor enum ExampleReader {\n' + reader_methods + '}\n'

source += r'''
// Only FFI calls are stubs. No production serialization is copied here.
var expected: Any = NSNull()
var reply = "checked"
var itemsReply = "[]"
var expectedExpr = "item"
var expectedName: String? = "item"
var hasScope = true
var expectedResult = ""
var storedResult: Data?
var expectedAction = "sync\u{0}tail雪"
var expectedBind = "title\u{0}tail雪"
var expectedValue = ""
var expectedChangeValue: Any?
var actionCalls = 0
var changeCalls = 0
var freeCalls = 0
func crepusMobileFreeString(_ pointer: UnsafeMutablePointer<CChar>?) { freeCalls += 1; free(pointer) }
func crepusMobileDispatchJsonOwned(_ p: UnsafePointer<CChar>, _ n: UInt) -> UnsafeMutablePointer<CChar>? {
    precondition(Data(bytes: p, count: Int(n)) == Data(expectedAction.utf8), "action UTF-8 truncated")
    actionCalls += 1
    return strdup("{}")
}
func crepusMobileDispatchChangeJsonOwned(_ action: UnsafePointer<CChar>, _ an: UInt,
                                   _ bind: UnsafePointer<CChar>, _ bn: UInt,
                                   _ value: UnsafePointer<CChar>, _ vn: UInt) -> UnsafeMutablePointer<CChar>? {
    precondition(Data(bytes: action, count: Int(an)) == Data(expectedAction.utf8), "change action UTF-8 truncated")
    precondition(Data(bytes: bind, count: Int(bn)) == Data(expectedBind.utf8), "binding UTF-8 truncated")
    let bytes = Data(bytes: value, count: Int(vn))
    if let expectedChangeValue {
        guard let decoded = try? JSONSerialization.jsonObject(with: bytes, options: [.fragmentsAllowed]) else {
            fatalError("invalid change JSON")
        }
        precondition((decoded as AnyObject).isEqual(expectedChangeValue), "change value changed")
        if let number = expectedChangeValue as? NSNumber {
            precondition((CFGetTypeID(decoded as CFTypeRef) == CFBooleanGetTypeID()) ==
                         (CFGetTypeID(number) == CFBooleanGetTypeID()), "change Boolean/number type changed")
        }
    } else {
        precondition(bytes == Data(expectedValue.utf8), "change JSON UTF-8 truncated")
    }
    changeCalls += 1
    return strdup("{}")
}
func checkScope(_ expr: UnsafePointer<CChar>, _ exprLength: UInt,
                _ name: UnsafePointer<CChar>?, _ nameLength: UInt,
                _ scope: UnsafePointer<CChar>?, _ scopeLength: UInt) {
    precondition(Data(bytes: expr, count: Int(exprLength)) == Data(expectedExpr.utf8), "expression UTF-8 truncated")
    if let expectedName {
        guard let name else { fatalError("missing scope name") }
        precondition(Data(bytes: name, count: Int(nameLength)) == Data(expectedName.utf8), "scope name UTF-8 truncated")
    } else {
        precondition(name == nil && nameLength == 0)
    }
    if !hasScope {
        precondition(scope == nil && scopeLength == 0)
        return
    }
    guard let scope else { fatalError("missing scope JSON") }
    let bytes = Data(bytes: scope, count: Int(scopeLength))
    guard let decoded = try? JSONSerialization.jsonObject(with: bytes, options: [.fragmentsAllowed]) else {
        fatalError("invalid scope JSON: \(String(decoding: bytes, as: UTF8.self))")
    }
    precondition((decoded as AnyObject).isEqual(expected), "scope value changed: \(decoded) != \(expected)")
    if let boolean = expected as? NSNumber, CFGetTypeID(boolean) == CFBooleanGetTypeID() {
        precondition(CFGetTypeID(decoded as CFTypeRef) == CFBooleanGetTypeID(), "bool became number")
    }
}
func crepusMobileStoreResultJson(_ p: UnsafePointer<CChar>, _ n: UInt) -> Bool {
    let bytes = Data(bytes: p, count: Int(n))
    precondition(bytes == Data(expectedResult.utf8), "result JSON UTF-8 truncated")
    storedResult = bytes
    // Labelled rejecting backend: verify invalid suffixes reach FFI intact.
    return false
}
'''
source += r'''
func writeReply(_ value: String, _ output: UnsafeMutablePointer<CChar>, _ capacity: UInt) -> UInt {
    let bytes = Array(value.utf8)
    if capacity > 0 {
        let count = min(bytes.count, Int(capacity) - 1)
        if count > 0 { bytes.withUnsafeBytes { raw in _ = memcpy(output, raw.baseAddress!, count) } }
        output[count] = 0
    }
    return UInt(bytes.count)
}
'''
# Match the production FFI signatures; each reader validates the same scope.
params = '_ e: UnsafePointer<CChar>, _ n: UInt, _ a: UnsafePointer<CChar>?, _ an: UInt, _ s: UnsafePointer<CChar>?, _ sn: UInt'
for name, result, body in [
    ('Bool', 'Bool', 'return true'), ('Number', 'Double', 'return 1.5'),
    ('Text', 'UInt', 'return writeReply(reply, o, capacity)'),
    ('ItemsJson', 'UInt', 'return writeReply(itemsReply, o, capacity)'),
]:
    buffer = ', _ o: UnsafeMutablePointer<CChar>, _ capacity: UInt' if result == 'UInt' else ''
    source += f'func crepusMobileEval{name}({params}{buffer}) -> {result} {{ checkScope(e,n,a,an,s,sn); {body} }}\n'
source += r'''
@main struct ScopeJsonHarness {
    @MainActor static func main() {
        let model = CrepusStateStore.shared
        let strings = ["plain", "a\\b", "\\n", "line\nfeed", "\"quoted\"", "\u{0}\u{1}\u{8}\u{c}\t\r", "雪😀é"]
        let values: [Any] = strings + [42, -7, 1.5, true, false, NSNull(),
            ["value": "a\\b\n\"雪", "nested": [true, NSNull()]],
            ["a\\b", "line\nfeed", 1.5, false, NSNull()]]
        for value in values {
            expected = value
            precondition(model.text("item", scopeName: "item", scope: value) == "checked")
            precondition(model.bool("item", scopeName: "item", scope: value))
            precondition(model.number("item", scopeName: "item", scope: value) == 1.5)
            precondition(model.items("item", scopeName: "item", scope: value).isEmpty)
        }
        reply = "prefix\u{0}雪suffix"
        precondition(model.text("item", scopeName: "item", scope: expected) == reply,
                     "readString ignored returned UTF-8 byte length")
        for count in [0, 4095, 4096, 4097, 16384] {
            for value in [String(repeating: "x", count: count), String(repeating: "x", count: count) + "雪\u{0}😀"] {
                reply = value
                precondition(model.text("item", scopeName: "item", scope: expected) == reply,
                             "large UTF-8 text disappeared")
                precondition(ExampleReader.readString(expr: "item", scopeName: "item",
                             scopeJson: CrepusRustActions.encodeQueuedChangeValue(expected),
                             reader: crepusMobileEvalText) == reply, "example text disappeared")
            }
        }
        let largeItems = (0..<1000).map { "item-\($0)-雪" }
        itemsReply = String(data: try! JSONSerialization.data(withJSONObject: largeItems), encoding: .utf8)!
        precondition(model.items("item", scopeName: "item", scope: expected) as? [String] == largeItems,
                     "large JSON list disappeared")
        let exampleItems = ExampleReader.readString(expr: "item", scopeName: "item",
                           scopeJson: CrepusRustActions.encodeQueuedChangeValue(expected),
                           reader: crepusMobileEvalItemsJson)
        precondition(exampleItems == itemsReply, "example JSON list disappeared")
        itemsReply = "[]"
        reply = "checked"
        for (expr, name, scoped) in [
            ("item\u{0}雪suffix", "item" as String?, true),
            ("item", "item\u{0}雪suffix" as String?, true),
            ("雪\u{0}expr", "名\u{0}scope" as String?, true),
            ("item", nil, true),
            ("item", "item" as String?, false),
            ("item", nil, false),
        ] {
            expectedExpr = expr; expectedName = name; hasScope = scoped
            let scope: Any? = scoped ? expected : nil
            precondition(model.text(expr, scopeName: name, scope: scope) == reply)
            precondition(model.bool(expr, scopeName: name, scope: scope))
            precondition(model.number(expr, scopeName: name, scope: scope) == 1.5)
            precondition(model.items(expr, scopeName: name, scope: scope).isEmpty)
        }
        for json in ["{\"ok\":true}\u{0}雪", "{\"ok\":true} trailing雪"] {
            expectedResult = json
            let before = model.revision
            model.applyResult(json)
            precondition(storedResult == Data(json.utf8) && model.revision == before)
        }
        precondition(CrepusRustActions.dispatchStored(expectedAction) == "{}")
        let changedText = "text\u{0}雪\\\""
        expectedValue = CrepusRustActions.encodeQueuedChangeValue(changedText)!
        precondition(CrepusRustActions.dispatchChangeStored(expectedAction, bind: expectedBind, value: changedText) == "{}")
        // Queued raw JSON must also reach the backend intact, including invalid suffixes.
        expectedValue = "\"text\"\u{0}雪"
        precondition(CrepusRustActions.dispatchQueuedChange(expectedAction, bind: expectedBind, valueJson: expectedValue) == "{}")
        precondition(actionCalls == 1 && changeCalls == 2)
        for value in values {
            let exampleJson = ExampleCodec.encodeJsonValue(value)!
            let exampleValue = try! JSONSerialization.jsonObject(with: Data(exampleJson.utf8), options: [.fragmentsAllowed])
            precondition((exampleValue as AnyObject).isEqual(value))
            if let number = value as? NSNumber {
                precondition((CFGetTypeID(exampleValue as CFTypeRef) == CFBooleanGetTypeID()) ==
                             (CFGetTypeID(number) == CFBooleanGetTypeID()), "example Boolean/number type changed")
            }
            expectedChangeValue = value
            precondition(CrepusRustActions.dispatchChangeStored(expectedAction, bind: expectedBind, value: value) == "{}")
        }
        expectedChangeValue = nil
        let beforeInvalid = changeCalls
        for invalid: Any in [Double.nan, Double.infinity, -Double.infinity, ["nested": Double.infinity], Date()] {
            precondition(ExampleCodec.encodeJsonValue(invalid) == nil)
            let result = CrepusRustActions.dispatchChangeStored(expectedAction, bind: expectedBind, value: invalid)
            let decoded = try! JSONSerialization.jsonObject(with: Data(result.utf8)) as! [String: Any]
            precondition(decoded["ok"] as? Bool == false)
        }
        precondition(changeCalls == beforeInvalid, "invalid change reached FFI")
        precondition(freeCalls == actionCalls + changeCalls, "owned result leaked")
        print("PASS actual Swift StateStore: scope JSON, exact input/output UTF-8 lengths, synchronous change JSON types, optional reader args and rejected result transport (FFI stubs)")
    }
}
'''
source = 'import CoreFoundation\n' + source
with tempfile.TemporaryDirectory(prefix='crepus-swift-scope-json-') as directory:
    directory = Path(directory)
    swift = directory / 'ScopeJson.swift'
    swift.write_text(source)
    binary = directory / 'scope-json'
    subprocess.run(['swiftc', '-swift-version', '5', '-warnings-as-errors', '-parse-as-library', str(swift), '-o', str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=15)
