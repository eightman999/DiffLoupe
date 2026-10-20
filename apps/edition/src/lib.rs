//! diffloupe-edition: エディション(Free/Pro)の差し込み口。
//!
//! Swift 版 `DiffLoupeEdition` プロトコルに対応する。本クレートは UI ツールキットに
//! 依存しない部分だけを定義し、実際の拡張ビューの構築は各 OS のアプリ側
//! (Linux では `diffloupe-gtk` の `GtkEditionExtension` トレイト)が担う。
//!
//! Pro 版は別配布物として本トレイトを実装し、アプリ側のツールキット用拡張
//! トレイトも併せて実装することで、Free の標準ビューを Pro 拡張へ差し替える。

/// エディションが提供する拡張点の宣言。
/// UI 非依存なので「その拡張点を実装しているか」の能力フラグのみを持つ。
/// 実ビューの生成はツールキット側トレイト(GTK では `GtkEditionExtension`)が行う。
#[derive(Debug, Clone, Copy, Default)]
pub struct EditionCapabilities {
    /// テキスト詳細ビュー(編集可能差分ビュー等)を差し替えるか
    pub text_detail_view: bool,
    /// コピー計画シートの操作領域(同期実行など)を追加するか
    pub sync_plan_actions: bool,
    /// 3-way 差分シート全体を差し替えるか
    pub three_way_view: bool,
    /// 設定画面へ追加セクションを挿入するか
    pub extra_settings_section: bool,
}

/// DiffLoupe エディション。Swift 版 `DiffLoupeEdition` に対応する。
///
/// - `textDetailView` / `syncPlanActions` / `threeWayView` / `settingsSection`
///   (AnyView を返すビュー拡張点)は UI 非依存にできないため、ここでは
///   `capabilities()` で提供有無だけを宣言する。実ビューは各アプリが
///   ツールキット固有トレイト経由で取得する。
/// - `hasUnsavedChanges` / `confirmDiscardUnsavedChanges` はそのまま移植。
pub trait DiffLoupeEdition: Send + Sync {
    /// 表示名("Free" / "Pro")
    fn name(&self) -> &str;
    /// フォルダ比較の件数上限。`None` なら無制限。
    fn max_folder_entries(&self) -> Option<usize>;

    /// 拡張点の提供有無。実ビューの構築はツールキット側に委譲する。
    fn capabilities(&self) -> EditionCapabilities {
        EditionCapabilities::default()
    }

    /// 未保存の編集があるか(確認ダイアログは出さない)。
    /// 自動再比較の保留判定に使う。Free は常に false。
    fn has_unsaved_changes(&self) -> bool {
        false
    }

    /// 比較開始・フォルダ変更・左右交換・履歴復元など、編集内容を失いうる
    /// 操作の直前に呼ぶ。false を返すと操作を中止する(実装側で必要なら
    /// 確認ダイアログを出し、保存/破棄を済ませてから true を返す)。
    fn confirm_discard_unsaved_changes(&self) -> bool {
        true
    }
}

/// 公開版のエディション。フォルダ比較は 2000 件まで。
/// 拡張点はすべて従来ビューを使う(能力フラグはすべて false)。
pub struct FreeEdition;

impl DiffLoupeEdition for FreeEdition {
    fn name(&self) -> &str {
        "Free"
    }

    fn max_folder_entries(&self) -> Option<usize> {
        Some(diffloupe_core::CompareOptions::FREE_MAX_ENTRIES)
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
        let c = e.capabilities();
        assert!(!c.text_detail_view);
        assert!(!c.sync_plan_actions);
        assert!(!c.three_way_view);
        assert!(!c.extra_settings_section);
    }
}
