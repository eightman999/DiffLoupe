//! Win32 固有のエディション拡張点とアプリ構成(`AppConfig`)。
//!
//! `diffloupe-edition` の `Edition` はツールキット非依存(名前・件数上限・
//! 未保存編集フック・能力フラグ)。実際の拡張ビューはこちらのトレイトで供給する。
//! Pro 版は `Edition` と本トレイトの両方を実装したオブジェクトを `AppConfig` に載せて渡す。
//! Free では `extension: None`(= 従来ビューを使う)。設計は `docs/RUST_EDITION.md`。

use std::path::PathBuf;
use std::sync::Arc;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::WindowsAndMessaging::HMENU;

use diffloupe_edition::Edition;

use crate::appmodel::AppModel;
use crate::extmodel::{ExtMenu, ExtMenuItem};

/// 拡張ビューの生成結果。
///
/// `hwnd` は `parent` の子ウィンドウとして拡張側が作成したもの。
/// 以後の配置・表示・破棄は UI 側が管理する(拡張側は自分の子孫ウィンドウだけを管理する)。
/// `height` は設定セクションなど「高さを要求する」用途の希望値(論理 px)。
/// 0 なら親が決める(領域いっぱい)。
pub struct ExtView {
    pub hwnd: HWND,
    pub height: i32,
}

/// 拡張ビュー生成時のコンテキスト。
/// すべて UI スレッド上で構築され、その場限りで有効(保持してはいけない)。
pub struct ExtContext<'a> {
    /// 拡張ビューの親となるウィンドウ(詳細領域ならメインウィンドウ、
    /// コピー計画・設定ならそれぞれのシート)
    pub parent: HWND,
    /// メインウィンドウ(再比較要求などアプリ全体への連絡用)
    pub main: HWND,
    /// アプリ状態(読み取り専用。選択中エントリ・コピー計画などを参照する)
    pub model: &'a AppModel,
    /// UI フォント(生成した子コントロールへ適用すること)
    pub font: HFONT,
    /// メインウィンドウの DPI
    pub dpi: u32,
}

/// Win32 ビュー差し替え用の拡張トレイト。Swift 版の `AnyView` を返す拡張メソッドに対応。
/// いずれも `None` を返すと従来ビューが使われる。
///
/// すべてのメソッドは UI スレッドで呼ばれる。`Send`/`Sync` は要求しない。
/// コマンド ID は `crate::extmodel::EXT_CMD_MIN..=EXT_CMD_MAX` の範囲を使うこと。
#[allow(unused_variables)]
pub trait Win32EditionExtension {
    /// テキスト差分の詳細ビュー(編集可能な差分ビューなど)。
    /// 選んだエントリがテキスト差分のとき、従来の読み取り専用グリッドの代わりに配置される。
    /// `ctx.model.selected_entry()` で対象エントリを参照できる。
    fn text_detail_view(&self, ctx: &ExtContext) -> Option<ExtView> {
        None
    }

    /// コピー計画シート下部の操作領域(同期の「実行」など)。
    /// `ctx.model.sync_plan` に現在の計画がある。計画の方向が変わると作り直される。
    fn sync_plan_actions(&self, ctx: &ExtContext) -> Option<ExtView> {
        None
    }

    /// 3-way 差分シートのコンテンツ全体(マージ適用・保存など)。
    /// `ctx.model.three_way_*` に選択済みファイルがある。
    fn three_way_view(&self, ctx: &ExtContext) -> Option<ExtView> {
        None
    }

    /// 設定画面へ追加するセクション(幅は設定画面のコンテンツ幅いっぱい、
    /// 高さは `ExtView.height` を使う)
    fn settings_section(&self, ctx: &ExtContext) -> Option<ExtView> {
        None
    }

    /// メニューバーへ追加するトップレベルメニュー(「ヘルプ」の手前に挿入)。
    fn extra_menus(&self) -> Vec<ExtMenu> {
        Vec::new()
    }

    /// 拡張メニューのコマンドが選ばれた。処理したら true(未処理なら false でフォールバック)。
    fn menu_command(&self, id: u16, main: HWND, model: &mut AppModel) -> bool {
        false
    }

    /// 拡張メニューの項目の有効/無効(メニューが開かれる直前に項目ごとに呼ぶ)。
    fn menu_item_enabled(&self, id: u16, model: &AppModel) -> bool {
        true
    }

    /// 動的サブメニューの中身(開かれる直前に呼ぶ)。`submenu_id` は
    /// `ExtMenuItem::submenu` で宣言した識別子。`ExtMenuItem` の id は
    /// `EXT_CMD_MIN..=EXT_CMD_MAX` または `separator()`。
    fn build_submenu(&self, submenu_id: u16, model: &AppModel) -> Vec<ExtMenuItem> {
        Vec::new()
    }

    /// 拡張のポップアップメニューが開かれる直前(タイトルの更新など任意の調整用)。
    fn init_menu_popup(&self, menu: HMENU, model: &AppModel) {}
}

/// スモークテスト実装の関数型: `(config, left, right, out_dir) -> exit_code`
pub type SmokeFn = fn(&AppConfig, PathBuf, PathBuf, PathBuf) -> i32;

/// アプリ起動構成。`win::run` に渡す。
///
/// Free 版は [`AppConfig::free`]、Pro 版はエディション・拡張・表示名・設定先を
/// 差し替えて使う。アプリ ID は設定ディレクトリ名(「DiffLoupe」/「DiffLoupe Pro」)。
pub struct AppConfig {
    /// ウィンドウタイトル・アプリ ID(設定ディレクトリ名)
    pub display_name: String,
    /// バージョン情報ダイアログの本文
    pub about_text: String,
    /// エディション(比較上限・拡張点の宣言・未保存確認)
    pub edition: Arc<dyn Edition>,
    /// Win32 拡張ビュー。None ならすべて従来ビュー
    pub extension: Option<Arc<dyn Win32EditionExtension>>,
    /// 設定の保存先ディレクトリ。None なら `%APPDATA%\<display_name>`
    pub settings_dir: Option<PathBuf>,
    /// スモークテスト実装の差し替え(Pro は自前の検証手順を持つ)。
    /// `(config, left, right, out_dir) -> exit_code`
    pub smoke: Option<SmokeFn>,
}

impl AppConfig {
    /// Free 版構成(従来どおりの振る舞い)
    pub fn free() -> Self {
        Self {
            display_name: crate::strings::TITLE.to_string(),
            about_text: crate::strings::ABOUT_TEXT.to_string(),
            edition: Arc::new(diffloupe_edition::FreeEdition),
            extension: None,
            settings_dir: None,
            smoke: None,
        }
    }
}
