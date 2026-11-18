import Foundation
import XCTest
import CrepuscularityPlugin

final class CrepuscularityPluginTests: XCTestCase {
    func testLargeOutputAndDiagnosticsDoNotFillPipes() throws {
        try withTransport(fails: false) { path, _ in
            let ir = try renderIr(path: path)
            XCTAssertEqual(ir.version, 7)
            guard case let .string(content)? = ir.root.first?["content"] else {
                return XCTFail("missing text node")
            }
            XCTAssertEqual(content, String(repeating: "雪🦋", count: 32768))
        }
    }

    func testFailedTransportPreservesStatusAndUnicodeDiagnostic() throws {
        try withTransport(fails: true) { path, diagnostic in
            do {
                _ = try renderIr(path: path)
                XCTFail("CLI failure accepted")
            } catch {
                let error = error as NSError
                XCTAssertEqual(error.domain, "Crepuscularity")
                XCTAssertEqual(error.code, 9)
                XCTAssertEqual(error.localizedDescription, diagnostic)
            }
        }
    }

    func testHTMLAttributesPreserveCarriageReturns() throws {
        try withTransport(fails: false) { path, _ in
            let html = try renderHtml(path: path)
            XCTAssertTrue(html.contains("<button data-onclick=\"save&#13;\n雪\">Label &amp;雪</button>"))
        }
    }

    private func withTransport(fails: Bool, run: (String, String) throws -> Void) throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let executable = directory.appendingPathComponent("crepus")
        let diagnostic = String(repeating: "雪 diagnostic\n", count: 16384)
        let script = """
        #!/usr/bin/env python3
        import json,sys
        sys.stderr.buffer.write(('\\u96ea diagnostic\\n' * 16384).encode('utf-8'))
        sys.stderr.buffer.flush()
        if \(fails ? "True" : "False"):
            sys.stdout.buffer.write(bytes([255]))
            sys.exit(9)
        sys.stdout.buffer.write(json.dumps({'version':7,'root':[
            {'kind':'text','content':'\\u96ea\\U0001f98b' * 32768},
            {'kind':'button','label':'Label &\\u96ea','onClick':'save\\r\\n\\u96ea'}
        ]},ensure_ascii=False).encode('utf-8'))
        """
        try script.write(to: executable, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: executable.path)
        let previous = ProcessInfo.processInfo.environment["CREPUS_BIN"]
        setenv("CREPUS_BIN", executable.path, 1)
        defer {
            if let previous { setenv("CREPUS_BIN", previous, 1) }
            else { unsetenv("CREPUS_BIN") }
        }
        try run("fixture.crepus", diagnostic)
    }
}
