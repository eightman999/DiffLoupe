import Foundation
import DiffLoupeMCP

// stdoutはJSON-RPC専用。起動設定とエラーはstderrへ。
let args = Array(CommandLine.arguments.dropFirst())
if args == ["--help"] {
    FileHandle.standardError.write(Data("Usage: diffloupe-mcp --root /absolute/folder [--root /another/folder]\nRead-only MCP over stdio. No network listener.\n".utf8))
    exit(0)
}
do {
    var roots: [String] = []
    var i = 0
    while i < args.count {
        guard args[i] == "--root", i + 1 < args.count else {
            throw NSError(domain: "DiffLoupeMCP", code: 2, userInfo: [NSLocalizedDescriptionKey: "Expected --root /absolute/folder"])
        }
        roots.append(args[i + 1]); i += 2
    }
    let server = try MCPServer(roots: roots)
    var buffer = Data()
    // read(upToCount:)はパイプで指定バイト数まで待つため、到着済みデータを読む。
    while case let chunk = FileHandle.standardInput.availableData, !chunk.isEmpty {
        buffer.append(chunk)
        while let newline = buffer.firstIndex(of: 10) {
            let line = Data(buffer[..<newline])
            buffer.removeSubrange(...newline)
            guard line.count <= 1048576 else { throw NSError(domain: "DiffLoupeMCP", code: 2) }
            if let reply = await server.handle(line) {
                var data = try JSONSerialization.data(withJSONObject: reply, options: [.sortedKeys, .withoutEscapingSlashes])
                data.append(10)
                try FileHandle.standardOutput.write(contentsOf: data)
            }
        }
        guard buffer.count <= 1048576 else { throw NSError(domain: "DiffLoupeMCP", code: 2) }
    }
} catch {
    FileHandle.standardError.write(Data("diffloupe-mcp: \(error.localizedDescription)\n".utf8))
    exit(2)
}
