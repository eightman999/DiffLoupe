// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較履歴。左右パス・左右git ref・日時を保持するCodableモデルと、
// UserDefaultsへ保存/復元する純粋なストアを定義する(UI・AppModelから分離してテスト可能にする)

import Foundation

/// 実行した1回の比較の履歴。同一ペアは先頭へ移動し、最大件数を超えた古いものは破棄される。
struct RecentComparison: Codable, Equatable, Hashable, Identifiable {
    let leftPath: String
    let rightPath: String
    let leftGitRef: String?
    let rightGitRef: String?
    let date: Date

    /// 同一ペア判定用の識別子(左右パスを順序込みで結合)。
    /// recordで同一ペアが重複しないため、一覧内では常に一意。
    var id: String { "\(leftPath)|\(rightPath)" }
}

/// 比較履歴の保存/復元を担う純粋ストア。
/// - 最大件数: `RecentComparisonStore.maxCount`(先頭が最新)
/// - 同一ペア(左右パス順序一致)は先頭へ移動
/// - UserDefaultsの値が壊れていても空として安全に扱う
struct RecentComparisonStore {

    static let maxCount = 10
    static let storageKey = "recentComparisons"

    private let defaults: UserDefaults
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        self.encoder = encoder
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder
    }

    /// UserDefaultsから履歴を復元する。データが壊れている・型が違う場合は空を返す(安全に無視)。
    func load() -> [RecentComparison] {
        guard let data = defaults.data(forKey: Self.storageKey) else { return [] }
        guard let list = try? decoder.decode([RecentComparison].self, from: data) else { return [] }
        return Array(list.prefix(Self.maxCount))
    }

    /// 履歴を保存する。最大件数を超える分は切り詰める。
    func save(_ comparisons: [RecentComparison]) {
        let trimmed = Array(comparisons.prefix(Self.maxCount))
        guard let data = try? encoder.encode(trimmed) else { return }
        defaults.set(data, forKey: Self.storageKey)
    }

    /// 比較を記録する。同一ペアは取り除いてから先頭に挿入し、最大件数に切り詰める。
    /// 保存後の一覧を返す(呼び出し側の表示状態に反映するため)。
    @discardableResult
    func record(_ comparison: RecentComparison) -> [RecentComparison] {
        var list = load().filter {
            $0.leftPath != comparison.leftPath || $0.rightPath != comparison.rightPath
        }
        list.insert(comparison, at: 0)
        list = Array(list.prefix(Self.maxCount))
        save(list)
        return list
    }
}
