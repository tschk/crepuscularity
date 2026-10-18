from pathlib import Path
import json
import sys
base=Path(sys.argv[2])
repo=Path(sys.argv[1])
s=(repo/'crates/crepuscularity-native/src/codegen/swiftui.rs').read_text()
a=s.index('"import Foundation');generated=json.JSONDecoder().raw_decode(s[a:])[0].replace('{{','{').replace('}}','}').replace('{webview}','')
a=generated.index('@MainActor\npublic enum');b=generated.index('    public static func performChange',a)
generated=generated[:b]+'}\n';b=len(generated)
generated=generated.replace('    public static let model = CrepusStateStore.shared\n','').replace('    public static func applyResult(_ json: String) {\n        model.applyResult(json)\n    }\n','')
snapshot=(repo/'crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift').read_text().split('public struct')[0].replace('import SwiftUI','')
snapshot=snapshot[:snapshot.index('    public static func performChange')]+'}\n'
snapshot=snapshot.replace('    public static let model = CrepusStateStore.shared\n','')
variants={'generated':'import Foundation\n'+generated[a:b], 'snapshot':snapshot}
harness='''
final class Probe {
 let lock=NSLock()
 var starts=[String](); var results=[String](); var active=0; var maximum=0
 let entered=DispatchSemaphore(value:0); let release=DispatchSemaphore(value:0)
 let secondEntered=DispatchSemaphore(value:0)
 func dispatch(_ value:String)->String {
  if value=="1" {secondEntered.signal()}
  lock.lock()
  precondition(results == (0..<Int(value)!).map(String.init), "previous main sink not delivered")
  active += 1; maximum=max(maximum,active); starts.append(value); lock.unlock()
  precondition(!Thread.isMainThread)
  if value=="0" {entered.signal(); precondition(release.wait(timeout:.now()+3) == .success)}
  lock.lock(); active -= 1; lock.unlock()
  return value
 }
}
@main struct Main {
 @MainActor static func main() {
  let p=Probe();let count=40
  CrepusActions.dispatch={p.dispatch($0)}
  CrepusActions.resultSink={value in precondition(Thread.isMainThread);p.lock.lock();p.results.append(value);p.lock.unlock()}
  for i in 0..<count {CrepusActions.perform(String(i))}
  precondition(p.entered.wait(timeout:.now()+3) == .success)
  p.lock.lock();precondition(p.starts == ["0"]);p.lock.unlock()
  // Hook replacement must not alter any already submitted request.
  CrepusActions.dispatch={_ in fatalError("uncaptured dispatch")}
  CrepusActions.resultSink={_ in fatalError("uncaptured sink")}
  var heartbeat=false
  DispatchQueue.main.async {
   heartbeat=true;p.release.signal()
   // Hold main delivery behind a bounded semaphore probe, not a sleep.
   // Old async delivery starts action 1 here; sync delivery cannot do so.
   precondition(p.secondEntered.wait(timeout:.now()+0.1) == .timedOut)
  }
  let deadline=Date().addingTimeInterval(5)
  while p.results.count<count && Date()<deadline {RunLoop.main.run(until:Date().addingTimeInterval(0.005))}
  precondition(heartbeat)
  precondition(p.results == (0..<count).map(String.init))
  precondition(p.starts == p.results)
  precondition(p.maximum == 1)
  print("PASS FIFO40 prior-sink-before-next-dispatch/exactly-once/max-active1/main-sink/captured-hooks/blocked-first-main-heartbeat")
 }
}
'''
for name,prefix in variants.items():(base/(name+'.swift')).write_text(prefix+harness)
