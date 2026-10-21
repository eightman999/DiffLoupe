//! 日本語 UI 文字列。SwiftUI 版(`SwiftUI/DiffLoupe/UI/`)の文言を正典として移植する。
//! 正典セットの網羅を優先し、UI 配線待ちの文言も保持する。
#![allow(dead_code)]
//! `Finderに表示` だけは Linux 向けに「ファイルマネージャで表示」に読み替える。

pub const APP_NAME: &str = "DiffLoupe";

// ツールバー
pub const COMPARE: &str = "比較";
pub const CANCEL: &str = "キャンセル";
pub const SWAP_TOOLTIP: &str = "左右を交換 (Ctrl+Shift+S)";
pub const HISTORY: &str = "履歴";
pub const HISTORY_TOOLTIP: &str = "比較履歴から復元";
pub const HISTORY_EMPTY: &str = "履歴はまだありません";
pub const THREE_WAY: &str = "3-way差分";
pub const THREE_WAY_TOOLTIP: &str = "3つのファイルを3-way差分で比較します";
pub const LEFT_FOLDER: &str = "左フォルダ";
pub const RIGHT_FOLDER: &str = "右フォルダ";
pub const PATH_PLACEHOLDER: &str = "未選択";
pub const SELECT: &str = "選択";
pub const SELECT_LEFT_FOLDER: &str = "左側のフォルダを選択してください";
pub const SELECT_RIGHT_FOLDER: &str = "右側のフォルダを選択してください";
pub const SETTINGS: &str = "設定";

// git ref ピッカー
pub const WORKING_TREE: &str = "作業ツリー";
pub const ENTER_REF: &str = "任意のrefを入力…";
pub const REF_NAME: &str = "ref名を入力";
pub const REF_PLACEHOLDER: &str = "例: origin/main, v1.0.0, コミットハッシュ";
pub const APPLY: &str = "適用";

// ステータスフィルタ / 検索
pub const SEARCH_PLACEHOLDER: &str = "相対パスを検索";
pub const SEARCH_CLEAR_TOOLTIP: &str = "検索をクリア";
pub const FILTER_LABEL: &str = "表示";
pub const FILTER_TOOLTIP: &str = "表示する状態を選択";
pub const FILTER_ALL: &str = "すべて";
pub const FILTER_ALL_DIFFS: &str = "差分すべて";
pub const FILTER_IDENTICAL: &str = "一致";
pub const FILTER_DIFFERENT: &str = "相違";
pub const FILTER_LEFT_ONLY: &str = "左のみ";
pub const FILTER_RIGHT_ONLY: &str = "右のみ";
pub const FILTER_ERROR: &str = "エラー";
pub const STATUS_HAS_DIFFS: &str = "差分あり";

// ツリー空状態
pub const EMPTY_RESULT: &str = "比較結果がここに表示されます";
pub const EMPTY_NO_DIFFS: &str = "差分はありません";
pub const EMPTY_NO_MATCH: &str = "絞り込み条件に一致する項目はありません";
pub const EMPTY_NO_FILES: &str = "対象ファイルがありません";

// コンテキストメニュー
pub const SIDE_LEFT: &str = "左側";
pub const SIDE_RIGHT: &str = "右側";
pub const OPEN: &str = "開く";
pub const REVEAL: &str = "ファイルマネージャで表示";
pub const COPY_PATH: &str = "パスをコピー";

// ステータスバー
pub const STATUS_PROMPT: &str = "左右のフォルダを選択して「比較」を押してください";
pub const WATCH_ACTIVE: &str = "自動再比較 監視中";
pub const WATCH_ACTIVE_TOOLTIP: &str =
    "フォルダの変更を監視しています。変更を検知すると自動で再比較します。";
pub const WATCH_PAUSED: &str = "自動再比較 一時停止";
pub fn watch_pause_reason(max: usize) -> String {
    format!("自動再比較が{max}回連続したため一時停止しました。設定のトグルを再度オンにするか、手動で比較してください。")
}
pub fn status_scanning(found: usize) -> String {
    format!("走査中… {found} 件")
}
pub fn status_comparing(done: usize, total: usize) -> String {
    format!("内容比較中… {done}/{total}")
}
pub fn status_renames(done: usize, total: usize) -> String {
    format!("リネーム/移動を検出中… {done}/{total}")
}
pub fn status_failed(msg: &str) -> String {
    format!("比較に失敗しました: {msg}")
}

