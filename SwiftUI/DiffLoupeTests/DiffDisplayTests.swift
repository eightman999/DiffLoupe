// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分ビューの純粋ロジック(DiffDisplay)の仕様検証。
// 検索・コンテキスト折りたたみ・表示アイテム構築・差分行ナビゲーション

import XCTest
@testable import DiffLoupe

final class DiffDisplayTests: XCTestCase {

    /// テスト用の行列を構築する。
    /// 0..<2: context 2連続 / 2: modified / 3..<10: context 7連続 / 10: added / 11..<14: context 3連続
    private func makeRows() -> [DiffRow] {
        var rows: [DiffRow] = []
        var id = 0
        func add(_ kind: DiffRowKind, left: String?, right: String?) {
            rows.append(DiffRow(id: id, kind: kind, leftNumber: id + 1, leftText: left, rightNumber: id + 1, rightText: right))
            id += 1
        }
        add(.context, left: "c1", right: "c1")
        add(.context, left: "c2", right: "c2")
        add(.modified, left: "m1", right: "m1'")
        for i in 0..<7 { add(.context, left: "g\(i)", right: "g\(i)") }
        add(.added, left: nil, right: "new")
        for i in 1...3 { add(.context, left: "x\(i)", right: "x\(i)") }
        return rows
    }

    // MARK: - 検索

