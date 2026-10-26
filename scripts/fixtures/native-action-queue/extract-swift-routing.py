from pathlib import Path
import sys
p=(Path(sys.argv[1])/'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift').read_text()
install=p[p.index('    public static func install()'):p.index('    private nonisolated static func defaultDispatch')]
default=p[p.index('    private nonisolated static func defaultDispatch'):p.index('    public static func dispatchStored')]
oversize=p[p.index('    private nonisolated static func decodeOwnedResult'):p.index('    private static func dispatchHostAction')]
s='''import Foundation
import Darwin
func crepusMobileDispatchJsonOwned(_ p:UnsafePointer<CChar>,_ n:UInt)->UnsafeMutablePointer<CChar>? {
 precondition(!Thread.isMainThread)
 let value="rust:"+String(cString:p)
 return strdup(value)
}
func crepusMobileFreeString(_ p:UnsafeMutablePointer<CChar>?) { free(p) }
@MainActor enum CrepusActions {
 static var dispatch:(String)->String={_ in ""}
 static var resultSink:(String)->Void={_ in}
}
@MainActor final class CrepusStateStore {
 static let shared=CrepusStateStore()
 func applyResult(_ value:String) {}
}
@MainActor enum CrepusRustActions {
'''+install+default+oversize+'''
 private static func dispatchHostAction(_ action:String)->String? {
  precondition(Thread.isMainThread)
  return action=="host" ? "ui:host" : nil
 }
}
@MainActor final class ResultBox {var values=[String]();var done=false}
@main struct Main {
 @MainActor static func main() {
  CrepusRustActions.install()
  let dispatch=CrepusActions.dispatch
  precondition(dispatch("host")=="ui:host")
  let box=ResultBox()
  DispatchQueue(label:"proof.worker").async {
   let values=[dispatch("host"),dispatch("plain")]
   DispatchQueue.main.async {box.values=values;box.done=true}
  }
  let deadline=Date().addingTimeInterval(3)
  while !box.done && Date()<deadline {RunLoop.main.run(until:Date().addingTimeInterval(0.005))}
  precondition(box.done && box.values == ["ui:host","rust:plain"])
  print("PASS main fast-path + worker host routing main + FFI worker")
 }
}
'''
# Harness result state is confined to main after handoff; no production Sendable changes.
(Path(sys.argv[2])/'route.swift').write_text(s)
