// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分ビュー向けの純粋ロジック。検索一致行の検出・コンテキスト行の折りたたみ・
// 表示アイテム構築・差分行ナビゲーションをUIから分離して定義する(テスト可能な分離)。
// DiffRow の API は変更せず、この層だけで拡張する。

import Foundation

public enum DiffDisplay {

    /// 折りたたみ対象とみなすコンテキスト行の最小連続数(既定 5)
    public static let collapseThreshold = 5

    // MARK: - 検索

    /// クエリを含む行のIDリスト(左右どちらかの元テキストが対象)。
    /// 片側のみの行は nil でない側のテキストを検索対象にする。
    /// 空クエリは常に空を返す(空検索は安全に解除される)。
    public static func searchHitRowIDs(in rows: [DiffRow], query: String, caseSensitive: Bool) -> [Int] {
        guard !query.isEmpty else { return [] }
        return rows.compactMap { row -> Int? in
            let left = row.leftText ?? ""
            let right = row.rightText ?? ""
            let contains: Bool
            if caseSensitive {
                contains = left.contains(query) || right.contains(query)
            } else {
                let options: String.CompareOptions = [.caseInsensitive]
                contains = left.range(of: query, options: options) != nil
                    || right.range(of: query, options: options) != nil
            }
            return contains ? row.id : nil
        }
    }

    // MARK: - コンテキスト折りたたみ

    /// 連続するコンテキスト行の折りたたみグループ(threshold 行以上)。差分行で区切られる。
    public struct FoldGroup: Equatable, Identifiable, Sendable {
        /// 先頭行の行ID。グループの安定したIDとして使う
        public let id: Int
        public let rowIDs: [Int]
        public let count: Int
    }

    /// コンテキスト行の連続ブロックのうち、threshold 行以上のものを折りたたみグループとして返す。
    public static func foldGroups(in rows: [DiffRow], threshold: Int = collapseThreshold) -> [FoldGroup] {
        var groups: [FoldGroup] = []
        var runCount = 0

        func flush(_ endIndex: Int) {
            if runCount >= threshold {
                let start = endIndex - runCount
                groups.append(FoldGroup(
                    id: rows[start].id,
                    rowIDs: (start..<endIndex).map { rows[$0].id },
                    count: runCount
                ))
            }
            runCount = 0
        }

        for (index, row) in rows.enumerated() {
            if row.kind == .context {
                runCount += 1
            } else {
                flush(index)
            }
        }
        flush(rows.count)
        return groups
    }

    /// 指定の行IDを1つ以上含む折りたたみグループIDの集合を返す(検索ヒット行の自動展開用)。
    /// 空の行IDリストは空集合を返す(クエリ空時の安全な解除)。
    public static func expandedGroupIDs(
        containingRowIDs rowIDs: [Int],
        in rows: [DiffRow],
        threshold: Int = collapseThreshold
    ) -> Set<Int> {
        guard !rowIDs.isEmpty else { return [] }
        let target = Set(rowIDs)
        return Set(
            foldGroups(in: rows, threshold: threshold)
                .filter { group in group.rowIDs.contains { target.contains($0) } }
                .map(\.id)
        )
    }

    // MARK: - 表示アイテム

    /// 折りたたみ適用後の表示アイテム。行または折りたたみヘッダ。
    public enum DisplayItem: Equatable, Identifiable, Sendable {
        case row(DiffRow)
        case collapsed(FoldGroup)

        public var id: Int {
            switch self {
            case .row(let row): return row.id
            case .collapsed(let group): return group.id
            }
        }
    }

    /// 折りたたみ適用後の表示アイテム列を返す。
    /// 差分行は常に表示される。コンテキスト行は折りたたみグループ内ならヘッダ1行に集約される。
    /// showAllContextRows が true のときは全行をそのまま返す(既存の全行表示を維持)。
    public static func displayItems(
        rows: [DiffRow],
        expandedGroupIDs: Set<Int>,
        showAllContextRows: Bool,
        threshold: Int = collapseThreshold
    ) -> [DisplayItem] {
        if showAllContextRows {
            return rows.map { .row($0) }
        }
        let groups = foldGroups(in: rows, threshold: threshold)
        var groupByFirstRowID: [Int: FoldGroup] = [:]
        for group in groups {
            groupByFirstRowID[group.rowIDs[0]] = group
        }

        var items: [DisplayItem] = []
        items.reserveCapacity(rows.count)
        var index = 0
        while index < rows.count {
            let row = rows[index]
            if row.kind == .context, let group = groupByFirstRowID[row.id] {
                if expandedGroupIDs.contains(group.id) {
                    for _ in group.rowIDs {
                        items.append(.row(rows[index]))
                        index += 1
                    }
                } else {
                    items.append(.collapsed(group))
                    index += group.count
                }
            } else {
                items.append(.row(row))
                index += 1
            }
        }
        return items
    }

    // MARK: - 差分行ナビゲーション

    /// added / removed / modified の行IDリスト(行順)。
    public static func differenceRowIDs(in rows: [DiffRow]) -> [Int] {
        rows.filter { $0.kind != .context }.map(\.id)
    }
}
