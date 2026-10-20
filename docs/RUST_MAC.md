# macOS SwiftUI 版の Rust コア化

macOS アプリ(`SwiftUI/`)のエンジンを Swift 実装から Rust コア(`core/crates/diffloupe-core`)へ切り替えた。UI・Pro 版が依存する Swift の公開 API は変えていない。

## 構成
- `core/crates/diffloupe-ffi`: UniFFI(proc-macro 方式)で core を包む。関数名は `diffloupe_*`、レコードは `Ffi*`(`types.rs`)。フォルダ比較は同期ブロッキングで、進捗コールバック・キャンセルトークン・エントリ上限を持つ。
- `SwiftUI/Scripts/build_rust_core.sh`: `release-ffi` プロファイル(opt-level 2, fat LTO)で aarch64 / x86_64 の両ターゲットをビルド(x86_64 は未導入なら `rustup target add`)し、`lipo` でユニバーサル `libdiffloupe_ffi.a` を作る。UniFFI で Swift バインディングを生成し、`SwiftUI/RustCore/`(include, xcframework, bindings)と `DiffLoupe/Engine/Generated/diffloupe.swift` に配置する。生成物は git 管理外。
- Swift モジュール `DiffLoupeCore` は同名・同ファイル構成の薄いファサード。型宣言と public/internal API(テストが使う internal を含む)を残し、実装だけ生成バインディング呼び出しに置換した。`ImageDiffEngine.swift` は AppKit のため Swift のまま。
- SwiftPM: `Package.swift` の `.binaryTarget`(`RustCore/diffloupeFFI.xcframework`)を `DiffLoupeCore` が依存する。SwiftPM にはビルドフックがないため、`swift build/test` の前に `Scripts/build_rust_core.sh` を一度実行する(`verify_cli.sh` / `build_cli.sh` は先頭で自動実行)。`unsafeFlags` は path 依存(Pro)を壊すため使っていない。
- Xcode: DiffLoupe ターゲットに Run Script フェーズ「Build Rust Core」(Sources より前)を追加。`SWIFT_INCLUDE_PATHS` と `-ldiffloupe_ffi` を設定し、生成 Swift を両ターゲット(アプリ・テスト)に追加。
- CLI: Swift 版 `diffloupe` を維持し、`CLILib` がファサード経由で Rust コアを使う(Pro が `DiffLoupeCLI` に依存するため API 維持が必要。Rust CLI への置換は行わない)。

## core への追加 API
- `diffloupe_core::comparator::sha256_file` を `pub` 化(振る舞い変更なし)。

## 依存の追加
- `uniffi 0.32`(diffloupe-ffi 限定)。バインディング生成器(`uniffi/cli`)は `bindgen` フィーチャ配下に分離し、配布する static library には含めない(含めるとアプリが約10MB増えた)。

## 検証結果
| 項目 | 結果 |
|---|---|
| `cargo test`(core) | 407 PASS / 0 FAIL |
| `core/tests/parity/run.sh` | 86/86 一致(高負荷時に mtime 系ケースが 1 度だけ揺れたが、再実行で一致。負荷依存の既存の揺らぎ) |
| `swift build` / `swift test` | 成功 / 189 件中 187 PASS, 2 SKIP(性能計測。`DIFFLOUPE_STRESS_DIR` 未設定時), 0 FAIL |
| `xcodebuild build test`(ad-hoc) | 成功 / 229 PASS, 0 FAIL |
| `Scripts/verify_cli.sh` | ALL PASSED(FAIL 0) |
| Pro 一時コピー(path 依存を本 worktree へ) | `swift build` 成功 / `swift test` 90 PASS, 0 FAIL。Pro 側 API 変更は不要 |
| `lipo -archs`(Release アプリ) | x86_64 arm64 |
| 起動確認 | 5 秒生存、quit で終了 |

## サイズ
Release の universal .app(未 strip)は Swift コア時代 7.4MB に対し 13.6MB。7.4MB 以下には戻せなかった。主因は「Rust コアが 1 アーキテクチャあたり約 1.9MB(×2 アーキ)」で、Swift 側の置換で減った分(約 0.4MB/arch)を大きく上回るため。リンクマップ(`LD_GENERATE_MAP_FILE`)で arm64 の内訳を取った結果:

