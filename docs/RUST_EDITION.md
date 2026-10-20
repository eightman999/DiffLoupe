# エディション拡張(`apps/edition`)

Linux(GTK4)と Windows(Win32)のアプリが共有する、Free/Pro の差し込み口。
Swift 版 `DiffLoupeEdition` プロトコル(拡張点で `AnyView` を返す)に相当するが、
ツールキットに依存しない形にしてある。

## 構成

```
apps/edition/           crate diffloupe-edition (依存は diffloupe-core のみ)
  Edition               トレイト。別名 DiffLoupeEdition は同一トレイト(Swift 名に合わせた別名)
  ExtensionPoint        拡張点の列挙 (non_exhaustive, ExtensionPoint::ALL)
  EditionCapabilities   提供有無のスナップショット (provides() から導出)
  FreeEdition           公開版 (上限 2000 件、拡張なし)
apps/linux/src/edition_ext.rs   GTK 固有: GtkEditionExtension (Option<gtk4::Widget> を返す)
apps/windows/src/win/ext.rs     Win32 固有: Win32EditionExtension (Option<ExtView{hwnd}> を返す)
                        + AppConfig (起動構成: 表示名/エディション/拡張/設定保存先/スモーク差替)
apps/windows/src/extmodel.rs    拡張メニュー記述の UI 非依存モデル(ホストでテスト可)
```

## トレイトの契約

| メソッド | 必須 | 既定 | 用途 |
|---|---|---|---|
| `name()` | 必須 | - | 表示名 ("Free" / "Pro") |
| `max_folder_entries()` | 必須 | - | フォルダ比較の件数上限。`None` は無制限 |
| `provides(point)` | 任意 | `false` | その拡張点を差し替えるか |
| `capabilities()` | 任意 | `provides` から導出 | UI が一括で参照するフラグ |
| `has_unsaved_changes()` | 任意 | `false` | 自動再比較の保留判定 |
| `confirm_discard_unsaved_changes()` | 任意 | `true` | 破棄しうる操作の直前に呼ぶ。`false` で中止 |

`Send + Sync` を要求する (UI 層がワーカースレッドと共有するため)。

拡張点 (`ExtensionPoint`): `TextDetailView` / `SyncPlanActions` / `ThreeWayView` / `SettingsSection`。

## Win32 の起動構成 (`AppConfig` / `Win32EditionExtension`)

Windows 版は `apps/windows` がライブラリ(`diffloupe-win`)で、エントリは
`diffloupe_win::win::run(args, config)`。Free の `main.rs` は `AppConfig::free()`
を渡すだけ。Pro は `AppConfig` の各フィールドを差し替えて呼ぶ。

| `AppConfig` フィールド | 意味 |
|---|---|
| `display_name` | ウィンドウタイトル兼アプリ ID。設定先の既定は `%APPDATA%\<display_name>` |
| `about_text` | バージョン情報ダイアログの本文 |
| `edition` | `Arc<dyn Edition>`(名前・上限・`provides`・未保存確認) |
| `extension` | `Option<Arc<dyn Win32EditionExtension>>`(Win32 拡張ビュー供給元) |
| `settings_dir` | 設定保存先の明示指定(Free/Pro で別パスにする) |
| `smoke` | `--smoke-test` 実装の差し替え(Pro は自前の検証手順) |

`Win32EditionExtension` のメソッドはすべて UI スレッドで呼ばれ、`Send`/`Sync` は
要求しない。各ビュー生成メソッドは `ExtContext`(親 HWND・メイン HWND・
`&AppModel`・UI フォント・DPI)を受け、`Option<ExtView>` を返す。`None` なら
従来ビューを使う(Free と同じ見た目・挙動)。`ExtView` は拡張が作った子 HWND で、
配置・表示・破棄は UI 側が管理する。

