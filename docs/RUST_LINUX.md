# DiffLoupe Linux 版(GTK4)設計メモ

決定日: 2026-10-20。Linux 版のネイティブ GUI を GTK4(`gtk4-rs`)で実装する。
比較エンジンは `diffloupe-core` を Rust から直接呼ぶ(FFI なし)。

## 構成

```
apps/
├─ edition/              クレート diffloupe-edition
│   └─ src/lib.rs        Edition トレイト + FreeEdition(ツールキット非依存)
└─ linux/                独立 Cargo ワークスペース + クレート diffloupe-gtk
    ├─ Cargo.toml        gtk4 / diffloupe-core / diffloupe-edition / serde_json
    ├─ Dockerfile.dev    Ubuntu 24.04 + libgtk-4-dev + rustup の開発・検証イメージ
    ├─ scripts/check.sh  fmt/clippy(-D warnings)/test/release build/smoke をコンテナ内実行
    ├─ scripts/smoke_in_container.sh  Xvfb 上のスモークテスト
    ├─ scripts/package.sh             tarball(+AppImage があれば)生成
    ├─ data/com.furin-lab.diffloupe.desktop
    ├─ data/icons/       SwiftUI/DiffLoupe/AppIcon.icns から変換した PNG
    ├─ screenshots/      スモークテストのスナップショット出力先
    └─ src/              以下のモジュール構成
        ├─ main.rs       エントリ。--smoke-test / --compare-time を受理
        ├─ model.rs      AppModel 相当。比較はワーカースレッド→MainContext 通知
        ├─ window.rs     メインウィンドウ(ツールバー/フォルダバー/メニュー/ステータスバー)
        ├─ tree.rs       結果ツリー(TreeListModel + TreeExpander + ListView)
        ├─ detail.rs     詳細ペイン(メタデータヘッダ + ビュー切替)
        ├─ textdiff.rs   テキスト差分(2ペイン TextView、タグで行色/インライン/折りたたみ)
        ├─ hexview.rs    Hex 差分(ListView + Pango markup)
        ├─ imageops.rs   画像デコード(gdk-pixbuf)と差分計算(ImageDiffEngine 移植)
        ├─ imageview.rs  画像比較(5モード・ズーム/パン/閾値、DrawingArea+cairo)
        ├─ syncplan.rs   コピー計画プレビュー(読み取り専用)
        ├─ threeway.rs   3-way 差分(読み取り専用)
        ├─ settingsui.rs 設定ウィンドウ
        ├─ settings.rs   設定の永続化(GLib KeyFile → $XDG_CONFIG_HOME/diffloupe/settings.ini)
        ├─ recent.rs     比較履歴($XDG_CONFIG_HOME/diffloupe/history.json、最大10件)
        ├─ edition_ext.rs GTK 固有のエディション拡張トレイト
        ├─ smoke.rs      ヘッドレススモークテスト・PNG スナップショット
        ├─ strings.rs    日本語 UI 文字列(SwiftUI 版の文言を正典とする)
        └─ css.rs        バッジ/差分色の CSS
```

## エディション拡張の設計

Swift 版の `DiffLoupeEdition` プロトコルは `AnyView` を返すため、そのままでは
ツールキット非依存にできない。そこで2層に分ける。

- **`diffloupe-edition`(`apps/edition`)**: UI 非依存の共通部。
  `DiffLoupeEdition` トレイトは `name` / `max_folder_entries` /
  `capabilities()`(拡張ビュー提供有無のフラグ)/
  `has_unsaved_changes` / `confirm_discard_unsaved_changes` を持つ。
  `FreeEdition` は `max_folder_entries = Some(2000)` で拡張なし。
- **GTK 固有部(`apps/linux/src/edition_ext.rs`)**: `GtkEditionExtension` トレイトが
  `text_detail_view` / `sync_plan_actions` / `three_way_view` / `settings_section` の
  4つの拡張ウィジェットを `Option<gtk4::Widget>` で返す。既定実装はすべて `None`
  (=Free の従来ビュー)。

Pro 版は `DiffLoupeEdition` で能力を宣言し、各 OS アプリ側のツールキット用
トレイト(GTK では `GtkEditionExtension`)で実ビューを供給する。
Windows 版はこの `apps/edition` クレートをそのまま共有できる
(GTK 依存は `diffloupe-gtk` 側に隔離済み)。

## UI 実装の方針

- **plain GTK4**(libadwaita なし)。配布サイズを抑えるため。
- **結果ツリー**: `TreeListModel` + `TreeExpander` + `ListView`。
  行データは `glib::BoxedAnyObject` に Rust 構造体を包んで保持(GObject 派生不要)。
- **テキスト差分**: 左右 `TextView` を1つの `ScrolledWindow` 内の `HBox` に並べ、
  両バッファで同じ行数を維持(片側のみの行は空行)。行全体背景は
  `paragraph-background` タグ、インライン差分と検索ヒットは `background` タグ。
  コンテキスト折りたたみは「N 行を表示」行を `event` 付きタグでクリック展開。
- **画像比較**: gdk-pixbuf でデコード → α乗算済み RGBA → cairo `ImageSurface` を
  `DrawingArea` で描画。差分計算(Manhattan 距離・閾値・16Mpx 縮小・赤ブレンド)は
  Swift `ImageDiffEngine` を `imageops.rs` へ移植。
- **設定/履歴の永続化**: OS ネイティブな場所 = `$XDG_CONFIG_HOME/diffloupe/`。
  設定は `settings.ini`(GLib KeyFile)、履歴は `history.json`。
  設定キー名は Swift 版 AppModel の UserDefaults キーと同一に揃える。
- **ウィンドウ状態**: サイズと最大化は保存して復元。位置は Wayland で取得不可の
  ため保存対象外(X11 でのみ有効な値を保持)。

## スモークテスト

`diffloupe-gtk --smoke-test <left> <right> [outdir]` が
比較→テキスト差分選択→Hex→画像→設定の順でウィンドウを PNG スナップショットして
終了 0 で返す。コンテナ内では `scripts/smoke_in_container.sh` が Xvfb 上で実行し、
`apps/linux/screenshots/` に保存する。

## 残課題

- ウィンドウ位置の永続化は X11 限定(Wayland は制約)。
- 画像の EXIF 回転は gdk-pixbuf ローダー依存(Swift 版は CIImage で常時適用)。
- HEIC/HEIF は gdk-pixbuf ローダーが無い環境では読めない。
