@main struct AsyncChangeMain {
    @MainActor static func pump(until done: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        while !done() && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.002))
        }
        precondition(done(), "missing main delivery")
    }

    @MainActor static func main() {
        let model = CrepusActions.model
        var completed = 0
        func text(_ path: String) -> String { nativeProbe.values[path] as? String ?? "original" }
        CrepusActions.resultSink = { raw in
            // Even old acknowledgements must retain the most recent user draft.
            precondition(model.draftText(path: "items.0.name", fallback: "wrong") == "abc")
            model.applyResult(raw)
            completed += 1
        }
        CrepusActions.performChange("blocked", bind: "items.0.name", value: "a")
        CrepusActions.performChange("echo", bind: "items.0.name", value: "ab")
        CrepusActions.performChange("upper", bind: "items.0.name", value: "abc")
        precondition(nativeProbe.entered.wait(timeout: .now() + 3) == .success)
        var fallbackCalled = false
        func unexpectedFallback() -> String { fallbackCalled = true; return "wrong" }
        precondition(model.draftText(path: "items.0.name", fallback: unexpectedFallback()) == "abc")
        precondition(!fallbackCalled && nativeProbe.values.isEmpty)
        CrepusActions.resultSink = { _ in preconditionFailure("queued sink was not captured") }
        DispatchQueue.main.async { nativeProbe.release.signal() }
        pump { completed == 3 }
        precondition(model.draftText(path: "items.0.name", fallback: text("items.0.name")) == "ABC")

        CrepusActions.resultSink = { raw in model.applyResult(raw); completed += 1 }
        CrepusActions.performChange("fail", bind: "items.0.name", value: "rejected")
        precondition(model.draftText(path: "items.0.name", fallback: "wrong") == "rejected")
        pump { completed == 4 }
        precondition(model.draftText(path: "items.0.name", fallback: text("items.0.name")) == "ABC")

        CrepusActions.performChange("echo", bind: "items.1.enabled", value: true)
        CrepusActions.performChange("echo", bind: "items.2.amount", value: 2.5)
        precondition(model.draftBool(path: "items.1.enabled", fallback: false))
        precondition(model.draftNumber(path: "items.2.amount", fallback: 0) == 2.5)
        for (raw, expected) in [("0.75", 0.75), ("-2.5", -2.5), ("1e2", 100.0)] {
            nativeProbe.numberScope = raw
            nativeProbe.numberResult = expected
            let sequence = model.beginDraft(path: "numeric-text", valueJson: "\"\(raw)\"")!
            precondition(model.draftNumber(path: "numeric-text", fallback: -999) == expected)
            model.finishDraft(path: "numeric-text", sequence: sequence)
        }
        nativeProbe.numberResult = 0
        for raw in ["invalid", "NaN", "Infinity", "1e9999"] {
            nativeProbe.numberScope = raw
            let sequence = model.beginDraft(path: "numeric-text", valueJson: "\"\(raw)\"")!
            precondition(model.draftNumber(path: "numeric-text", fallback: -999) == 0)
            model.finishDraft(path: "numeric-text", sequence: sequence)
        }
        for raw in ["null", "false", "true", "[]", "[0]", "{}"] {
            let sequence = model.beginDraft(path: "numeric-other", valueJson: raw)!
            precondition(model.draftNumber(path: "numeric-other", fallback: -999) == 0, raw)
            model.finishDraft(path: "numeric-other", sequence: sequence)
            precondition(model.draftNumber(path: "numeric-other", fallback: -999) == -999)
        }
        for (raw, expected) in [
            ("null", false),
            ("false", false),
            ("true", true),
            ("0", false),
            ("-2", true),
            ("0.5", true),
            ("\"\"", false),
            ("\"false\"", true),
            ("\"0\"", true),
            ("[]", false),
            ("[0]", true),
            ("{}", false),
            ("{\"a\":0}", true)
        ] {
            let sequence = model.beginDraft(path: "truthy", valueJson: raw)!
            precondition(model.draftBool(path: "truthy", fallback: !expected) == expected, raw)
            model.finishDraft(path: "truthy", sequence: sequence)
            precondition(model.draftBool(path: "truthy", fallback: !expected) == !expected)
        }
        for raw in ["null", "false", "true", "0", "-0.0", "1e20", "[]", "[0]", "{}"] {
            nativeProbe.textScope = try! JSONSerialization.jsonObject(with: Data(raw.utf8), options: [.fragmentsAllowed])
            let sequence = model.beginDraft(path: "display", valueJson: raw)!
            precondition(model.draftText(path: "display", fallback: "STALE") == "formatted by Rust", raw)
            model.finishDraft(path: "display", sequence: sequence)
            precondition(model.draftText(path: "display", fallback: "absent") == "absent")
        }
        pump { completed == 6 }
        precondition(nativeProbe.values["items.1.enabled"] as? Bool == true)
        precondition(nativeProbe.values["items.2.amount"] as? Double == 2.5)

        // Empty action is a pure envelope until the captured sink commits it.
        let beforeCalls = nativeProbe.calls
        CrepusActions.resultSink = { raw in
            let envelope = try! JSONSerialization.jsonObject(with: Data(raw.utf8)) as! [String: Any]
            precondition(envelope["action"] as? String == "bind")
            completed += 1
        }
        CrepusActions.performChange(nil, bind: "custom", value: "draft")
        precondition(nativeProbe.values["custom"] == nil)
        pump { completed == 7 }
        precondition(nativeProbe.calls == beforeCalls && nativeProbe.values["custom"] == nil)
        precondition(model.draftText(path: "custom", fallback: "unchanged") == "unchanged")
        CrepusActions.resultSink = { raw in model.applyResult(raw); completed += 1 }
        CrepusActions.performChange(nil, bind: "empty", value: "committed")
        precondition(nativeProbe.values["empty"] == nil)
        pump { completed == 8 }
        precondition(nativeProbe.values["empty"] as? String == "committed")
        precondition(nativeProbe.calls == beforeCalls)

        // A sink may enqueue a newer draft before the older defer finishes.
        var reentered = false
        CrepusActions.resultSink = { raw in
            model.applyResult(raw); completed += 1
            if !reentered {
                reentered = true
                CrepusActions.performChange("echo", bind: "reentrant", value: "new")
            } else {
                precondition(model.draftText(path: "reentrant", fallback: "wrong") == "new")
            }
        }
        CrepusActions.performChange("echo", bind: "reentrant", value: "old")
        pump { completed == 10 }
        precondition(model.draftText(path: "reentrant", fallback: text("reentrant")) == "new")

        CrepusActions.resultSink = { raw in model.applyResult(raw); completed += 1 }
        let mutable = NSMutableString(string: "snapshot")
        CrepusActions.performChange("echo", bind: "mutable", value: mutable)
        mutable.append("-changed")
        precondition(model.draftText(path: "mutable", fallback: "wrong") == "snapshot")
        pump { completed == 11 }
        precondition(nativeProbe.values["mutable"] as? String == "snapshot")

        CrepusActions.resultSink = { raw in model.applyResult(raw); completed += 1 }
        CrepusActions.performChange(nil, bind: "array", value: [1, 2, 3])
        CrepusActions.performChange("echo", bind: "object", value: ["nested": [true, false]])
        let mutableObject = NSMutableDictionary(dictionary: ["name": "snapshot"])
        CrepusActions.performChange("echo", bind: "mutableObject", value: mutableObject)
        mutableObject["name"] = "changed"
        pump { completed == 14 }
        precondition(nativeProbe.values["array"] as? [Int] == [1, 2, 3])
        precondition((nativeProbe.values["object"] as? [String: [Bool]])?["nested"] == [true, false])
        precondition((nativeProbe.values["mutableObject"] as? [String: String])?["name"] == "snapshot")
        let tooDeep = Array(repeating: "a", count: 127).joined(separator: ".")
        CrepusActions.resultSink = { raw in
            let envelope = try! JSONSerialization.jsonObject(with: Data(raw.utf8)) as! [String: Any]
            precondition(envelope["ok"] as? Bool == false)
            precondition(envelope["error"] as? String == "binding path exceeds 126 segments")
            completed += 1
        }
        CrepusActions.performChange(nil, bind: tooDeep, value: "rejected")
        pump { completed == 15 }
        precondition(nativeProbe.values[tooDeep] == nil)
        precondition(model.draftText(path: tooDeep, fallback: "clean") == "clean")

        // Failed main-thread encoding submits no worker and leaves no draft.
        let calls = nativeProbe.calls
        var failures = 0
        CrepusActions.resultSink = { raw in
            let value = try! JSONSerialization.jsonObject(with: Data(raw.utf8)) as! [String: Any]
            precondition(value["ok"] as? Bool == false); failures += 1
        }
        CrepusActions.performChange("echo", bind: "bad", value: Double.nan)
        CrepusActions.performChange("echo", bind: "bad", value: [1: "non-string key"])
        CrepusActions.performChange("echo", bind: "bad", value: ["nested": [Double.infinity]])
        CrepusActions.performChange("echo", bind: "bad", value: Date())
        CrepusActions.performChange("echo", bind: "bad", value: Double.infinity)
        CrepusActions.performChange("echo", bind: "bad", value: ["nested": [Double.nan]])
        precondition(failures == 6 && nativeProbe.calls == calls)
        precondition(model.draftText(path: "bad", fallback: "clean") == "clean")
        let large = String(repeating: "雪", count: 6000)
        let beforeLarge = nativeProbe.calls
        CrepusActions.resultSink = { raw in model.applyResult(raw); completed += 1 }
        CrepusActions.performChange("echo", bind: "large", value: large)
        pump { completed == 16 }
        precondition(nativeProbe.calls == beforeLarge + 1, "large edit replayed")
        precondition(model.draftText(path: "large", fallback: text("large")) == large,
                     "large successful edit rolled back")
        precondition(nativeProbe.frees == nativeProbe.calls, "owned result leaked")
        print("PASS async edits: latest drafts/lazy fallbacks/FIFO sinkcommit/rollback/transforms/JSON values/canonical paths/custom sink/reentrant/empty deferred/encode failure")
    }
}