// Free 版上限アラート
pub const LIMIT_TITLE: &str = "Free版の比較上限に達しました";
pub const OPEN_SETTINGS: &str = "設定を開く";
pub const CLOSE: &str = "閉じる";

// 詳細ペイン
pub const DETAIL_HEADER: &str = "ファイル差分";
pub const META_SIZE: &str = "サイズ";
pub const META_MTIME: &str = "変更日時";
pub const META_PERM: &str = "パーミッション";
pub const RENAME_CONFIRMED: &str = "リネーム/移動の候補(確定)";
pub const RENAME_CANDIDATE: &str = "リネーム/移動の候補";
pub fn rename_same_count(n: usize) -> String {
    format!("(同内容 {n}件)")
}
pub fn structured_badge(fmt: &str) -> String {
    format!("{fmt}正規化で比較")
}
pub const STRUCTURED_FAIL_BADGE: &str = "構造化正規化に失敗";
pub fn structured_badge_tooltip(fmt: &str) -> String {
    format!("{fmt}の表記揺れを正規化して比較しています")
}
pub const STRUCTURED_FAIL_TOOLTIP: &str =
    "片側または両側のファイルを構造化パースできなかったため、元のテキストで比較しています。";

pub const PREV_DIFF: &str = "前の差分";
pub const NEXT_DIFF: &str = "次の差分";
pub const SEARCH: &str = "検索";
pub const PREV_MATCH: &str = "前の一致";
pub const NEXT_MATCH: &str = "次の一致";
pub const CASE_TOGGLE_TOOLTIP: &str = "大文字と小文字を区別する";
pub const SHOW_ALL: &str = "全行表示";
pub const SHOW_ALL_TOOLTIP: &str = "コンテキスト行の折りたたみを無効にする";
pub fn fold_label(count: usize) -> String {
    format!("{count} 行を表示")
}

pub const MSG_SELECT_FILE: &str = "ツリーからファイルを選択してください";
pub const MSG_COMPUTING: &str = "差分を計算中…";
pub const MSG_IS_DIRECTORY: &str = "フォルダです。配下のファイルを選択してください";
pub const MSG_LEFT_ONLY: &str = "左フォルダにのみ存在します";
pub const MSG_RIGHT_ONLY: &str = "右フォルダにのみ存在します";
pub fn msg_load_error(e: &str) -> String {
    format!("読み込みエラー: {e}")
}
pub const MSG_NO_TARGET: &str = "比較対象がありません";
pub const MSG_IDENTICAL: &str = "内容は一致しています";
pub const MSG_TOO_LARGE: &str = "ファイルが大きすぎるため差分表示できません(上限32MiB)";
pub fn msg_too_many_lines(max: usize) -> String {
    format!("行数が多すぎるため差分計算を省略しました(上限 計{max}行)")
}
pub const MSG_BINARY: &str = "バイナリファイル";
pub const DASH: &str = "—";

// Hex
pub fn hex_side_size(size: &str) -> String {
    size.to_string()
}
pub fn hex_compared(n: usize) -> String {
    format!("比較済み: {n}")
}
pub fn hex_truncated(size: &str) -> String {
    format!("上限({size})を超えるため、先頭分のみ表示しています")
}
pub const HEX_COL_OFFSET: &str = "オフセット";
pub const HEX_COL_LEFT: &str = "左 (16進)";
pub const HEX_COL_RIGHT: &str = "右 (16進)";
pub const HEX_COL_LASCII: &str = "左 ASCII";
pub const HEX_COL_RASCII: &str = "右 ASCII";
pub const HEX_COMPUTING: &str = "バイナリ差分を計算中…";
pub const HEX_UNKNOWN: &str = "不明";
pub const HEX_MISSING: &str = "··";

