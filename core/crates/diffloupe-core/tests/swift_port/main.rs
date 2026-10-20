//! Swift 非UI テストの 1:1 移植スイート。
//! 各モジュールは Swift 側テストファイルに対応し、テスト関数名も
//! Swift の `testXxx` を snake_case にした同名とする。
//! 対応表: core/tests/parity/TEST_MAP.md

mod comparator_tests;
mod comparison_report_tests;
mod diff_display_tests;
mod directory_watcher_tests;
mod engine_options_tests;
mod filter_set_tests;
mod git_snapshot_tests;
mod hex_diff_tests;
mod inline_diff_tests;
mod line_diff_tests;
mod recursive_comparison_tests;
mod structured_normalizer_core_tests;
mod structured_normalizer_diffloupe_tests;
mod support;
mod sync_plan_tests;
mod three_way_diff_tests;