| メソッド | 差し替え対象 |
|---|---|
| `text_detail_view(ctx)` | テキスト差分の詳細領域全体(従来は読み取り専用グリッド+ツールバー) |
| `sync_plan_actions(ctx)` | コピー計画シート下部の帯(高さは `ExtView.height`、方向変更で再生成) |
| `three_way_view(ctx)` | 3-way シートのコンテンツ全体(クライアント領域いっぱい) |
| `settings_section(ctx)` | 設定画面末尾の追加セクション(幅は画面幅、高さは `ExtView.height`) |
| `extra_menus()` | メニューバーの「ヘルプ」手前に挿入するトップレベルメニュー |
| `menu_command(id, main, model)` | 拡張メニューのコマンド処理(`true` で処理済み) |
| `menu_item_enabled(id, model)` | メニューを開く直前の有効/無効 |
| `build_submenu(submenu_id, model)` | 動的サブメニュー(履歴など)の中身を開く直前に再構築 |
| `init_menu_popup(menu, model)` | 拡張ポップアップを開く直前の任意調整 |

拡張コマンド ID は `extmodel::EXT_CMD_MIN..=EXT_CMD_MAX`(3000 番台)を使う。
組み込み ID(1xxx)と衝突しない。拡張がアプリ全体へ連絡するときはメイン
ウィンドウへ `WM_APP_EXT`(wParam: 1=選択を保持して再比較 / 2=保留中の
自動再比較を再開)をポストする。

## 未保存編集の確認(Win32)

`Edition::has_unsaved_changes` / `confirm_discard_unsaved_changes` は
`AppModel` 経由で次の操作に接続済み: 比較し直し(手動・自動再比較・
フィルタ/オプション変更・履歴復元の直後の再比較)、左右フォルダ変更
(ボタン・D&D)、左右交換、履歴復元、ウィンドウを閉じる(`WM_CLOSE`)、
終了(ファイル→終了 = `WM_CLOSE` 経由)。「破棄して続行」が選ばれたあとは
実際に編集を破棄する実装にすること(二重確認にならない)。

## 責務の分け方

- **共通 (この crate)**: 「何を提供するか」の宣言と、UI に依存しないフック。実ビューは持たない。
- **ツールキット固有 (各アプリ)**: 「どう描くか」。GTK は `GtkEditionExtension`
  (4 つの拡張点それぞれが `Option<Widget>`、`None` で従来ビュー)、
  Win32 は `Win32EditionExtension`(`Option<ExtView>`、同じく `None` で従来ビュー)。
- UI 層は `edition.provides(point)` / `capabilities()` が true のときだけ、ツールキット側の拡張オブジェクトからビューを取得する。

## Pro が拡張する手順

1. Pro crate (非公開) が `diffloupe_edition::Edition` を実装し、`provides` で差し替える拡張点を返す。
   `name()` は `"Pro"`、`max_folder_entries()` は `None`。
2. GTK: 同じ型 (または別の型) で `GtkEditionExtension` を実装し、`window.rs` が受け取る
   `Rc<dyn Edition>` / `Rc<dyn GtkEditionExtension>` に渡す。
3. Win32: `Win32EditionExtension` を実装し、拡張オブジェクトと `Edition` を
   `AppConfig` に載せて `diffloupe_win::win::run(args, config)` を呼ぶ。
   未保存確認は `Edition` 側、実ビューは `Win32EditionExtension` 側に置く。

## 互換性の方針

- 拡張点の追加は `non_exhaustive` により破壊的変更にならない。`match` には `_` を置く。
- 既存メソッドのシグネチャは変えない。新しい能力は既定実装つきメソッドで足す。
- Linux 版の旧名 `DiffLoupeEdition`、Windows 版の旧名 `Edition` は両方使える。

## 統合の経緯

Linux 版と Windows 版が別々に `apps/edition` を作っていた (Linux: 能力フラグ構造体、
Windows: `ExtensionPoint` 列挙 + `provides`)。両方を含む上位互換として統合した。
`name()` は `&str` (Windows 版の `&'static str` 実装もそのまま使える)。
