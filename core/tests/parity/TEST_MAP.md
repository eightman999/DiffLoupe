# テスト対応表(Swift → Rust 1:1 対応)

Swift 側の各非 UI テストメソッドを、Rust 側で**同名(snake_case)の個別 `#[test]`** として
1 対 1 移植した対応表。粗い集約テストへの寄せは行わない。

- コア側移植実体: `core/crates/diffloupe-core/tests/swift_port/`(mod 単位で Swift ファイルに対応)
- CLI 側移植実体: `core/crates/diffloupe-cli/tests/cli_swift_port.rs`(内部 mod で Swift ファイルに対応)
- Swift テスト関数名 `testFooBar` → Rust テスト関数名 `test_foo_bar`。
  Swift 側スイート名が衝突するファイル(CoreTests / DiffLoupeTests の同名ファイル)は
  Rust 側ファイル名で区別する。
- 差分テストハーネス: `core/tests/parity/run.sh`(86ケース)

## 件数サマリ

| Swift ファイル | Swift 件数 | Rust 移植件数 | 対象外 | 対象外理由 |
|---|---:|---:|---:|---|
| DiffLoupeTests/EngineOptionsTests.swift | 47 | 39 | 8 | UserDefaults/AppModel 依存(UI 層) |
| DiffLoupeTests/LineDiffTests.swift | 17 | 17 | 0 | — |
| DiffLoupeTests/InlineDiffTests.swift | 21 | 21 | 0 | — |
| DiffLoupeTests/DiffDisplayTests.swift | 18 | 18 | 0 | — |
| DiffLoupeTests/HexDiffTests.swift | 18 | 18 | 0 | — |
| DiffLoupeTests/StructuredNormalizerTests.swift | 15 | 14 | 1 | testStructuredModePersistsInUserDefaults(UserDefaults) |
| DiffLoupeTests/SyncPlanTests.swift | 3 | 3 | 0 | — |
| DiffLoupeTests/ThreeWayDiffTests.swift | 15 | 15 | 0 | — |
| DiffLoupeTests/ComparatorTests.swift | 8 | 8 | 0 | — |
| DiffLoupeTests/ComparisonReportTests.swift | 14 | 14 | 0 | — |
| DiffLoupeTests/GitSnapshotTests.swift | 8 | 8 | 0 | — |
| CoreTests/HexDiffTests.swift | 9 | 9 | 0 | — |
| CoreTests/SyncPlanTests.swift | 6 | 6 | 0 | — |
| CoreTests/StructuredNormalizerTests.swift | 31 | 31 | 0 | — |
| CoreTests/ThreeWayDiffTests.swift | 24 | 24 | 0 | — |
| CoreTests/FilterSetTests.swift | 23 | 23 | 0 | — |
| CoreTests/RecursiveComparisonTests.swift | 5 | 5 | 0 | — |
| CoreTests/DirectoryWatcherTests.swift | 15 | 15 | 0 | — |
| CLITests/CLIOptionsTests.swift | 32 | 32 | 0 | — |
| CLITests/CLIRunnerTests.swift | 13 | 13 | 0 | — |
| CLITests/ExitCodeTests.swift | 9 | 9 | 0 | — |
| CLITests/FilterCLITests.swift | 12 | 12 | 0 | — |
| CLITests/ReportOutputTests.swift | 8 | 8 | 0 | — |
| **合計** | **371** | **362** | **9** | — |

補足: `comparison_report_tests.rs` には上記 1:1 分に加えて、Foundation
ISO8601DateFormatter の丸め規則を検証する追加ユニットテスト
`test_iso8601_date_ms_rounding_rule` を 1 件含む(合計 289 tests in swift_port)。

## Swift ファイル → Rust ファイル対応

