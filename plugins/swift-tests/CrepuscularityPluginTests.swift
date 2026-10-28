import Foundation
import XCTest
import CrepuscularityPlugin

final class CrepuscularityPluginTests: XCTestCase {
    func testFailedTransportPreservesLargeUnicodeDiagnostic() throws {
        try withTransportFixture(fails: true) { source, diagnostic in
            for html in [false, true] {
                do {
                    if html { _ = try renderHtml(path: source.path) }
                    else { _ = try renderIr(path: source.path) }
                    XCTFail("CLI failure accepted")
                } catch {
                    let error = error as NSError
                    XCTAssertEqual(error.domain, "Crepuscularity")
                    XCTAssertEqual(error.code, 9)
                    XCTAssertEqual(error.localizedDescription, diagnostic)
                }
            }
        }
    }

    func testSuccessfulTransportDrainsLargeStderr() throws {
        try withTransportFixture(fails: false) { source, _ in
            let ir = try renderIr(path: source.path)
            XCTAssertEqual(ir.version, 7)
            guard case let .string(content)? = ir.root.first?["content"] else {
                return XCTFail("missing text node")
            }
            XCTAssertEqual(content, "雪 🦋")
            XCTAssertEqual(try renderHtml(path: source.path), "雪 🦋")
        }
    }

    // Labelled subprocess fixture exercises transport, not Rust parsing.
    private func withTransportFixture(fails: Bool, run: (URL, String) throws -> Void) throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("source.crepus")
        try "span \"Source\"".write(to: source, atomically: true, encoding: .utf8)
        let executable = directory.appendingPathComponent("crepus")
        let diagnostic = String(repeating: "雪 diagnostic\n", count: 16_384)
        let script = """
        #!/usr/bin/env python3
        import json,sys
        sys.stderr.buffer.write(('\\u96ea diagnostic\\n' * 16384).encode('utf-8'))
        sys.stderr.buffer.flush()
        if \(fails ? "True" : "False"):
            sys.stdout.buffer.write(bytes([255]))
            sys.exit(9)
        sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'\\u96ea \\U0001f98b'}]},ensure_ascii=False).encode('utf-8'))
        """
        try script.write(to: executable, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: executable.path)
        let previous = ProcessInfo.processInfo.environment["CREPUS_BIN"]
        setenv("CREPUS_BIN", executable.path, 1)
        defer {
            if let previous { setenv("CREPUS_BIN", previous, 1) }
            else { unsetenv("CREPUS_BIN") }
        }
        try run(source, diagnostic)
    }

    func testHTMLActionAttributesPreserveCarriageReturns() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("action.jsx")
        try "<button onClick=\"save\r\nnext\rend &quot;雪\">Label &雪</button>"
            .write(to: source, atomically: true, encoding: .utf8)
        let html = try renderHtml(path: source.path)
        XCTAssertEqual(html, "<button data-onclick=\"save&#13;\nnext&#13;end &amp;quot;雪\">Label &amp;雪</button>")
    }

    func testLargeIRDoesNotFillTheProcessPipe() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("large.crepus")
        let text = String(repeating: "x", count: 131_072)
        try "span\n  \"\(text)\"".write(to: source, atomically: true, encoding: .utf8)
        let ir = try renderIr(path: source.path)
        XCTAssertEqual(ir.version, 7)
        guard case let .string(content)? = ir.root.first?["content"] else {
            return XCTFail("missing text node")
        }
        XCTAssertEqual(content, text)
    }
}
