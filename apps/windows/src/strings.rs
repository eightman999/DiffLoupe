//! 日本語 UI 文字列(SwiftUI 版の表示文言に合わせる)

// メイン画面
pub const TITLE: &str = "DiffLoupe";
pub const LEFT_FOLDER: &str = "左フォルダ…";
pub const RIGHT_FOLDER: &str = "右フォルダ…";
pub const PICK_LEFT: &str = "左側のフォルダを選択してください";
pub const PICK_RIGHT: &str = "右側のフォルダを選択してください";
pub const UNSELECTED: &str = "未選択";
pub const SWAP: &str = "⇄ 交換";
pub const SWAP_TIP: &str = "左右を交換 (Ctrl+Shift+S)";
pub const COMPARE: &str = "比較";
pub const CANCEL: &str = "キャンセル";
pub const HISTORY: &str = "履歴";
pub const NO_HISTORY: &str = "履歴はまだありません";
pub const HISTORY_TIP: &str = "比較履歴から復元";
pub const THREE_WAY: &str = "3-way差分…";
pub const THREE_WAY_TIP: &str = "3つのファイルを3-way差分で比較します";
pub const SETTINGS: &str = "設定…";
pub const SEARCH_CUE: &str = "相対パスを検索";
pub const FILTER_TIP: &str = "表示する状態を選択";
pub const COPY_PLAN: &str = "コピー計画…";
pub const SAVE_REPORT: &str = "レポート保存…";

// ステータスフィルタ(ResultFilter.displayName 準拠)
pub const FILTER_ALL: &str = "すべて";
pub const FILTER_DIFFERENCES: &str = "差分すべて";
pub const FILTER_DIFFERENT: &str = "相違";
pub const FILTER_LEFT_ONLY: &str = "左のみ";
pub const FILTER_RIGHT_ONLY: &str = "右のみ";
pub const FILTER_ERROR: &str = "エラー";
pub const FILTER_IDENTICAL: &str = "一致";

// ステータスバッジ(StatusBadge.label 準拠)
pub const BADGE_IDENTICAL: &str = "一致";
pub const BADGE_DIFFERENT: &str = "相違";
pub const BADGE_DIR_HAS_DIFF: &str = "差分あり";
pub const BADGE_LEFT_ONLY: &str = "左のみ";
pub const BADGE_RIGHT_ONLY: &str = "右のみ";
pub const BADGE_ERROR: &str = "エラー";

// ステータスバー
pub const STATUS_PROMPT: &str = "左右のフォルダを選択して「比較」を押してください";
pub const STATUS_SCANNING: &str = "走査中…";
pub const STATUS_COMPARING: &str = "内容比較中…";
pub const STATUS_RENAMING: &str = "リネーム/移動を検出中…";
pub const STATUS_WATCHING: &str = "自動再比較 監視中";
pub const STATUS_PAUSED: &str = "自動再比較 一時停止";
pub const WATCHING_TIP: &str =
    "フォルダの変更を監視しています。変更を検知すると自動で再比較します。";
pub const ITEMS_UNIT: &str = "件";
pub const TOTAL_FILES: &str = "計";
pub const FILES_UNIT: &str = "ファイル";

// 結果ツリー空表示(DiffTreeView.emptyMessage 準拠)
pub const EMPTY_RESULT: &str = "比較結果がここに表示されます";
pub const EMPTY_NO_DIFF: &str = "差分はありません";
pub const EMPTY_FILTERED: &str = "絞り込み条件に一致する項目はありません";
pub const EMPTY_NO_FILES: &str = "対象ファイルがありません";

// コンテキストメニュー
pub const MENU_LEFT: &str = "左側";
pub const MENU_RIGHT: &str = "右側";
pub const MENU_OPEN: &str = "開く";
pub const MENU_REVEAL: &str = "エクスプローラーに表示";
pub const MENU_COPY_PATH: &str = "パスをコピー";

