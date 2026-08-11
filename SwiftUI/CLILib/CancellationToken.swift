// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 実行中の比較タスクをキャンセルするための共有トークン。
// SIGINT/SIGTERM(シグナルハンドラ)とCLIRunner(比較タスク)を安全に橋渡しする。

import Foundation

/// スレッドセーフなキャンセルトークン。シグナルハンドラからcancel()を呼ぶと、
/// 実行中の比較Taskへキャンセルを伝播する(Swift ConcurrencyのTaskキャンセル)。
public final class CancellationToken: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelAction: (() -> Void)?

    public init() {}

    public func set(_ action: @escaping @Sendable () -> Void) {
        lock.lock()
        defer { lock.unlock() }
        cancelAction = action
    }

    public func clear() {
        lock.lock()
        defer { lock.unlock() }
        cancelAction = nil
    }

    public func cancel() {
        lock.lock()
        defer { lock.unlock() }
        cancelAction?()
    }
}
