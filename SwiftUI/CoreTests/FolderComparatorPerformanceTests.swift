// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 1万ファイル規模の FolderComparator.compare の所要時間を測る性能計測。
// 環境変数 DIFFLOUPE_STRESS_DIR(Scripts/make_stress_fixtures.sh の出力先)が無ければスキップする。

import XCTest
@testable import DiffLoupeCore

final class FolderComparatorPerformanceTests: XCTestCase {
    func testStressCompareTiming() async throws {
        guard let base = ProcessInfo.processInfo.environment["DIFFLOUPE_STRESS_DIR"] else {
            throw XCTSkip("DIFFLOUPE_STRESS_DIR 未設定")
        }
        let left = URL(fileURLWithPath: base).appendingPathComponent("left")
        let right = URL(fileURLWithPath: base).appendingPathComponent("right")
        var times: [Double] = []
        var count = 0
        for _ in 0..<7 {
            let start = DispatchTime.now()
            let result = try await FolderComparator().compare(
                left: left, right: right, options: .init(recursive: true), progress: { _ in })
            let t = Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1e9
            times.append(t)
            count = result.entries.count
        }
        times.sort()
        print(String(format: "PERF entries=%d median=%.3f min=%.3f max=%.3f",
                     count, times[3], times[0], times[6]))
    }

#if canImport(diffloupeFFI)
    /// Rust 側の比較本体・FFI 受け渡し・Swift 型への変換を分けて測る(Rust コア版のみ)
    func testStressCompareBreakdown() throws {
        guard let base = ProcessInfo.processInfo.environment["DIFFLOUPE_STRESS_DIR"] else {
            throw XCTSkip("DIFFLOUPE_STRESS_DIR 未設定")
        }
        let left = URL(fileURLWithPath: base).appendingPathComponent("left")
        let right = URL(fileURLWithPath: base).appendingPathComponent("right")
        var ffiTimes: [Double] = [], convTimes: [Double] = []
        for _ in 0..<7 {
            let t0 = DispatchTime.now()
            let r = try diffloupeCompareFolders(
                leftPath: left.path, rightPath: right.path,
                options: FolderComparator.Options(recursive: true).ffi, progress: nil, cancel: nil)
            let t1 = DispatchTime.now()
            _ = ComparisonResult(r, leftRoot: left, rightRoot: right)
            let t2 = DispatchTime.now()
            ffiTimes.append(Double(t1.uptimeNanoseconds - t0.uptimeNanoseconds) / 1e9)
            convTimes.append(Double(t2.uptimeNanoseconds - t1.uptimeNanoseconds) / 1e9)
        }
        ffiTimes.sort(); convTimes.sort()
        print(String(format: "PERF-BREAKDOWN ffi_call(rust+lift) median=%.3f  swift_convert median=%.3f",
                     ffiTimes[3], convTimes[3]))
    }
#endif
}