// 詳細ビュー
pub const DETAIL_PLACEHOLDER: &str = "ツリーからファイルを選択してください";
pub const DETAIL_LOADING: &str = "差分を計算中…";
pub const DETAIL_IS_DIR: &str = "フォルダです。配下のファイルを選択してください";
pub const DETAIL_LEFT_ONLY: &str = "左フォルダにのみ存在します";
pub const DETAIL_RIGHT_ONLY: &str = "右フォルダにのみ存在します";
pub const DETAIL_NO_TARGET: &str = "比較対象がありません";
pub const DETAIL_IDENTICAL: &str = "内容は一致しています";
pub const DETAIL_TOO_LARGE: &str = "ファイルが大きすぎるため差分表示できません(上限32MiB)";
pub const DETAIL_TOO_MANY_LINES: &str = "行数が多すぎるため差分計算を省略しました(上限 計40000行)";
pub const META_SIZE: &str = "サイズ";
pub const META_MTIME: &str = "変更日時";
pub const META_PERM: &str = "パーミッション";
pub const RENAME_UNIQUE: &str = "リネーム/移動の候補(確定)";
pub const RENAME_CANDIDATE: &str = "リネーム/移動の候補";
pub const FOLD_SHOW_LINES: &str = "行を表示";

// 詳細ツールバー
pub const PREV_DIFF: &str = "前の差分";
pub const NEXT_DIFF: &str = "次の差分";
pub const DIFF_SEARCH_CUE: &str = "検索";
pub const PREV_HIT: &str = "前の一致";
pub const NEXT_HIT: &str = "次の一致";
pub const CASE_SENSITIVE_TIP: &str = "大文字と小文字を区別する";
pub const SHOW_ALL_ROWS: &str = "全行表示";
pub const SHOW_ALL_TIP: &str = "コンテキスト行の折りたたみを無効にする";

// Hex ビュー
pub const HEX_OFFSET: &str = "オフセット";
pub const HEX_LEFT: &str = "左 (Hex)";
pub const HEX_LEFT_ASCII: &str = "左 (ASCII)";
pub const HEX_RIGHT: &str = "右 (Hex)";
pub const HEX_RIGHT_ASCII: &str = "右 (ASCII)";
pub const HEX_TRUNCATED: &str = "サイズ上限(8 MiB)を超えているため、先頭部分のみ表示しています";

// 画像比較
pub const IMG_LOADING: &str = "画像差分を計算中…";
pub const IMG_SIDE_BY_SIDE: &str = "左右";
pub const IMG_OVERLAY: &str = "重ね合わせ";
pub const IMG_BLINK: &str = "ブリンク";
pub const IMG_DIFF: &str = "差分";
pub const IMG_SLICE: &str = "スライス";
pub const IMG_OPACITY: &str = "不透明度";
pub const IMG_THRESHOLD: &str = "閾値";
pub const IMG_BOUNDARY: &str = "境界";
pub const IMG_ZOOM: &str = "ズーム";
pub const IMG_ZOOM_OUT_TIP: &str = "ズームアウト";
pub const IMG_ZOOM_IN_TIP: &str = "ズームイン";
pub const IMG_FIT_TIP: &str = "フィット表示に戻す";
pub const IMG_LEFT: &str = "左";
pub const IMG_RIGHT: &str = "右";
pub const IMG_DIM: &str = "寸法";
pub const IMG_FILESIZE: &str = "ファイルサイズ";
pub const IMG_FORMAT: &str = "形式";
pub const IMG_DIFFSTAT: &str = "差分";
pub const IMG_UNKNOWN: &str = "不明";
pub const IMG_DOWNSAMPLED: &str = "大きな画像のため差分計算は縮小画像で実行";
pub const IMG_DIM_DIFFERS: &str = "サイズ違い: 大きい方のキャンバスに正規化";
pub const IMG_UNREADABLE_PREFIX: &str = "画像を読み込めません: ";

// 設定画面
pub const SETTINGS_TITLE: &str = "設定";
pub const SEC_EDITION: &str = "エディション";
pub const CURRENT_EDITION: &str = "現在のエディション";
pub const SEC_COMPARE: &str = "比較";
pub const OPT_RECURSIVE: &str = "サブフォルダを含める";
pub const OPT_RECURSIVE_NOTE: &str =
    "無効にすると、選択した左右フォルダの直下だけを比較します。次回の比較から反映されます。";