// 画像比較
pub const IMG_MODE_LABEL: &str = "表示";
pub const IMG_SIDE_BY_SIDE: &str = "左右";
pub const IMG_OVERLAY: &str = "重ね合わせ";
pub const IMG_BLINK: &str = "ブリンク";
pub const IMG_DIFFERENCE: &str = "差分";
pub const IMG_SLICE: &str = "スライス";
pub const IMG_OPACITY: &str = "不透明度";
pub const IMG_THRESHOLD: &str = "閾値";
pub const IMG_BOUNDARY: &str = "境界";
pub const IMG_ZOOM: &str = "ズーム";
pub const IMG_ZOOM_OUT: &str = "ズームアウト";
pub const IMG_ZOOM_IN: &str = "ズームイン";
pub const IMG_FIT: &str = "フィット表示に戻す";
pub const IMG_DIM: &str = "寸法";
pub const IMG_FILESIZE: &str = "ファイルサイズ";
pub const IMG_FORMAT: &str = "形式";
pub const IMG_LEFT: &str = "左";
pub const IMG_RIGHT: &str = "右";
pub const IMG_DIFF_ROW: &str = "差分";
pub const IMG_WARN_DOWNSAMPLED: &str = "大きな画像のため差分計算は縮小画像で実行";
pub const IMG_WARN_SIZE_MISMATCH: &str = "サイズ違い: 大きい方のキャンバスに正規化";
pub const IMG_COMPUTING: &str = "画像差分を計算中…";
pub const IMG_DIFF_HIGHLIGHT: &str = "差分ハイライト";
pub fn img_load_error(name: &str) -> String {
    format!("画像を読み込めません: {name}")
}
pub const IMG_CONVERT_ERROR: &str = "画像のビットマップ変換に失敗しました";

// コピー計画
pub const PLAN_TITLE: &str = "コピー計画プレビュー";
pub const PLAN_DIRECTION: &str = "方向";
pub const DIR_LTR: &str = "左 → 右";
pub const DIR_RTL: &str = "右 → 左";
pub const CLOSE_TOOLTIP: &str = "閉じる (Ctrl+W)";
pub fn plan_copy(n: usize) -> String {
    format!("コピー {n}")
}
pub fn plan_dirs(n: usize) -> String {
    format!("ディレクトリ {n}")
}
pub fn plan_diffs(n: usize) -> String {
    format!("差分 {n}")
}
pub fn plan_kept(n: usize) -> String {
    format!("保持 {n}")
}
pub fn plan_errors(n: usize) -> String {
    format!("エラー {n}")
}
pub const PLAN_COL_STATUS: &str = "状態";
pub const PLAN_COL_PATH: &str = "相対パス";
pub const PLAN_COL_SOURCE: &str = "コピー元";
pub const PLAN_COL_SIZE: &str = "サイズ";
pub const PLAN_COL_NOTE: &str = "備考";
pub const PLAN_EMPTY: &str = "コピーする項目はありません";

// 3-way
pub const TW_TITLE: &str = "3-way差分（読み取り専用）";
/// エディションが 3-way ビューを差し替える(マージ可能な)場合のウィンドウタイトル
pub const TW_MERGE_TITLE: &str = "3-wayマージ";
pub const TW_BASE: &str = "ベース";
pub const TW_LEFT: &str = "左";
pub const TW_RIGHT: &str = "右";
pub const SELECT_BASE_FILE: &str = "共通祖先(ベース)のファイルを選択してください";
pub const SELECT_LEFT_FILE: &str = "左側のファイルを選択してください";
pub const SELECT_RIGHT_FILE: &str = "右側のファイルを選択してください";
pub fn tw_truncated(max: usize) -> String {
    format!("上限(3側合計{max}行)を超えたため表示を中断しました")
}
pub fn tw_left_changed(n: usize) -> String {
    format!("左のみ変更 {n}")
}
pub fn tw_right_changed(n: usize) -> String {
    format!("右のみ変更 {n}")
}
pub fn tw_both_same(n: usize) -> String {
    format!("両者同一変更 {n}")
}
pub fn tw_conflicts(n: usize) -> String {
    format!("競合 {n}")
}
pub const TW_SAME: &str = "同一";
pub fn tw_lines(n: usize) -> String {
    format!("行 {n}")
}
pub const TW_COL_STATUS: &str = "状態";
pub const TW_COL_BASE: &str = "ベース";
pub const TW_COL_LEFT: &str = "左";
pub const TW_COL_RIGHT: &str = "右";
pub const TW_PLACEHOLDER: &str = "ベース・左・右の3つのファイルを選択してください";
pub const TW_COMPUTING: &str = "3-way差分を計算中…";
pub fn tw_not_text(label: &str, name: &str) -> String {
    format!("{label}「{name}」はテキストとして読み込めません")
}
pub fn tw_too_large(label: &str) -> String {
    format!("{label}ファイルが大きすぎて読み込めません")
}

