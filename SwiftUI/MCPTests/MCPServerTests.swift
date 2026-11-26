import XCTest
@testable import DiffLoupeMCP

final class MCPServerTests: XCTestCase {
    private var root: URL!
    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    }
    override func tearDownWithError() throws { try FileManager.default.removeItem(at: root) }
    private func request(_ server: MCPServer, _ method: String, _ params: [String: Any] = [:]) async -> [String: Any] {
        let data = try! JSONSerialization.data(withJSONObject: ["jsonrpc": "2.0", "id": 1, "method": method, "params": params])
        return await server.handle(data)!
    }
    private func server() async throws -> MCPServer {
        let server = try MCPServer(roots: [root.path])
        _ = await request(server, "initialize", ["protocolVersion": "2025-11-25", "capabilities": [:], "clientInfo": ["name": "test", "version": "1"]])
        let reply = await server.handle(Data(#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.utf8))
        XCTAssertNil(reply)
        return server
    }
    private func tool(_ server: MCPServer, _ name: String, _ arguments: [String: Any]) async -> [String: Any] {
        let reply = await request(server, "tools/call", ["name": name, "arguments": arguments])
        return reply["result"] as! [String: Any]
    }
    private func output(_ result: [String: Any]) throws -> [String: Any] {
        XCTAssertEqual(result["isError"] as? Bool, false)
        let content = result["content"] as! [[String: Any]]
        return try JSONSerialization.jsonObject(with: Data((content[0]["text"] as! String).utf8)) as! [String: Any]
    }
    private func file(_ name: String, _ text: String) throws -> String {
        let url = root.appendingPathComponent(name)
        try Data(text.utf8).write(to: url)
        return url.path
    }

    func testLifecycleAndProtocolErrors() async throws {
        let s = try MCPServer(roots: [root.path])
        let early = await request(s, "tools/list")
        XCTAssertNotNil(early["error"])
        let malformed = await s.handle(Data("{".utf8))
        XCTAssertEqual((malformed?["error"] as? [String: Any])?["code"] as? Int, -32700)
        let ready = try await server()
        let tools = await request(ready, "tools/list")
        XCTAssertEqual(((tools["result"] as? [String: Any])?["tools"] as? [Any])?.count, 2)
        let unknown = await request(ready, "missing")
        XCTAssertEqual((unknown["error"] as? [String: Any])?["code"] as? Int, -32601)
    }

    func testChangedRowsPaginationAndClipping() async throws {
        let s = try await server()
        let left = try file("a", "same\nold\nkeep\n" + String(repeating: "a", count: 200))
        let right = try file("b", "same\nnew\nkeep\n" + String(repeating: "b", count: 200))
        let first = try output(await tool(s, "diff_files", ["left": left, "right": right, "context": 0, "limit": 1]))
        XCTAssertEqual(first["totalRows"] as? Int, 2)
        XCTAssertEqual(first["nextOffset"] as? Int, 1)
        let rows = first["rows"] as! [[String: Any]]
        XCTAssertEqual(rows[0]["leftLine"] as? Int, 2)
        XCTAssertEqual(rows[0]["left"] as? String, "old")
        let second = try output(await tool(s, "diff_files", ["left": left, "right": right, "context": 0, "offset": 1, "maxTextCharacters": 40]))
        let row = (second["rows"] as! [[String: Any]])[0]
        XCTAssertEqual((row["left"] as? String)?.count, 40)
        XCTAssertEqual(row["leftTruncated"] as? Bool, true)
        XCTAssertTrue(second["nextOffset"] is NSNull)
    }

    func testIdenticalAndLineEndingSemantics() async throws {
        let s = try await server()
        let a = try file("a", "hello\r\n"), b = try file("b", "hello\n")
        let result = try output(await tool(s, "diff_files", ["left": a, "right": b]))
        XCTAssertEqual(result["changedRows"] as? Int, 0)
        XCTAssertEqual(result["decodedTextEqual"] as? Bool, false)
        let identical = try output(await tool(s, "diff_files", ["left": a, "right": a]))
        XCTAssertEqual(identical["decodedTextEqual"] as? Bool, true)
    }

    func testPathAndArgumentValidation() async throws {
        let s = try await server()
        let a = try file("a", "hello")
        for extra: [String: Any] in [["limit": true], ["offset": -1], ["limit": 1.5], ["limit": 101], ["unknown": 1]] {
            let args = ["left": a, "right": a].merging(extra) { _, new in new }
            let result = await tool(s, "diff_files", args)
            XCTAssertEqual(result["isError"] as? Bool, true)
        }
        for outside in [root.path + "/../outside", root.path + "-sibling/a", "relative"] {
            let result = await tool(s, "diff_files", ["left": a, "right": outside])
            XCTAssertEqual(result["isError"] as? Bool, true)
        }
        let link = root.appendingPathComponent("escape")
        try FileManager.default.createSymbolicLink(atPath: link.path, withDestinationPath: "/etc/hosts")
        let result = await tool(s, "diff_files", ["left": a, "right": link.path])
        XCTAssertEqual(result["isError"] as? Bool, true)
        XCTAssertThrowsError(try MCPServer(roots: []))
    }

    func testBinaryAndResourceLimits() async throws {
        let s = try await server()
        let binary = try file("binary", "\0binary")
        let large = try file("large", String(repeating: "x", count: 1048577))
        let lines = try file("lines", String(repeating: "x\n", count: 20001))
        for path in [binary, large, lines] {
            let result = await tool(s, "diff_files", ["left": path, "right": path])
            XCTAssertEqual(result["isError"] as? Bool, true)
        }
    }

    func testFolderSummarySkipsContentsAndSymlinks() async throws {
        let s = try await server()
        let left = root.appendingPathComponent("left"), right = root.appendingPathComponent("right")
        for dir in [left, right] { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true) }
        for dir in [left, right] { try Data("same".utf8).write(to: dir.appendingPathComponent("same")) }
        try Data("secret body".utf8).write(to: left.appendingPathComponent("only"))
        try FileManager.default.createSymbolicLink(atPath: left.appendingPathComponent("escape").path, withDestinationPath: "/etc/hosts")
        let result = try output(await tool(s, "compare_folders", ["left": left.path, "right": right.path]))
        let entries = result["entries"] as! [[String: Any]]
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0]["path"] as? String, "only")
        let summary = result["summary"] as! [String: Any]
        XCTAssertEqual(summary["identical"] as? Int, 1)
        XCTAssertEqual(summary["leftOnly"] as? Int, 1)
        XCTAssertFalse(String(describing: result).contains("secret body"))
    }
    func testFolderLimitIsAnExplicitError() async throws {
        let s = try await server()
        let left = root.appendingPathComponent("left"), right = root.appendingPathComponent("right")
        for dir in [left, right] { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true) }
        for i in 0...2000 { try Data().write(to: left.appendingPathComponent("file-\(i)")) }
        let result = await tool(s, "compare_folders", ["left": left.path, "right": right.path])
        XCTAssertEqual(result["isError"] as? Bool, true)
    }

    func testFolderTypeConflictIsNotOmitted() async throws {
        let s = try await server()
        let left = root.appendingPathComponent("left"), right = root.appendingPathComponent("right")
        for dir in [left, right] { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true) }
        try FileManager.default.createDirectory(at: left.appendingPathComponent("conflict"), withIntermediateDirectories: true)
        try Data("file".utf8).write(to: right.appendingPathComponent("conflict"))
        let result = try output(await tool(s, "compare_folders", ["left": left.path, "right": right.path]))
        let entries = result["entries"] as! [[String: Any]]
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0]["status"] as? String, "different")
    }

}