| arm64 の内訳(最終構成) | サイズ |
|---|---|
| Rust 静的ライブラリ全体 | 約 1.89MB |
| うち core の Unicode テーブル(NFC/小文字化) | 約 0.25MB(改善前は約 0.45MB + 再配置) |
| うち encoding_rs(Shift_JIS/EUC-JP 等) | 約 0.26MB |
| うち std の backtrace 周り(gimli/addr2line/rustc_demangle/miniz) | 約 0.13MB(stable では除去不可。`-Zbuild-std` 等の nightly が必要) |
| UniFFI 生成 Swift(`diffloupe.swift`) | 約 0.37MB |
| アプリ Swift(UI + ファサード) | 約 1.0MB |

試した施策と結果(universal 実行ファイルを `strip -x` した後のサイズで比較。変更前の Swift コアは 3.3MB):

| 施策 | strip 後 | 判定 |
|---|---|---|
| 初期構成(release, opt-level 3, thin LTO, uniffi/cli 同梱) | 18.3MB(.app 24.4MB) | - |
| `uniffi/cli` を `bindgen` フィーチャへ分離 | 9.25MB(.app 14.5MB) | 採用 |
| FFI 用プロファイル opt-level z + fat LTO | 9.76MB | 不採用(逆に大きく、速度も遅い) |
| opt-level s + fat / thin LTO | 10.0MB | 不採用 |
| opt-level 2 + fat / thin LTO | 9.12MB | 採用(LTO の差は出ない) |
| Unicode テーブルをポインタ無しの (cp, offset, len) + 共有配列へ変更 | 8.71MB(.app 13.6MB) | 採用 |
| 依存クレートのみ opt-level z、sha2/memchr は 2 | 8.78MB | 不採用(効果なし) |
| `-Wl,-dead_strip` | Release では既定で有効 | 追加効果なし |
| 旧 Swift 実装の二重リンク | リンクマップに旧エンジンのオブジェクトなし | 問題なし |

opt-level は z / s ほど遅く(1万ファイル比較の core 部: opt 2 で約 0.34s、s 0.40s、z 0.59s)、サイズも小さくならなかったため 2 を採用。FFI 用の `release-ffi` プロファイル(`core/Cargo.toml`)は CLI の `release` と分離してある。さらに削るには nightly の `build-std`(backtrace 除去)、encoding_rs の用途限定、UniFFI 生成コードの削減(公開関数の整理)が必要で、今回は見送った。

## 速度(1 万ファイル再帰比較)
FFI 境界のオーバーヘッドを削った: (1) 結果ツリーが全エントリの相対パスを 2 重に転送していたため、ツリーノードをエントリ参照(index)にして文字列転送を無くした(Swift → Rust 方向のツリーは Rust 側で再構築)、(2) `URL(fileURLWithPath:)` が毎回ディレクトリ判定の stat を発行していたため、既知の `isDirectory` を渡すようにした。進捗コールバックは 1% ごと(約 100 回)で影響なし。

| 指標 | 変更前 Swift コア | Rust 化直後 | 最適化後 |
|---|---|---|---|
| Swift 型への変換(`ComparisonResult(ffi)`) | - | 約 74ms | 約 20ms |
| FFI 呼び出し − core 比較本体 | - | 約 100ms | 約 30ms |
| `FolderComparator.compare` 1 万ファイル(XCTest, 7 回の min / median) | 0.44〜0.47 / 0.57〜0.61s | 0.51 / 0.52s | 0.41〜0.43 / 0.42〜0.60s |
| Swift CLI `-r -q`(15 回の min) | 0.50〜0.51s | - | 0.47〜0.48s |
| 参考: Rust CLI 直接(15 回の min) | - | - | 0.34〜0.36s |

計測は他プロセスの負荷が高い環境(load average 12〜20)で行ったため、中央値は揺れる。min で比較すると最適化後は Swift コア時代と同等〜やや高速(約 5〜10%)で、「明確に高速」とまでは言えない。残りの時間は core 比較本体(ファイル I/O と SHA-256)が支配的で、FFI 変換は約 30ms まで減った。計測方法: `DIFFLOUPE_STRESS_DIR=<dir> swift test -c release --filter FolderComparatorPerformanceTests`(Rust 版のみ内訳テストあり)、`DIFFLOUPE_STRESS_DIR=<dir> cargo test --profile release-ffi -p diffloupe-ffi --test perf_stress -- --ignored --nocapture`。
