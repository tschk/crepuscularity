// UIKit collaborators are stubs; delegate, retention, JSON and copies are production code.
@MainActor private protocol UIDocumentPickerDelegate {}
private final class UIDocumentPickerViewController {}
@MainActor private enum CrepusActions {
    static var resultSink: (String) -> Void = { _ in fatalError("unexpected result") }
}

private enum Probe {
    static var gate: DispatchSemaphore?
    static var onRead: (() -> Void)?
    static var copies: [String] = []
}

private func pickedFileJson(url: URL) -> [String: Any]? {
    precondition(!Thread.isMainThread, "provider read/copy ran on main")
    Probe.onRead?()
    Probe.gate?.wait()
    let file = actualPickedFileJson(url: url)
    if let path = file?["filePath"] as? String { Probe.copies.append(path) }
    return file
}

@main private struct Main {
    static func main() {
        DispatchQueue.global().asyncAfter(deadline: .now() + 10) {
            print("FAIL picker timeout")
            exit(1)
        }
        DispatchQueue.main.async { start() }
        RunLoop.main.run()
    }

    @MainActor static func start() {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try! FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        let source = directory.appendingPathComponent("first.bin")
        try! Data(repeating: 42, count: 65536).write(to: source)
        let delegate = FilePickerDelegate(action: "cancel-import")
        CrepusHostPicker.shared.retain(delegate: delegate)
        let controller = UIDocumentPickerViewController()
        let gate = DispatchSemaphore(value: 0)
        Probe.gate = gate
        var results = 0
        CrepusActions.resultSink = { raw in
            precondition(Thread.isMainThread)
            results += 1
            let json = try! JSONSerialization.jsonObject(with: Data(raw.utf8)) as! [String: Any]
            precondition(json["action"] as? String == "cancel-import")
            precondition(!CrepusHostPicker.shared.contains(delegate: delegate))
            precondition(CrepusHostPicker.shared.isActive, "canceled copies still occupy the single worker")
        }
        Probe.onRead = {
            DispatchQueue.main.async {
                // This heartbeat runs while the worker is blocked in provider preparation.
                precondition(results == 0)
                delegate.documentPickerWasCancelled(controller)
                delegate.documentPickerWasCancelled(controller)
                precondition(results == 1)
                gate.signal()
                pickerImportQueue.async {
                    DispatchQueue.main.async {
                        // Stale completion has enqueued cleanup before this check.
                        pickerImportQueue.async {
                            precondition(Probe.copies.count == 1)
                            precondition(!FileManager.default.fileExists(atPath: Probe.copies[0]))
                            Probe.gate = nil
                            Probe.onRead = nil
                            DispatchQueue.main.async {
                                precondition(results == 1)
                                success(directory: directory, source: source)
                            }
                        }
                    }
                }
            }
        }
        delegate.documentPicker(controller, didPickDocumentsAt: [source])
        delegate.documentPicker(controller, didPickDocumentsAt: [source])
    }

    @MainActor static func success(directory: URL, source: URL) {
        let second = directory.appendingPathComponent("second.bin")
        try! Data([1, 2, 3]).write(to: second)
        let delegate = FilePickerDelegate(action: "import")
        CrepusHostPicker.shared.retain(delegate: delegate)
        CrepusActions.resultSink = { raw in
            precondition(Thread.isMainThread)
            precondition(!CrepusHostPicker.shared.isActive)
            let json = try! JSONSerialization.jsonObject(with: Data(raw.utf8)) as! [String: Any]
            precondition(json["action"] as? String == "import")
            precondition(json["ok"] as? Bool == false && json["failedCount"] as? Int == 1)
            let files = (json["value"] as! [String: Any])["files"] as! [[String: Any]]
            precondition(files.map { $0["name"] as! String } == ["first.bin", "second.bin"])
            for (file, original) in zip(files, [source, second]) {
                let path = file["filePath"] as! String
                precondition(try! Data(contentsOf: URL(fileURLWithPath: path)) == Data(contentsOf: original))
                try! FileManager.default.removeItem(atPath: path)
            }
            try! FileManager.default.removeItem(at: directory)
            print("PASS Swift picker: blocked worker/main heartbeat, duplicate/cancel, stale cleanup, real copies, ordered partial failure")
            exit(0)
        }
        delegate.documentPicker(UIDocumentPickerViewController(), didPickDocumentsAt: [source, directory.appendingPathComponent("missing.bin"), second])
    }
}
