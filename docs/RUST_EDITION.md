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

## Pro が拡張する手順

1. Pro crate (非公開) が `diffloupe_edition::Edition` を実装し、`provides` で差し替える拡張点を返す。
   `name()` は `"Pro"`、`max_folder_entries()` は `None`。
2. GTK: 同じ型 (または別の型) で `GtkEditionExtension` を実装し、`window.rs` が受け取る
   `Rc<dyn Edition>` / `Rc<dyn GtkEditionExtension>` に渡す。
3. Win32: 同様に Win32 固有の拡張トレイトを `apps/windows` に追加して渡す
   (現状 Win32 側の UI は `provides` を参照する箇所を持たず、`Edition` の
   名前・上限・未保存フックのみ使用している)。

## 互換性の方針

- 拡張点の追加は `non_exhaustive` により破壊的変更にならない。`match` には `_` を置く。
- 既存メソッドのシグネチャは変えない。新しい能力は既定実装つきメソッドで足す。
- Linux 版の旧名 `DiffLoupeEdition`、Windows 版の旧名 `Edition` は両方使える。

## 統合の経緯

Linux 版と Windows 版が別々に `apps/edition` を作っていた (Linux: 能力フラグ構造体、
Windows: `ExtensionPoint` 列挙 + `provides`)。両方を含む上位互換として統合した。
`name()` は `&str` (Windows 版の `&'static str` 実装もそのまま使える)。