pub const OPT_DEFAULT_EXCLUDES: &str = "既定の除外パターンを有効にする";
pub const OPT_RENAMES: &str = "リネーム/移動候補を検出する";
pub const OPT_RENAMES_NOTE: &str = "左右の片側のみにあるファイル同士を内容ハッシュで照合し、リネーム/移動の候補を表示します。有効にすると追加のハッシュ計算が入るため比較時間が長くなることがあります。";
pub const SEC_NORMALIZE: &str = "内容比較の正規化";
pub const OPT_IGNORE_LINE_ENDINGS: &str = "改行コードの違いを無視する";
pub const OPT_IGNORE_TRAILING_WS: &str = "行末の空白を無視する";
pub const OPT_NORMALIZE_WS: &str = "連続する空白を1つに正規化する";
pub const OPT_IGNORE_CASE: &str = "大文字と小文字の違いを無視する";
pub const OPT_STRUCTURED: &str = "構造化比較";
pub const NORMALIZE_NOTE: &str = "すべてオフの場合はバイト単位の厳密比較になります(既定)。正規化設定はフォルダ比較と詳細差分ビューの両方に反映されます。";
pub const STRUCTURED_NOTE: &str = "構造化比較はJSON・XML・CSV/TSVの表記揺れ(キー順・属性順・引用・区切りなど)を正規化して比較します。パースできないファイルは元のテキストで比較します。";
pub const SEC_EXCLUDE: &str = "ユーザー除外パターン";
pub const SEC_INCLUDE: &str = "ユーザーインクルードパターン";
pub const PATTERN_HINT: &str =
    "1行に1パターン。* はセパレータをまたがない任意列、** はセパレータをまたぐ任意列、? は任意の1文字です。";
pub const INCLUDE_NOTE: &str = "非空のとき、ファイル名または相対パスが一致するファイルだけを比較対象に含めます。空のときは従来どおり全ファイルが対象です(除外は常に優先されます)。";
pub const VALID_PATTERNS: &str = "有効なパターン: ";
pub const NONE_TEXT: &str = "なし";
pub const SEC_AUTO_REFRESH: &str = "自動再比較";
pub const OPT_AUTO_REFRESH: &str = "フォルダ変更時に自動再比較";
pub const AUTO_REFRESH_NOTE: &str = "左右のフォルダのファイル追加・削除・サイズ/更新時刻の変更を監視し、変更を検知すると自動で再比較します。監視は既定で1秒間隔です。比較中に変更を検知した場合は、その比較が終わった後に一度だけ再比較します。既定では無効です。";
pub const SEC_PRESETS: &str = "フィルタプリセット (opt-in)";
pub const PRESETS_NOTE: &str =
    "選択したプリセットの除外パターンが次回の比較から追加されます。既定ではすべて無効です。";
pub const CLOSE: &str = "閉じる";

// エディション関連
pub const ENTRY_LIMIT_TITLE: &str = "Free版の比較上限に達しました";
pub const OPEN_SETTINGS: &str = "設定を開く";
pub const EDITION_LIMIT_NOTE: &str =
    "版はフォルダ比較が2000ファイルまでに制限されます。上限なしで比較できるのは有償のPro版です。";

// git ref
pub const WORKING_TREE: &str = "作業ツリー";
pub const CUSTOM_REF: &str = "任意のrefを入力…";
pub const REF_INPUT_TITLE: &str = "ref名を入力";
pub const REF_INPUT_CUE: &str = "例: origin/main, v1.0.0, コミットハッシュ";
pub const APPLY: &str = "適用";

