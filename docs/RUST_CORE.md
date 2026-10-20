# DiffLoupe Rust Core 設計

決定日: 2026-10-20。Mac / Windows / Linux の全版で、比較エンジン(Core)を Rust の共通実装にする。
UI は各 OS のネイティブで作る。

| OS | UI | Core の呼び方 |
|---|---|---|
| macOS | 既存 SwiftUI(`SwiftUI/`) | UniFFI で生成した Swift バインディング(`diffloupe-ffi`) |
| Linux | GTK4(`gtk4-rs`) | Rust から直接呼ぶ |
| Windows | Win32(`windows` crate) | Rust から直接呼ぶ |

有償版(DiffLoupe Pro, 非公開repo)は、この公開 Core クレートに依存する `diffloupe-pro-core` を持つ。
同期の実行、マージ、編集、プロジェクト、レポートテンプレートはそちらに置く。

## 正典

- 振る舞いの正典は `SwiftUI/SPEC.md` と、Swift 版のテスト(`SwiftUI/DiffLoupeTests`, `CoreTests`, `CLITests`)。
- 移行中は、Swift 版 CLI(`SwiftUI` で `swift build -c release` した `diffloupe`)が参照実装になる。
  Rust 版は同じ入力に対して同じ JSON レポートと同じ終了コードを返すこと(差分テスト)。
- 仕様と Swift 実装が食い違う箇所を見つけたら、勝手に決めずに `docs/RUST_PARITY_NOTES.md` に記録する。

## リポジトリ構成

```
core/                     Cargo workspace(公開, MIT)
├─ Cargo.toml
├─ crates/
│  ├─ diffloupe-core/     ライブラリ。UI・OS 依存なし
│  ├─ diffloupe-cli/      バイナリ `diffloupe`。Swift 版 CLI と互換
│  └─ diffloupe-ffi/      UniFFI(第2段階で作る)
├─ tests/parity/          Swift 版との差分テスト(スクリプトと期待値)
└─ benches/               速度比較
```

## 軽さの方針

- 依存クレートは最小限にとどめる。許可するのは次のものだけ。
  - `sha2`(asm 機能)
  - `encoding_rs`
  - `serde` / `serde_json`
  - `quick-xml`
  - `memchr`
- 上記以外を足す場合は、理由を `RUST_PARITY_NOTES.md` に書く。glob、Myers 差分、CSV 解析、並列化(std::thread)は自前で実装する。
- release プロファイルは `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`(ライブラリ利用時を除く)。
- 目標: CLI 単体バイナリで数 MB。

## 移植する機能(第1段階)

Swift の `Engine/` と `Models/` と `CLILib/` を移植する。ImageDiffEngine は第2段階で各 OS に分けて扱う。

- `FolderComparator`
  - 再帰走査、件数上限、キャンセル、進捗
  - 判定はサイズ → SHA-256。ハッシュは並列にする
  - 既定除外
- `FilterSet` / `ComparisonOptions`
  - include/exclude glob(`*` `?` `**`)とプリセット
  - 内容比較モード: 改行・行末空白・連続空白・大文字小文字の無視
- `TextFileLoader`
  - UTF-8 / BOM / UTF-16 / UTF-32 / Shift_JIS / EUC-JP / ISO-2022-JP の判定とデコード
  - サイズ・行数の上限
- `LineDiff` / `InlineDiff` / `DiffDisplay`
  - Myers 差分、文字単位差分、コンテキストの折りたたみ
- `StructuredNormalizer`(JSON / XML / CSV / TSV)
- `HexDiff`
- `SyncPlan`(読み取り専用の計画)
- `ThreeWayDiff`
- `GitSnapshot`(`git` コマンドを呼び出す)
- `RecentComparison` は UI 側の責務なので移植しない
- `DirectoryWatcher`(ポーリング。Swift 版と同じ間隔・上限)
- `ComparisonReport`(HTML / CSV / JSON / Markdown)
- CLI(`CLIOptions` / `CLIRunner` / `CLIExitCode` / `CLIReporter`): WinMerge 互換スイッチを含む

## 検証

1. Swift のテストを Rust の単体テストへ移植する。テスト名の対応表を `tests/parity/TEST_MAP.md` に置く。
2. 差分テスト `tests/parity/run.sh`
   - Swift 版 CLI と Rust 版 CLI を、次の fixture に同じ引数で実行する。
     - `SwiftUI/TestFixtures`
     - `SwiftUI/Scripts/make_demo_fixtures.sh` で作る fixture
     - `make_stress_fixtures.sh` で作る fixture
     - エンコーディング・改行・構造化ファイルの fixture(追加で用意する)
   - JSON レポートと終了コードを比べる。タイムスタンプなど実行ごとに変わる項目は正規化してから比べる。
   - `SwiftUI/Scripts/verify_cli.sh` と同等のケースを、Rust 版にもすべて通す。
3. 速度比較 `benches/compare.sh`
   - stress fixture(1万ファイル)、大きいテキストの行差分、大きいバイナリのハッシュで、Swift 版と Rust 版の wall time を計測する。
   - 結果を `docs/RUST_BENCH.md` に残す。Rust 版が遅い項目があれば原因を書く。

## 第2段階以降(別途依頼)

1. `diffloupe-ffi`(UniFFI)を作り、Mac の SwiftUI 版を Swift Core から Rust Core へ切り替える。
   `DiffLoupeEdition` などの UI 構造は維持する。
2. Linux GTK4 版と Windows Win32 版を作る。機能は Mac 版と同じ(Free の 2,000 件上限を含む)。
   Windows のビルドとテストは GitHub Actions で行う。
3. Pro: `diffloupe-pro-core`(Rust)に Pro の ProKit 相当を移植し、3 OS の Pro UI を作る。
