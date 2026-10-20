"""Extract production queue, JSON bridge and draft store; stub only native FFI.

The Swift fixture supplies mock Rust responses/commits. No queue/draft algorithm
is recreated in the harness. Platform UI and bool/items readers excluded; text/number FFI use routing stubs.
"""
import json
from pathlib import Path
import sys

root, out = map(Path, sys.argv[1:3])
fixtures = Path(__file__).resolve().parent
host = (root / 'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift').read_text()
model_start = host.index('@MainActor\npublic final class CrepusStateStore')
model_end = host.index('    public func text(', model_start)
model = (host[model_start:host.index('    public func bool(', model_end)]
         + host[host.index('    public func number(', model_end):host.index('    public func items(', model_end)]
         + host[host.index('    private func scopeJson(', model_end):host.index('\nprivate func currentArchitecture()', model_end)])
bridge_start = host.index('    static func encodeQueuedChangeValue')
bridge_end = host.index('    public static func lastError', bridge_start)
bridge = host[bridge_start:bridge_end]
oversize_start = host.index('    private nonisolated static func decodeOwnedResult')
oversize_end = host.index('    private static func dispatchHostAction', oversize_start)
bridge += host[oversize_start:oversize_end]
source = (root / 'crates/crepuscularity-native/src/codegen/swiftui.rs').read_text()
start = source.index('"import Foundation')
generated = json.JSONDecoder().raw_decode(source[start:])[0].replace('{{', '{').replace('}}', '}')
generated = generated[generated.index('@MainActor\npublic enum'):generated.index('    public static func dismissKeyboard')] + '}\n'
snapshot = (root / 'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift').read_text()
snapshot = snapshot[snapshot.index('@MainActor\npublic enum'):snapshot.index('public struct')]
for name, actions in [('generated', generated), ('snapshot', snapshot)]:
    (out / f'{name}-async.swift').write_text(
        'import Foundation\nimport SwiftUI\nimport Darwin\n'
        + (fixtures / 'SwiftAsyncStubs.swift').read_text()
        + '\n@MainActor enum CrepusRustActions {\n' + bridge + '}\n'
        + model + actions + (fixtures / 'SwiftAsyncHarness.swift').read_text()
    )
