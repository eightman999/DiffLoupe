// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分ビュー向けの純粋ロジック。検索一致行の検出・コンテキスト行の折りたたみ・
// 表示アイテム構築・差分行ナビゲーションをUIから分離して定義する(テスト可能な分離)。
// DiffRow の API は変更せず、この層だけで拡張する。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

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
        return diffloupeSearchHitRowIds(
            rows: rows.map { $0.ffi }, query: query, caseSensitive: caseSensitive
        ).map { Int($0) }
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
        diffloupeFoldGroups(rows: rows.map { $0.ffi }, threshold: UInt64(threshold))
            .map { FoldGroup(id: Int($0.id), rowIDs: $0.rowIds.map { Int($0) }, count: Int($0.count)) }
    }

    /// 指定の行IDを1つ以上含む折りたたみグループIDの集合を返す(検索ヒット行の自動展開用)。
    /// 空の行IDリストは空集合を返す(クエリ空時の安全な解除)。
    public static func expandedGroupIDs(
        containingRowIDs rowIDs: [Int],
        in rows: [DiffRow],
        threshold: Int = collapseThreshold
    ) -> Set<Int> {
        guard !rowIDs.isEmpty else { return [] }
        return Set(diffloupeExpandedGroupIds(
            rowIds: rowIDs.map { UInt64($0) },
            rows: rows.map { $0.ffi },
            threshold: UInt64(threshold)
        ).map { Int($0) })
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
        diffloupeDisplayItems(
            rows: rows.map { $0.ffi },
            expandedGroupIds: expandedGroupIDs.map { UInt64($0) },
            showAllContextRows: showAllContextRows,
            threshold: UInt64(threshold)
        ).map { item in
            switch item {
            case .row(let row):
                return .row(DiffRow(row))
            case .collapsed(let group):
                return .collapsed(FoldGroup(
                    id: Int(group.id),
                    rowIDs: group.rowIds.map { Int($0) },
                    count: Int(group.count)
                ))
            }
        }
    }

    // MARK: - 差分行ナビゲーション

    /// added / removed / modified の行IDリスト(行順)。
    public static func differenceRowIDs(in rows: [DiffRow]) -> [Int] {
        diffloupeDifferenceRowIds(rows: rows.map { $0.ffi }).map { Int($0) }
    }
}