    func testSearchHitRowIDs() {
        let rows = makeRows()
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "g", caseSensitive: true), Array(3...9))
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "g0", caseSensitive: true), [3])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "new", caseSensitive: true), [10])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "m1", caseSensitive: true), [2])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "x", caseSensitive: true), [11, 12, 13])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "z", caseSensitive: true), [])
    }

    func testSearchCaseInsensitive() {
        let rows = [
            DiffRow(id: 0, kind: .context, leftNumber: 1, leftText: "Hello", rightNumber: 1, rightText: "Hello"),
            DiffRow(id: 1, kind: .context, leftNumber: 2, leftText: "world", rightNumber: 2, rightText: "WORLD"),
        ]
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "hello", caseSensitive: false), [0])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "world", caseSensitive: false), [1])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "hello", caseSensitive: true), [])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "none", caseSensitive: false), [])
    }

    func testEmptyQueryReturnsEmpty() {
        let rows = makeRows()
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "", caseSensitive: false), [])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "", caseSensitive: true), [])
    }

    func testSearchUsesNonNullSide() {
        let rows = [
            DiffRow(id: 0, kind: .added, leftNumber: nil, leftText: nil, rightNumber: 1, rightText: "added-content"),
            DiffRow(id: 1, kind: .removed, leftNumber: 2, leftText: "removed-content", rightNumber: nil, rightText: nil),
        ]
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "added", caseSensitive: true), [0])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "removed", caseSensitive: true), [1])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "removed", caseSensitive: true), [1])
        XCTAssertEqual(DiffDisplay.searchHitRowIDs(in: rows, query: "content", caseSensitive: true), [0, 1])
    }

    // MARK: - 折りたたみ

    func testFoldGroups() {
        let rows = makeRows()
        let groups = DiffDisplay.foldGroups(in: rows)
        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups.first?.count, 7)
        XCTAssertEqual(groups.first?.rowIDs, Array(3...9))
    }

    func testFoldGroupsBelowThreshold() {
        let rows = makeRows()
        // threshold 4 → 7連続のみ
        XCTAssertEqual(DiffDisplay.foldGroups(in: rows, threshold: 4).count, 1)
        // threshold 2 → 2連続・7連続・3連続
        let small = DiffDisplay.foldGroups(in: rows, threshold: 2)
        XCTAssertEqual(small.count, 3)
        XCTAssertEqual(small.map(\.count), [2, 7, 3])
        // threshold 8 → どれも対象外
        XCTAssertEqual(DiffDisplay.foldGroups(in: rows, threshold: 8).count, 0)
    }

    func testFoldGroupsSplitsAtDifferenceRows() {
        // 差分行で区切られる: 5連続 → added → 5連続
        var rows: [DiffRow] = []
        for i in 0..<5 {
            rows.append(DiffRow(id: i, kind: .context, leftNumber: i + 1, leftText: "a\(i)", rightNumber: i + 1, rightText: "a\(i)"))
        }
        rows.append(DiffRow(id: 5, kind: .added, leftNumber: nil, leftText: nil, rightNumber: 6, rightText: "new"))
        for i in 6..<11 {
            rows.append(DiffRow(id: i, kind: .context, leftNumber: i + 1, leftText: "b\(i)", rightNumber: i + 1, rightText: "b\(i)"))
        }
        let groups = DiffDisplay.foldGroups(in: rows)
        XCTAssertEqual(groups.count, 2)
        XCTAssertEqual(groups[0].rowIDs, Array(0..<5))
        XCTAssertEqual(groups[1].rowIDs, Array(6..<11))
    }

    func testFoldGroupsOnlyContextRows() {
        let rows = [
            DiffRow(id: 0, kind: .modified, leftNumber: 1, leftText: "m", rightNumber: 1, rightText: "m'"),
            DiffRow(id: 1, kind: .removed, leftNumber: 2, leftText: "r", rightNumber: nil, rightText: nil),
        ]
        XCTAssertEqual(DiffDisplay.foldGroups(in: rows).count, 0)
    }

    // MARK: - 表示アイテム

    func testDisplayItemsCollapsedByDefault() {
        let items = DiffDisplay.displayItems(rows: makeRows(), expandedGroupIDs: [], showAllContextRows: false)
        // 2連続・3連続は非折りたたみ、7連続のみヘッダに集約される
        XCTAssertEqual(items.count, 8)
        let collapsedCount = items.filter { if case .collapsed = $0 { return true }; return false }.count
        XCTAssertEqual(collapsedCount, 1)
        let rowCount = items.filter { if case .row = $0 { return true }; return false }.count
        XCTAssertEqual(rowCount, 7)
    }

    func testDisplayItemsExpandedShowsAllRows() {
        let items = DiffDisplay.displayItems(rows: makeRows(), expandedGroupIDs: [3], showAllContextRows: false)
        XCTAssertEqual(items.count, 14)
        XCTAssertTrue(items.allSatisfy { if case .row = $0 { return true }; return false })
    }

    func testDisplayItemsShowAllContextRows() {
        let items = DiffDisplay.displayItems(rows: makeRows(), expandedGroupIDs: [], showAllContextRows: true)
        XCTAssertEqual(items.count, 14)
        XCTAssertTrue(items.allSatisfy { if case .row = $0 { return true }; return false })
    }

    func testDisplayItemsCollapsedGroupHasFirstRowID() {
        let items = DiffDisplay.displayItems(rows: makeRows(), expandedGroupIDs: [], showAllContextRows: false)
        let collapsed = try? XCTUnwrap(items.compactMap { item -> DiffDisplay.FoldGroup? in
            if case .collapsed(let group) = item { return group }
            return nil
        }.first)
        XCTAssertEqual(collapsed?.id, 3)
        XCTAssertEqual(collapsed?.count, 7)
    }

    // MARK: - 検索ヒットの折りたたみ自動展開

    func testExpandedGroupIDsContainingRowIDs() {
        let rows = makeRows()
        // makeRows の折りたたみグループは 3..9 の7連続のみ
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [5], in: rows), [3])
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [3, 9], in: rows), [3])
        // グループ外の行・空リストは空集合
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [0], in: rows), [])
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [], in: rows), [])
        // 差分行はグループに含まれない
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [2, 10], in: rows), [])
    }

    func testExpandedGroupIDsAcrossMultipleGroups() {
        let rows = makeRows()
        XCTAssertEqual(DiffDisplay.foldGroups(in: rows, threshold: 2).map(\.id), [0, 3, 11])
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [4, 12], in: rows, threshold: 2), [3, 11])
        XCTAssertEqual(DiffDisplay.expandedGroupIDs(containingRowIDs: [1], in: rows, threshold: 2), [0])
    }

    func testExpandedGroupIDsRevealHitRowsInDisplayItems() {
        let rows = makeRows()
        // 検索ヒット "g3" (id=6) は折りたたみグループ 3..9 内にある
        let hitIDs = DiffDisplay.searchHitRowIDs(in: rows, query: "g3", caseSensitive: true)
        XCTAssertEqual(hitIDs, [6])
        let expanded = DiffDisplay.expandedGroupIDs(containingRowIDs: hitIDs, in: rows)
        let items = DiffDisplay.displayItems(rows: rows, expandedGroupIDs: expanded, showAllContextRows: false)
        XCTAssertTrue(items.contains { item in
            if case .row(let row) = item { return row.id == 6 }
            return false
        })
        // 展開しない場合はヒット行が折りたたみヘッダに隠れる(検索なしの既定挙動)
        let collapsedItems = DiffDisplay.displayItems(rows: rows, expandedGroupIDs: [], showAllContextRows: false)
        XCTAssertFalse(collapsedItems.contains { item in
            if case .row(let row) = item { return row.id == 6 }
            return false
        })
    }

    // MARK: - 差分行ナビゲーション

    func testDifferenceRowIDs() {
        let rows = makeRows()
        XCTAssertEqual(DiffDisplay.differenceRowIDs(in: rows), [2, 10])
    }

    func testDifferenceRowIDsAllKinds() {
        let rows = [
            DiffRow(id: 0, kind: .context, leftNumber: 1, leftText: "c", rightNumber: 1, rightText: "c"),
            DiffRow(id: 1, kind: .added, leftNumber: nil, leftText: nil, rightNumber: 2, rightText: "a"),
            DiffRow(id: 2, kind: .removed, leftNumber: 3, leftText: "r", rightNumber: nil, rightText: nil),
            DiffRow(id: 3, kind: .modified, leftNumber: 4, leftText: "m", rightNumber: 4, rightText: "m'"),
        ]
        XCTAssertEqual(DiffDisplay.differenceRowIDs(in: rows), [1, 2, 3])
    }

    func testDifferenceRowIDsEmpty() {
        XCTAssertEqual(DiffDisplay.differenceRowIDs(in: []), [])
    }
}
