//! GTK 固有のエディション拡張点。
//!
//! `diffloupe-edition` の `DiffLoupeEdition` はツールキット非依存(名前・件数上限・
//! 未保存編集フック・能力フラグ)。実際の拡張ウィジェットはこちらのトレイトで供給する。
//! Pro 版は `DiffLoupeEdition` と本トレイトの両方を実装したオブジェクトを `AppConfig` で渡す。
//! Free ではすべて既定実装(= 従来ビューを使う)となる。
//!
//! 互換性: メソッドの追加は既定実装つきのみ(既存メソッドのシグネチャは変えない)。

use std::rc::Rc;

use diffloupe_core::{sync_plan::SyncPlan, ComparedEntry};
use gtk4::Widget;

use crate::model::AppModel;
use crate::window::MainWindow;

/// GTK ビュー差し替え用の拡張トレイト。Swift 版の AnyView を返す4メソッドに対応し、
/// Pro のメニュー・アクション追加用のフックを加えている。
/// ビューを返すメソッドは `None` を返すと従来ビューが使われる。
#[allow(unused_variables)] // 拡張点トレイト: 既定実装は引数を使わない
pub trait GtkEditionExtension {
    /// テキスト差分の詳細ビュー(編集可能な差分ビューなど)。
    /// `Edition::provides(TextDetailView)` が true のときだけ呼ばれ、ファイル選択のたびに
    /// 新しいウィジェットを返してよい(古いものは詳細ペインが破棄する)。
    fn text_detail_view(&self, model: &Rc<AppModel>, entry: &ComparedEntry) -> Option<Widget> {
        None
    }
    /// コピー計画シートの操作領域(同期の「実行」など)。方向を切り替えるたびに呼ばれる。
    fn sync_plan_actions(&self, model: &Rc<AppModel>, plan: &SyncPlan) -> Option<Widget> {
        None
    }
    /// 3-way差分シート全体(マージ適用・保存など)
    fn three_way_view(&self, model: &Rc<AppModel>) -> Option<Widget> {
        None
    }
    /// 設定画面へ追加するセクション
    fn settings_section(&self, model: &Rc<AppModel>) -> Option<Widget> {
        None
    }

    // ── 追加フック(メニュー・アクション・状態通知) ──

    /// メインウィンドウ構築直後に 1 回呼ばれる。アクション(`window.window.add_action`)や
    /// アクセラレータの登録、`MainWindow` の弱参照の保持に使う。
    /// 標準アクション登録の後に呼ばれるため、同名アクションで上書きもできる。
    fn window_ready(&self, window: &Rc<MainWindow>) {}

    /// ヘッダバーの左側(履歴・左右交換の右隣)へ追加するウィジェット(「プロジェクト」メニューなど)。
    fn header_widgets(&self, model: &Rc<AppModel>) -> Vec<Widget> {
        Vec::new()
    }

    /// 比較結果が変わった(完了・破棄)ときに呼ばれる。メニュー項目の有効/無効の更新に使う。
    fn comparison_changed(&self, model: &Rc<AppModel>) {}
}

/// Free エディション: 拡張ビューは一切提供しない。
pub struct FreeGtkEdition;

impl GtkEditionExtension for FreeGtkEdition {}
