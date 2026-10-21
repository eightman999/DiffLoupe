// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較履歴のストア(RecentComparisonStore)をUserDefaultsの分離スイートで検証する。
// 同一ペアの先頭移動・最大10件・壊れたデータの安全な無視・保存/復元の往復を確認する

import XCTest
@testable import DiffLoupe

final class RecentComparisonTests: XCTestCase {

    private func makeStore() -> (store: RecentComparisonStore, cleanup: () -> Void) {
        let name = "RecentComparisonTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        let store = RecentComparisonStore(defaults: defaults)
        return (store, { defaults.removePersistentDomain(forName: name) })
    }

    private func item(
        left: String,
        right: String,
        leftGitRef: String? = nil,
        rightGitRef: String? = nil,
        date: Date = Date(timeIntervalSince1970: 1_700_000_000)
    ) -> RecentComparison {
        RecentComparison(
            leftPath: left,
            rightPath: right,
            leftGitRef: leftGitRef,
            rightGitRef: rightGitRef,
            date: date
        )
    }

    func testLoadEmptyStoreReturnsEmpty() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        XCTAssertEqual(store.load(), [])
    }

    func testRecordAddsNewestFirst() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        let first = item(left: "/a", right: "/b")
        let second = item(left: "/c", right: "/d")

        _ = store.record(first)
        let list = store.record(second)

        XCTAssertEqual(list.map(\.leftPath), ["/c", "/a"])
        // UserDefaultsへも永続化されている
        XCTAssertEqual(store.load().map(\.leftPath), ["/c", "/a"])
    }

    func testSamePairMovesToFront() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        let pair = item(left: "/a", right: "/b")
        let other = item(left: "/x", right: "/y")

        _ = store.record(pair)
        _ = store.record(other)
        let list = store.record(pair)

        XCTAssertEqual(list.count, 2)
        XCTAssertEqual(list[0].leftPath, "/a")
        XCTAssertEqual(list[0].rightPath, "/b")
        XCTAssertEqual(list[1].leftPath, "/x")
    }

    func testSwappedOrderIsDifferentPair() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        let ab = item(left: "/a", right: "/b")
        let ba = item(left: "/b", right: "/a")

        _ = store.record(ab)
        let list = store.record(ba)

        XCTAssertEqual(list.count, 2)
    }

    func testMaxCountCapsAtTen() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        for index in 0..<12 {
            _ = store.record(item(left: "/dir\(index)", right: "/other"))
        }
        let list = store.load()
        XCTAssertEqual(list.count, 10)
        // 最新10件が残り、古い2件は捨てられる
        XCTAssertEqual(list.first?.leftPath, "/dir11")
        XCTAssertEqual(list.last?.leftPath, "/dir2")
        XCTAssertFalse(list.contains { $0.leftPath == "/dir0" })
        XCTAssertFalse(list.contains { $0.leftPath == "/dir1" })
    }

    func testRoundTripPreservesRefsAndDate() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let original = item(left: "/a", right: "/b", leftGitRef: "main", rightGitRef: "HEAD", date: date)

        _ = store.record(original)
        let loaded = store.load()

        XCTAssertEqual(loaded, [original])
    }

    func testCorruptedDataIsIgnoredSafely() {
        let name = "RecentComparisonTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set(Data("not valid json".utf8), forKey: RecentComparisonStore.storageKey)

        let store = RecentComparisonStore(defaults: defaults)
        XCTAssertEqual(store.load(), [])
    }

    func testNonDataValueIsIgnoredSafely() {
        let name = "RecentComparisonTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set("文字列", forKey: RecentComparisonStore.storageKey)

        let store = RecentComparisonStore(defaults: defaults)
        XCTAssertEqual(store.load(), [])
    }

    func testSaveTrimsBeyondMaxCount() {
        let (store, cleanup) = makeStore()
        defer { cleanup() }
        // リストは新着順(先頭が最新)。15件のうち最新10件が残る
        let many = (0..<15).map { item(left: "/dir\(14 - $0)", right: "/other") }

        store.save(many)
        let loaded = store.load()

        XCTAssertEqual(loaded.count, 10)
        XCTAssertEqual(loaded.first?.leftPath, "/dir14")
        XCTAssertEqual(loaded.last?.leftPath, "/dir5")
        XCTAssertFalse(loaded.contains { $0.leftPath == "/dir0" })
    }
}
