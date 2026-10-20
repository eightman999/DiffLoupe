//! diffloupe-edition: エディション(Free/Pro)の差し込み口。
//!
//! Swift `DiffLoupeEdition` プロトコル(`SwiftUI/DiffLoupe/UI/DiffLoupeEdition.swift`)に相当。
//! Swift 版は拡張点で `AnyView` を返すが、Rust 版は UI ツールキット(Win32/GTK)に依存しない
//! よう「提供する拡張機能の種別」を列挙値で返す設計にする。各 UI 層はこの列挙値を見て
//! 自身のツールキットのビューを差し込む。
//!
//! 公開版は `FreeEdition` のみを持つ。有償版(非公開リポジトリ)はこのトレイトを実装した
//! 独自エディションを UI 層へ渡す。

/// エディションが置き換え/追加を提供できる拡張点の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionPoint {
    /// テキスト差分の詳細ビュー(編集可能な差分ビューなど)
    TextDetailView,
    /// コピー計画シートの操作領域(同期の「実行」など)
    SyncPlanActions,
    /// 3-way 差分シート全体(マージ適用・保存など)
    ThreeWayView,
    /// 設定画面へ追加するセクション
    SettingsSection,
}

/// エディション(Free/Pro)の振る舞いを決めるトレイト。
///
/// UI ツールキット非依存。各 OS の UI 層は `provides()` で拡張点の有無を確認し、
/// 提供がある場合だけエディション実装側の差し込みビューを表示する。
/// `Send + Sync` を要求するのは、UI 層がワーカースレッドとの間でエディションを
/// 共有するため(参照の受け渡しのみ・状態変更は各実装の責務)。
pub trait Edition: Send + Sync {
    /// 表示名("Free" / "Pro")
    fn name(&self) -> &'static str;
    /// フォルダ比較の件数上限。None なら無制限
    fn max_folder_entries(&self) -> Option<usize>;

    /// 指定した拡張点のビューをエディションが提供するか。
    /// true を返した場合、UI 層は従来ビューの代わりにエディション側のビューを使う。
    /// Free 版はすべて false(従来ビューのみ)。
    fn provides(&self, _point: ExtensionPoint) -> bool {
        false
    }

    /// 未保存の編集があるか(確認ダイアログは出さない)。自動再比較の保留判定に使う。
    /// Free 版は編集機能がないため常に false。
    fn has_unsaved_changes(&self) -> bool {
        false
    }

    /// 比較開始・フォルダ変更・左右交換・履歴復元など、編集内容を失いうる操作の直前に呼ぶ。
    /// false を返すと操作を中止する(必要なら実装側で確認ダイアログを出し、
    /// 保存/破棄を済ませてから true を返す)。Free 版は常に true。
    fn confirm_discard_unsaved_changes(&self) -> bool {
        true
    }
}

/// 公開版のエディション。フォルダ比較は 2,000 件までで、拡張点はすべて従来ビューを使う。
#[derive(Debug, Default, Clone, Copy)]
pub struct FreeEdition;

impl Edition for FreeEdition {
    fn name(&self) -> &'static str {
        "Free"
    }
    fn max_folder_entries(&self) -> Option<usize> {
        Some(diffloupe_core::comparator::CompareOptions::FREE_MAX_ENTRIES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_edition_is_named_free() {
        assert_eq!(FreeEdition.name(), "Free");
    }

    #[test]
    fn free_edition_limits_folder_entries_to_2000() {
        assert_eq!(FreeEdition.max_folder_entries(), Some(2000));
    }

    #[test]
    fn free_edition_provides_no_extensions() {
        for p in [
            ExtensionPoint::TextDetailView,
            ExtensionPoint::SyncPlanActions,
            ExtensionPoint::ThreeWayView,
            ExtensionPoint::SettingsSection,
        ] {
            assert!(!FreeEdition.provides(p));
        }
    }

    #[test]
    fn free_edition_never_blocks_operations() {
        assert!(!FreeEdition.has_unsaved_changes());
        assert!(FreeEdition.confirm_discard_unsaved_changes());
    }

    /// Pro 側の実装例として: 拡張点を差し込めることを確認する。
    #[test]
    fn custom_edition_can_provide_extensions() {
        struct Pro;
        impl Edition for Pro {
            fn name(&self) -> &'static str {
                "Pro"
            }
            fn max_folder_entries(&self) -> Option<usize> {
                None
            }
            fn provides(&self, p: ExtensionPoint) -> bool {
                p == ExtensionPoint::TextDetailView
            }
        }
        let pro = Pro;
        assert_eq!(pro.name(), "Pro");
        assert_eq!(pro.max_folder_entries(), None);
        assert!(pro.provides(ExtensionPoint::TextDetailView));
        assert!(!pro.provides(ExtensionPoint::SyncPlanActions));
    }
}
