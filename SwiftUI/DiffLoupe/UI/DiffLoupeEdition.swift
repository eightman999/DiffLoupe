// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: エディション(Free/Pro)の差し込み口。件数上限と、差分ビュー・同期実行・3-way・設定への
// 拡張ビューと未保存編集の確認フックを定義する。拡張点の既定実装はnil(= Freeの従来ビューを表示)。公開版はFreeEditionのみを持ち、
// 有償版は別モジュールでこのプロトコルに準拠したエディションを渡す

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

@MainActor
public protocol DiffLoupeEdition: AnyObject {
    /// 表示名("Free" / "Pro")
    var name: String { get }
    /// フォルダ比較の件数上限。nilなら無制限
    var maxFolderEntries: Int? { get }

    // 拡張点: nil以外を返すと、対応する画面で従来のビューの代わりにそのビューを表示する
    /// テキスト差分の詳細ビュー(編集可能な差分ビューなど)
    func textDetailView(model: AppModel, entry: ComparedEntry) -> AnyView?
    /// コピー計画シートの操作領域(同期の「実行」など)
    func syncPlanActions(model: AppModel, plan: SyncPlan) -> AnyView?
    /// 3-way差分シート全体(マージ適用・保存など)
    func threeWayView(model: AppModel) -> AnyView?
    /// 設定画面へ追加するセクション
    func settingsSection(model: AppModel) -> AnyView?

    // 未保存の編集の保護(Freeは編集機能がないため既定のまま)
    /// 未保存の編集があるか(確認ダイアログは出さない)。自動再比較の保留判定に使う
    var hasUnsavedChanges: Bool { get }
    /// 比較開始・フォルダ変更・左右交換・履歴復元など、編集内容を失いうる操作の直前に呼ぶ。
    /// falseを返すと操作を中止する(必要なら実装側で確認ダイアログを出し、保存/破棄を済ませてからtrueを返す)
    func confirmDiscardUnsavedChanges() -> Bool
}

public extension DiffLoupeEdition {
    func textDetailView(model: AppModel, entry: ComparedEntry) -> AnyView? { nil }
    func syncPlanActions(model: AppModel, plan: SyncPlan) -> AnyView? { nil }
    func threeWayView(model: AppModel) -> AnyView? { nil }
    func settingsSection(model: AppModel) -> AnyView? { nil }
    var hasUnsavedChanges: Bool { false }
    func confirmDiscardUnsavedChanges() -> Bool { true }
}

/// 公開版のエディション。フォルダ比較は2000件までで、拡張点はすべて従来ビューを使う
public final class FreeEdition: DiffLoupeEdition {
    public let name = "Free"
    public let maxFolderEntries: Int? = FolderComparator.Options.freeMaxEntries

    public init() {}
}