// メニュー
pub const MENU_REPORT: &str = "レポート";
pub const REPORT_HTML: &str = "HTMLを保存…";
pub const REPORT_CSV: &str = "CSVを保存…";
pub const REPORT_JSON: &str = "JSONを保存…";
pub const REPORT_MD: &str = "Markdownを保存…";
pub const MENU_PLAN: &str = "コピー計画";
pub const PLAN_LTR: &str = "左 → 右の計画を表示…";
pub const PLAN_RTL: &str = "右 → 左の計画を表示…";
pub const MENU_3WAY: &str = "3-way差分";
pub const MENU_3WAY_ITEM: &str = "3つのファイルを比較…";
pub const SAVE_REPORT: &str = "レポートを保存";
pub fn report_default_name(ext: &str) -> String {
    // UTC の DiffLoupe-report-YYYYMMDD-HHMMSS.<ext>
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ts = diffloupe_core::report::iso8601(secs); // "YYYY-MM-DDTHH:MM:SSZ"
    let digits: String = ts.chars().filter(|c| c.is_ascii_digit()).collect();
    format!(
        "DiffLoupe-report-{}-{}.{}",
        &digits[..8],
        &digits[8..14],
        ext
    )
}
pub const OVERWRITE_TITLE: &str = "上書きしますか？";
pub fn overwrite_message(name: &str) -> String {
    format!("「{name}」は既に存在します。置き換えますか？")
}
pub const OVERWRITE: &str = "上書き";

// 設定画面
pub const SETTINGS_TITLE: &str = "設定";
pub const SEC_COMPARE: &str = "比較";
pub const SET_RECURSIVE: &str = "サブフォルダを含める";
pub const SET_DEFAULT_EXCLUDES: &str = "既定の除外(.git / .DS_Store / node_modules)を適用";
pub const SEC_FILTER: &str = "フィルタ";
pub const SET_USER_EXCLUDE: &str = "除外パターン(1行1glob)";
pub const SET_USER_INCLUDE: &str = "対象パターン(1行1glob)";
pub const SET_PRESETS: &str = "フィルタプリセット";
pub const SEC_CONTENT: &str = "内容比較";
pub const SET_IGNORE_EOL: &str = "改行コードの違いを無視";
pub const SET_IGNORE_TRAIL_WS: &str = "行末の空白を無視";
pub const SET_NORMALIZE_WS: &str = "連続する空白を正規化";
pub const SET_IGNORE_CASE: &str = "大文字と小文字を無視";
pub const SET_STRUCTURED: &str = "構造化比較";
pub const STRUCT_DISABLED: &str = "無効";
pub const STRUCT_AUTOMATIC: &str = "自動(拡張子)";
pub const SEC_RENAME: &str = "リネーム検出";
pub const SET_DETECT_RENAMES: &str = "リネーム/移動の候補を検出する";
pub const SEC_WATCH: &str = "自動再比較";
pub const SET_AUTO_REFRESH: &str = "フォルダの変更を監視して自動で再比較する";
pub const SEC_EDITION: &str = "エディション";
pub fn edition_limit_line(limit: usize) -> String {
    format!("フォルダ比較は{limit}件まで(Pro で制限解除)")
}
pub const EDITION_UNLIMITED: &str = "フォルダ比較の件数制限なし";

// about
pub fn about_comments(edition: &str) -> String {
    format!("フォルダ・ファイル比較ツール ({edition} エディション)")
}
