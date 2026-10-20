//! diffloupe-edition: エディション(Free/Pro)の差し込み口。ツールキット非依存。
//!
//! Swift `DiffLoupeEdition` プロトコルに相当する。Swift 版は拡張点で `AnyView` を返すが、
//! Rust 版は UI ツールキット(GTK / Win32)に依存しないよう、「どの拡張点を提供するか」を
//! [`ExtensionPoint`] で宣言するだけにする。実ビューの構築は各アプリ側が持つ。
//!
//! - GTK 固有: `apps/linux/src/edition_ext.rs` の `GtkEditionExtension`
//! - Win32 固有: `apps/windows` 側
//!
//! 公開版は [`FreeEdition`] のみ。Pro は別リポジトリでこのトレイトを実装し、
//! 各アプリのツールキット用拡張トレイトと併せて UI 層へ渡す。設計は `docs/RUST_EDITION.md`。

use diffloupe_core::comparator::CompareOptions;

/// エディションが置き換え/追加を提供できる拡張点の種別。
///
/// `non_exhaustive`: 拡張点の追加は破壊的変更にならない。`match` には `_` を置くこと。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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

impl ExtensionPoint {
    /// 既知の拡張点すべて。
    pub const ALL: [ExtensionPoint; 4] = [
        ExtensionPoint::TextDetailView,
        ExtensionPoint::SyncPlanActions,
        ExtensionPoint::ThreeWayView,
        ExtensionPoint::SettingsSection,
    ];
}

/// 拡張点の提供有無のスナップショット(能力フラグ)。[`Edition::capabilities`] が返す。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EditionCapabilities {
    pub text_detail_view: bool,
    pub sync_plan_actions: bool,
    pub three_way_view: bool,
    pub extra_settings_section: bool,
}

/// エディション(Free/Pro)の振る舞いを決めるトレイト。
///
/// 実装が必須なのは [`name`](Edition::name) と [`max_folder_entries`](Edition::max_folder_entries) だけ。
/// 他は既定実装があり、Pro は必要な項目だけ上書きする。`Send + Sync` を要求するのは、
/// UI 層がワーカースレッドとエディションを共有するため。
pub trait Edition: Send + Sync {
    /// 表示名("Free" / "Pro")
    fn name(&self) -> &str;

    /// フォルダ比較の件数上限。`None` なら無制限。
    fn max_folder_entries(&self) -> Option<usize>;

    /// 指定した拡張点のビューをエディションが提供するか。
    /// true の場合、UI 層は従来ビューの代わりにエディション側のビューを使う。
    fn provides(&self, _point: ExtensionPoint) -> bool {
        false
    }

    /// 拡張点の提供有無をまとめたもの。既定は [`provides`](Edition::provides) から導出する。
    fn capabilities(&self) -> EditionCapabilities {
        EditionCapabilities {
            text_detail_view: self.provides(ExtensionPoint::TextDetailView),
            sync_plan_actions: self.provides(ExtensionPoint::SyncPlanActions),
            three_way_view: self.provides(ExtensionPoint::ThreeWayView),
            extra_settings_section: self.provides(ExtensionPoint::SettingsSection),
        }
    }

    /// 未保存の編集があるか(確認ダイアログは出さない)。自動再比較の保留判定に使う。
    fn has_unsaved_changes(&self) -> bool {
        false
    }

    /// 比較開始・フォルダ変更・左右交換・履歴復元など、編集内容を失いうる操作の直前に呼ぶ。
    /// false を返すと操作を中止する(実装側で確認ダイアログを出し、保存/破棄を済ませてから
    /// true を返す)。
    fn confirm_discard_unsaved_changes(&self) -> bool {
        true
    }
}

/// Swift 版の名前に合わせた別名(GTK 側が使う)。同一のトレイト。
pub use Edition as DiffLoupeEdition;

/// 公開版のエディション。フォルダ比較は 2,000 件までで、拡張点はすべて従来ビューを使う。
#[derive(Debug, Default, Clone, Copy)]
pub struct FreeEdition;

impl Edition for FreeEdition {
    fn name(&self) -> &str {
        "Free"
    }

    fn max_folder_entries(&self) -> Option<usize> {
        Some(CompareOptions::FREE_MAX_ENTRIES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_edition_matches_swift_constants() {
        let e = FreeEdition;
        assert_eq!(e.name(), "Free");
        assert_eq!(e.max_folder_entries(), Some(2000));
        assert!(!e.has_unsaved_changes());
        assert!(e.confirm_discard_unsaved_changes());
    }

    #[test]
    fn free_edition_provides_no_extensions() {
        for p in ExtensionPoint::ALL {
            assert!(!FreeEdition.provides(p));
        }
        assert_eq!(FreeEdition.capabilities(), EditionCapabilities::default());
    }

    #[test]
    fn custom_edition_can_provide_extensions() {
        struct Pro;
        impl Edition for Pro {
            fn name(&self) -> &str {
                "Pro"
            }
            fn max_folder_entries(&self) -> Option<usize> {
                None
            }
            fn provides(&self, p: ExtensionPoint) -> bool {
                matches!(p, ExtensionPoint::TextDetailView | ExtensionPoint::SettingsSection)
            }
        }
        let c = Pro.capabilities();
        assert!(c.text_detail_view && c.extra_settings_section);
        assert!(!c.sync_plan_actions && !c.three_way_view);
        assert_eq!(Pro.max_folder_entries(), None);
    }

    #[test]
    fn alias_is_the_same_trait() {
        fn takes(_: &dyn DiffLoupeEdition) {}
        takes(&FreeEdition);
        let b: Box<dyn Edition> = Box::new(FreeEdition);
        takes(b.as_ref());
    }
}
