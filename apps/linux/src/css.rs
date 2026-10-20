//! アプリ固有の CSS。差分色は SwiftUI 版の配色(追加=緑系/削除=赤系/変更=黄系)に揃え、
//! ライト/ダーク両テーマで視認できるよう透明度つきの色を使う。

pub const APP_CSS: &str = r"
/* ステータスバッジ */
.dl-badge {
    border-radius: 4px;
    padding: 0 6px;
    font-size: 0.82em;
}
.dl-badge-identical { background: alpha(@success_color, 0.18); color: @success_color; }
.dl-badge-different { background: alpha(@warning_color, 0.22); color: @warning_color; }
.dl-badge-leftonly  { background: alpha(@accent_color, 0.18); color: @accent_color; }
.dl-badge-rightonly { background: alpha(@purple2, 0.22); color: @purple2; }
.dl-badge-error     { background: alpha(@error_color, 0.22); color: @error_color; }

/* テキスト差分(行背景は TextTag 側で付与。ここはフォントのみ) */
.dl-mono { font-family: Monospace; }
.dl-diff-toolbar { padding: 4px; border-bottom: 1px solid alpha(currentColor, 0.12); }
.dl-meta { padding: 6px 8px; border-bottom: 1px solid alpha(currentColor, 0.12); }
.dl-fold { color: @accent_color; }

/* 画像比較ツールバー */
.dl-img-toolbar { padding: 4px; border-bottom: 1px solid alpha(currentColor, 0.12); }

/* ステータスバー */
.dl-statusbar { padding: 3px 8px; border-top: 1px solid alpha(currentColor, 0.12); }
.dl-statusbar label { margin-right: 10px; }

/* 空状態 */
.dl-empty { color: alpha(currentColor, 0.55); }
";
