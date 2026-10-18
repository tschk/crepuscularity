import Foundation

public struct ViewIr: Decodable {
    public let version: Int
    public let root: [Node]

    public typealias Node = [String: JSONValue]
}

public enum JSONValue: Decodable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case object([String: JSONValue])
    case array([JSONValue])
    case null

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let v = try? c.decode(Bool.self) {
            self = .bool(v)
        } else if let v = try? c.decode(Double.self) {
            self = .number(v)
        } else if let v = try? c.decode(String.self) {
            self = .string(v)
        } else if let v = try? c.decode([JSONValue].self) {
            self = .array(v)
        } else {
            self = .object(try c.decode([String: JSONValue].self))
        }
    }
}

public func renderIr(path: String) throws -> ViewIr {
    let bin = ProcessInfo.processInfo.environment["CREPUS_BIN"] ?? "crepus"
    let process = Process()
    let output = Pipe()
    let diagnosticDirectory = FileManager.default.temporaryDirectory
        .appendingPathComponent("crepus-stderr-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: diagnosticDirectory, withIntermediateDirectories: false,
                                           attributes: [.posixPermissions: 0o700])
    defer { try? FileManager.default.removeItem(at: diagnosticDirectory) }
    let diagnosticURL = diagnosticDirectory.appendingPathComponent("stderr")
    try Data().write(to: diagnosticURL, options: .withoutOverwriting)
    let diagnosticHandle = try FileHandle(forUpdating: diagnosticURL)
    defer { try? diagnosticHandle.close() }
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = [bin, "native", "ir", path]
    process.standardOutput = output
    // A regular stderr file cannot fill a pipe while stdout is drained.
    process.standardError = diagnosticHandle
    try process.run()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    try? diagnosticHandle.seek(toOffset: 0)
    let diagnosticData = (try? diagnosticHandle.readToEnd()) ?? Data()
    try? FileHandle.standardError.write(contentsOf: diagnosticData)
    if process.terminationStatus != 0 {
        let diagnostic = String(decoding: diagnosticData, as: UTF8.self)
        throw NSError(domain: "Crepuscularity", code: Int(process.terminationStatus),
                      userInfo: diagnostic.isEmpty ? nil : [NSLocalizedDescriptionKey: diagnostic])
    }
    return try JSONDecoder().decode(ViewIr.self, from: data)
}

public func renderHtml(path: String) throws -> String {
    try renderIr(path: path).root.map(renderNode).joined()
}

private func renderNode(_ node: ViewIr.Node) -> String {
    guard case let .string(kind)? = node["kind"] else {
        return ""
    }
    switch kind {
    case "text":
        if case let .string(content)? = node["content"] {
            return escapeHtml(content)
        }
        return ""
    case "stack", "scroll":
        let axis: String
        if case let .string(value)? = node["axis"] {
            axis = value
        } else {
            axis = "column"
        }
        let children: String
        if case let .array(values)? = node["children"] {
            children = values.compactMap { value in
                if case let .object(child) = value {
                    return renderNode(child)
                }
                return nil
            }.joined()
        } else {
            children = ""
        }
        return "<div data-crepus-kind=\"\(escapeAttribute(kind))\" data-axis=\"\(escapeAttribute(axis))\">\(children)</div>"
    case "button":
        if case let .string(label)? = node["label"] {
            if case let .string(onClick)? = node["onClick"] {
                return "<button data-onclick=\"\(escapeAttribute(onClick))\">\(escapeHtml(label))</button>"
            }
            return "<button>\(escapeHtml(label))</button>"
        }
        return "<button></button>"
    default:
        return ""
    }
}

private func escapeAttribute(_ value: String) -> String {
    escapeHtml(value).replacingOccurrences(of: "\r", with: "&#13;")
}

private func escapeHtml(_ value: String) -> String {
    value.replacingOccurrences(of: "&", with: "&amp;")
        .replacingOccurrences(of: "<", with: "&lt;")
        .replacingOccurrences(of: ">", with: "&gt;")
        .replacingOccurrences(of: "\"", with: "&quot;")
}