// コピー計画
pub const SYNC_TITLE: &str = "コピー計画プレビュー";
pub const SYNC_DIRECTION: &str = "方向";
pub const SYNC_EMPTY: &str = "コピー計画を表示するには「コピー計画」メニューから生成してください";
pub const SYNC_LEFT_TO_RIGHT: &str = "左 → 右";
pub const SYNC_RIGHT_TO_LEFT: &str = "右 → 左";
pub const SYNC_COL_ACTION: &str = "状態";
pub const SYNC_COL_PATH: &str = "相対パス";
pub const SYNC_COL_SOURCE: &str = "コピー元";
pub const SYNC_COL_SIZE: &str = "サイズ";
pub const SYNC_COL_REASON: &str = "備考";
pub const SYNC_COPY: &str = "コピー";
pub const SYNC_MKDIR: &str = "ディレクトリ作成";
pub const SYNC_CONFLICT: &str = "差分(上書き候補)";
pub const SYNC_PRESERVED: &str = "保持";
pub const SYNC_BLOCKED: &str = "エラー";
pub const SYNC_SUMMARY_COPY: &str = "コピー";
pub const SYNC_SUMMARY_DIR: &str = "ディレクトリ";
pub const SYNC_SUMMARY_CONFLICT: &str = "差分";
pub const SYNC_SUMMARY_KEEP: &str = "保持";
pub const SYNC_SUMMARY_ERROR: &str = "エラー";

// 3-way
pub const THREE_WAY_TITLE: &str = "3-way差分（読み取り専用）";
/// エディションが編集可能な 3-way ビューを供給したときのタイトル
pub const THREE_WAY_MERGE_TITLE: &str = "3-wayマージ";
pub const PICK_BASE: &str = "共通祖先(ベース)のファイルを選択してください";
pub const PICK_LEFT_FILE: &str = "左側のファイルを選択してください";
pub const PICK_RIGHT_FILE: &str = "右側のファイルを選択してください";
pub const BASE_FILE: &str = "ベース…";
pub const LEFT_FILE: &str = "左ファイル…";
pub const RIGHT_FILE: &str = "右ファイル…";
pub const COMPARE_THREE: &str = "比較";
pub const COL_BASE: &str = "ベース";
pub const COL_LEFT: &str = "左";
pub const COL_RIGHT: &str = "右";
pub const TW_CONFLICTS: &str = "競合";
pub const TW_TRUNCATED: &str = "行数が多すぎるため3-way差分を計算できません(上限 計40000行)";
pub const TW_CHANGES: &str = "変更";
pub const TW_UNCHANGED: &str = "変更なし";

// レポート
pub const REPORT_SAVE_TITLE: &str = "レポートを保存";
pub const OVERWRITE_TITLE: &str = "上書きしますか？";
pub const OVERWRITE_SUFFIX: &str = "」は既に存在します。置き換えますか？";
pub const OVERWRITE: &str = "上書き";
pub const REPORT_SAVE_FAILED: &str = "レポート保存に失敗しました: ";
pub const REPORT_HTML: &str = "HTMLを保存…";
pub const REPORT_CSV: &str = "CSVを保存…";
pub const REPORT_JSON: &str = "JSONを保存…";
pub const REPORT_MD: &str = "Markdownを保存…";

// メニュー
pub const MENU_FILE: &str = "ファイル(&F)";
pub const MENU_COMPARE: &str = "比較(&R)\tCtrl+R";
pub const MENU_SWAP: &str = "左右を交換(&S)\tCtrl+Shift+S";
pub const MENU_SETTINGS: &str = "設定(&O)…";
pub const MENU_EXIT: &str = "終了(&X)";
pub const MENU_REPORT: &str = "レポート(&R)";
pub const MENU_SYNC_PLAN: &str = "コピー計画(&C)";
pub const MENU_SYNC_L2R: &str = "左 → 右の計画を表示…";
pub const MENU_SYNC_R2L: &str = "右 → 左の計画を表示…";
pub const MENU_THREEWAY: &str = "3-way差分(&3)";
pub const MENU_THREEWAY_OPEN: &str = "3つのファイルを比較…";
pub const MENU_HELP: &str = "ヘルプ(&H)";
pub const MENU_ABOUT: &str = "DiffLoupe について…";

// エラー
pub const COMPARE_FAILED: &str = "比較に失敗しました: ";
pub const ABOUT_TEXT: &str =
    "DiffLoupe 1.1.0 (Free)\nフォルダ・ファイル比較ツール\n\n(c) eightman 2005-2026\nFurin-lab All rights reserved.";
