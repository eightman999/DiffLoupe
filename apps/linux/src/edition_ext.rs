//! GTK 固有のエディション拡張点。
//!
//! `diffloupe-edition` の `DiffLoupeEdition` はツールキット非依存(名前・件数上限・
//! 未保存編集フック・能力フラグ)。実際の拡張ウィジェットはこちらのトレイトで供給する。
//! Pro 版は `DiffLoupeEdition` と本トレイトの両方を実装したオブジェクトを渡す。
//! Free ではすべて既定実装(= 従来ビューを使う)となる。

use diffloupe_core::{sync_plan::SyncPlan, ComparedEntry};
use gtk4::Widget;

use crate::model::AppModel;
use std::rc::Rc;

/// GTK ビュー差し替え用の拡張トレイト。Swift 版の AnyView を返す4メソッドに対応。
/// いずれも `None` を返すと従来ビューが使われる。
#[allow(dead_code)] // 拡張点トレイト: Pro 実装側で使用(Free は既定実装のみ)
pub trait GtkEditionExtension {
    /// テキスト差分の詳細ビュー(編集可能な差分ビューなど)
    fn text_detail_view(&self, _model: &Rc<AppModel>, _entry: &ComparedEntry) -> Option<Widget> {
        None
    }
    /// コピー計画シートの操作領域(同期の「実行」など)
    fn sync_plan_actions(&self, _model: &Rc<AppModel>, _plan: &SyncPlan) -> Option<Widget> {
        None
    }
    /// 3-way差分シート全体(マージ適用・保存など)
    fn three_way_view(&self, _model: &Rc<AppModel>) -> Option<Widget> {
        None
    }
    /// 設定画面へ追加するセクション
    fn settings_section(&self, _model: &Rc<AppModel>) -> Option<Widget> {
        None
    }
}

/// Free エディション: 拡張ビューは一切提供しない。
pub struct FreeGtkEdition;

impl GtkEditionExtension for FreeGtkEdition {}
