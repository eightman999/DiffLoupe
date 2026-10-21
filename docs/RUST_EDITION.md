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
apps/linux/src/lib.rs           GTK ライブラリの入口: AppConfig + run()(Free の bin も Pro もこれを呼ぶ)
apps/windows/                   Win32 固有: 現状は拡張ビューなし (下記)
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

## 責務の分け方

- **共通 (この crate)**: 「何を提供するか」の宣言と、UI に依存しないフック。実ビューは持たない。
- **ツールキット固有 (各アプリ)**: 「どう描くか」。GTK は `GtkEditionExtension`
  (4 つの拡張点それぞれが `Option<Widget>`、`None` で従来ビュー)。
- UI 層は `edition.provides(point)` / `capabilities()` が true のときだけ、ツールキット側の拡張オブジェクトからビューを取得する。

## GTK 版: ライブラリ化と `AppConfig`(Pro 用)

`apps/linux`(クレート `diffloupe-gtk`)は lib + bin 構成。Free の bin(`src/main.rs`)は
`diffloupe_gtk::run(AppConfig::free())` を呼ぶだけで、起動処理・ウィンドウ・各ビューは lib にある。
Pro(別 repo のクレート `diffloupe-pro-gtk`)はこの lib を path 依存で取り込み、`AppConfig` を差し替えて `run` を呼ぶ。

| `AppConfig` のフィールド | 内容 | Free の値 |
|---|---|---|
| `app_id` | GApplication ID。別にすれば同時インストール・同時起動できる | `com.furin-lab.diffloupe` |
| `app_name` | ウィンドウタイトルの表示名 | `DiffLoupe` |
| `config_dir_name` | 設定・履歴の保存先 `$XDG_CONFIG_HOME/<名前>/`(`settings::config_dir()` で Pro も参照できる) | `diffloupe` |
| `edition` | `Box<dyn Edition>`(名前・件数上限・未保存フック・能力フラグ) | `FreeEdition` |
| `extension` | `Box<dyn GtkEditionExtension>`(ビュー差し替え・メニュー/アクション追加) | `FreeGtkEdition` |
| `on_ready` | ウィンドウ表示後に 1 回呼ばれるフック(スモークテストの駆動など) | なし(`--smoke-test` 時のみ) |
| `extra_css` | 既定 CSS の後に適用する追加 CSS | なし |

`MainWindow`(`window` / `model` / `tree` / `detail` / `ext` と `apply_folder` / `start_compare` /
`recompare` / `on_tree_select` / `refresh_folder_ui`)と `AppModel` は pub。拡張側はこれらを使って
ダイアログの親・再比較・選択の操作ができる。`AppModel::main_window()` で親ウィンドウも取れる。

### `GtkEditionExtension` の拡張点

既存の 4 つは従来どおり(`None` で従来ビュー)。追加のみで、既存メソッドのシグネチャは変えていない。

| メソッド | いつ・何のために呼ばれるか |
|---|---|
| `text_detail_view(model, entry)` | テキスト詳細。`provides(TextDetailView)` のとき、ファイルを選ぶたびに呼ばれる。返したウィジェットは次の選択で破棄される(従来は 1 度しか差し替わらない不具合があった) |
| `sync_plan_actions(model, plan)` | コピー計画シートの操作領域。方向を切り替えるたびに作り直される |
| `three_way_view(model)` | 3-way ビュー全体(タイトルは「3-wayマージ」の別ウィンドウ) |
| `settings_section(model)` | 設定画面の末尾に追加するセクション |
| `window_ready(window)` **(追加)** | メインウィンドウ構築直後(標準アクション登録の後)。アクション/アクセラレータの登録、同名アクションの上書き、`Rc<MainWindow>` の弱参照の保持 |
| `header_widgets(model)` **(追加)** | ヘッダバー左側(履歴・左右交換の右)へ足すウィジェット(「プロジェクト」メニューなど) |
| `comparison_changed(model)` **(追加)** | 比較結果が変わった(開始・完了・破棄)とき。メニュー項目の有効/無効の更新用 |

### 未保存確認の経路

`Edition::confirm_discard_unsaved_changes()` は次のすべての操作の直前に呼ばれる(`false` で中止)。
Free は常に `true` なので挙動は変わらない。

| 操作 | 呼び出し元 |
|---|---|
| 比較(ボタン・Ctrl+R) | `AppModel::begin_compare(user_initiated = true)` |
| フォルダ変更(選択・D&D) | `AppModel::set_folder` |
| 左右交換 | `AppModel::swap_sides` |
| 履歴復元 | `MainWindow::restore_history` |
| ウィンドウを閉じる | `MainWindow` の `close-request`(**追加**) |
| 終了(Ctrl+Q) | `win.quit` アクション → ウィンドウを閉じる(**追加**) |

`has_unsaved_changes()` は自動再比較の保留判定に使う。GTK のダイアログは非同期だが、この契約は同期なので、
Pro は確認ダイアログを「ネストしたメインループ」で待つ(`GMainContext::iteration`)。
「破棄して続行」は編集を保存済みの状態へ戻すので、1 つの操作の中で続けて呼ばれても二度目は確認しない。

## Pro が拡張する手順

1. Pro crate (非公開) が `diffloupe_edition::Edition` を実装し、`provides` で差し替える拡張点を返す。
   `name()` は `"Pro"`、`max_folder_entries()` は `None`。`Edition` は `Send + Sync` なので、
   GTK のオブジェクトを持つ状態(編集セッションなど)は thread_local に置く。
2. GTK: `GtkEditionExtension` を実装し、上の `AppConfig` に `Box` で渡して `run` を呼ぶ
   (参考実装: 非公開 repo の `apps/linux/`)。
3. Win32: 同様に Win32 固有の拡張トレイトを `apps/windows` に追加して渡す
   (現状 Win32 側の UI は `provides` を参照する箇所を持たず、`Edition` の
   名前・上限・未保存フックのみ使用している)。

### path 依存の置き場所(Pro 側の注意)

Pro の `rust/diffloupe-pro-core` は公開 core を兄弟ディレクトリ `DiffLoupe-rust-core` への path 依存で取り込む。
GTK ライブラリも同じ公開 repo を指すため、Pro 側は **同じ名前の兄弟ディレクトリ** にまとめる
(`../../../DiffLoupe-rust-core/apps/linux`)。cargo は別パスの `diffloupe-core` を別パッケージとして扱い、
型が一致せず lockfile も衝突するため。コンテナ/CI は公開 repo(または worktree)をこの名前で置く。

## 互換性の方針

- 拡張点の追加は `non_exhaustive` により破壊的変更にならない。`match` には `_` を置く。
- 既存メソッドのシグネチャは変えない。新しい能力は既定実装つきメソッドで足す。
- Linux 版の旧名 `DiffLoupeEdition`、Windows 版の旧名 `Edition` は両方使える。

## 統合の経緯

Linux 版と Windows 版が別々に `apps/edition` を作っていた (Linux: 能力フラグ構造体、
Windows: `ExtensionPoint` 列挙 + `provides`)。両方を含む上位互換として統合した。
`name()` は `&str` (Windows 版の `&'static str` 実装もそのまま使える)。