| Swift | Rust |
|---|---|
| EngineOptionsTests.swift | `swift_port/engine_options_tests.rs` |
| LineDiffTests.swift | `swift_port/line_diff_tests.rs` |
| InlineDiffTests.swift | `swift_port/inline_diff_tests.rs` |
| DiffDisplayTests.swift | `swift_port/diff_display_tests.rs` |
| DiffLoupeTests/HexDiffTests.swift + CoreTests/HexDiffTests.swift | `swift_port/hex_diff_tests.rs`(`diffloupe_tests` / `core_tests` mod) |
| DiffLoupeTests/SyncPlanTests.swift + CoreTests/SyncPlanTests.swift | `swift_port/sync_plan_tests.rs` |
| DiffLoupeTests/StructuredNormalizerTests.swift | `swift_port/structured_normalizer_diffloupe_tests.rs` |
| CoreTests/StructuredNormalizerTests.swift | `swift_port/structured_normalizer_core_tests.rs` |
| DiffLoupeTests/ThreeWayDiffTests.swift + CoreTests/ThreeWayDiffTests.swift | `swift_port/three_way_diff_tests.rs`(`diffloupe_tests` / `core_tests` mod) |
| ComparatorTests.swift | `swift_port/comparator_tests.rs` |
| ComparisonReportTests.swift | `swift_port/comparison_report_tests.rs` |
| GitSnapshotTests.swift | `swift_port/git_snapshot_tests.rs`(実 git 一時リポジトリ使用) |
| FilterSetTests.swift | `swift_port/filter_set_tests.rs` |
| RecursiveComparisonTests.swift | `swift_port/recursive_comparison_tests.rs` |
| DirectoryWatcherTests.swift | `swift_port/directory_watcher_tests.rs` |
| CLITests/CLIOptionsTests.swift | `cli_swift_port.rs::cli_options_tests` |
| CLITests/CLIRunnerTests.swift | `cli_swift_port.rs::cli_runner_tests` |
| CLITests/ExitCodeTests.swift | `cli_swift_port.rs::exit_code_tests` |
| CLITests/FilterCLITests.swift | `cli_swift_port.rs::filter_cli_tests` |
| CLITests/ReportOutputTests.swift | `cli_swift_port.rs::report_output_tests` |

## 対象外の個別メソッド

### EngineOptionsTests.swift(8件)— UserDefaults/AppModel 依存(UI 層)

- `testNormalizeExcludePatterns` — `AppModel.normalizeExcludePatterns`
- `testDefaultSettingsYieldStrictComparison` — `AppModel.makeOptions` + UserDefaults
- `testMakeOptionsReadsAllUserDefaults` — 同上
- `testNormalizeIncludePatterns` — `AppModel.normalizeIncludePatterns`
- `testMakeOptionsReadsIncludePatterns` — 同上
- `testMakeOptionsExpandsSelectedPresets` — 同上
- `testMakeOptionsWithoutPresetsDoesNotExpand` — 同上
- `testSelectedFilterPresetIDsDropsUnknownAndBlank` — `AppModel.selectedFilterPreset*`
  - ※ `testFilterSetTrimsAndDeduplicatesPatternsPreservingOrder` の後半に
    UserDefaults 経由の検証があるが、前半の FilterSet 純粋ロジック部分を
    `test_filter_set_trims_and_deduplicates_patterns_preserving_order` として移植済み。

### DiffLoupeTests/StructuredNormalizerTests.swift(1件)

- `testStructuredModePersistsInUserDefaults` — UserDefaults 永続化(UI 層)

## スイート単位の対象外(UI 専用、ファイル単位で未移植)

| Swift 側 | 対象外理由 |
|---|---|
| AppModelTests | SwiftUI/AppModel/UserDefaults の UI 層。エンジン外 |
| UserDefaults 依存テスト(上記個別メソッド) | 設定永続化は UI 層責務 |
| SwiftUI ビュー/プレビューのテスト | 描画層。Rust コアの範囲外 |
| RecentComparisonTests | 最近使用一覧は AppModel 配下の UI 状態 |
| ImageDiffEngineTests | 画像エンジンは phase 1 のスコープ外 |

## 備考

- `swift_port/support/mod.rs` はテンポラリ生成・比較実行などの共有ヘルパー。
  テスト並行実行に備え PID + アトミックカウンタで一意名を保証する。
- `GitSnapshotTests` は `/usr/bin/git` が存在する前提で一時リポジトリを構築する。
  不在環境では早期 return(Swift 側 XCTSkip 相当)。
- `DirectoryWatcherTests` のウォッチャー系 5 件はタイミング依存であり、
  Swift 側 `wait(for:timeout:)` と同じく時間窓で検証する。
- `ComparatorTests.testSymlinkHandling` / `DirectoryWatcherTests.testScanDoesNotFollowDirectorySymlinks`
  は UNIX シンボリックリンク前提のため `#[cfg(unix)]`。
