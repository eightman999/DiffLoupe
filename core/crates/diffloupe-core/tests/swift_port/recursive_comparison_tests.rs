//! Swift `CoreTests/RecursiveComparisonTests.swift` の 1:1 移植(5件)

use super::support::{compare, temp_pair, write};
use diffloupe_core::comparator::CompareOptions;
use diffloupe_core::models::DiffStatus;

fn non_recursive() -> CompareOptions {
    CompareOptions {
        recursive: false,
        ..Default::default()
    }
}

/// 既定(recursive=true)はネストしたファイルも比較対象に含む
#[test]
fn test_recursive_default_includes_nested_files() {
    let (base, l, r) = temp_pair("rc1");
    write(&l, "sub/nested.txt", b"AAA");
    write(&r, "sub/nested.txt", b"BBB");
    let result = compare(&l, &r, &CompareOptions::default());
    assert!(result.entry_by_path("sub").unwrap().is_directory);
    assert_eq!(
        result.entry_by_path("sub/nested.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(result.summary.different_files, 1);
    std::fs::remove_dir_all(&base).ok();
}

/// recursive=false は直下エントリのみ。ディレクトリは残すが降下しない
#[test]
fn test_non_recursive_keeps_top_level_directories_without_descending() {
    let (base, l, r) = temp_pair("rc2");
    write(&l, "sub/nested.txt", b"AAA");
    write(&r, "sub/nested.txt", b"BBB");
    let result = compare(&l, &r, &non_recursive());
    assert!(result.entry_by_path("sub").unwrap().is_directory);
    assert_eq!(
        result.entry_by_path("sub").unwrap().status,
        DiffStatus::Identical
    );
    assert!(result.entry_by_path("sub/nested.txt").is_none());
    assert_eq!(result.summary.total_files(), 0);
    std::fs::remove_dir_all(&base).ok();
}

/// recursive=false でもトップレベルのファイル差分は検出される
#[test]
fn test_non_recursive_detects_top_level_differences() {
    let (base, l, r) = temp_pair("rc3");
    write(&l, "top.txt", b"AAA");
    write(&r, "top.txt", b"BBB");
    write(&l, "only_left.txt", b"left only");
    let result = compare(&l, &r, &non_recursive());
    assert_eq!(
        result.entry_by_path("top.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        result.entry_by_path("only_left.txt").unwrap().status,
        DiffStatus::LeftOnly
    );
    assert_eq!(result.summary.different_files, 1);
    assert_eq!(result.summary.left_only_files, 1);
    std::fs::remove_dir_all(&base).ok();
}

/// 両側に同名ディレクトリだけがある場合、recursive=false では identical
#[test]
fn test_non_recursive_same_named_directory_is_identical() {
    let (base, l, r) = temp_pair("rc4");
    write(&l, "d/inner.txt", b"x");
    write(&r, "d/inner.txt", b"y");
    let result = compare(&l, &r, &non_recursive());
    assert!(result.entry_by_path("d").unwrap().is_directory);
    assert_eq!(
        result.entry_by_path("d").unwrap().status,
        DiffStatus::Identical
    );
    assert!(result.entry_by_path("d/inner.txt").is_none());
    std::fs::remove_dir_all(&base).ok();
}

/// recursive=false でも直下のディレクトリ名の片側のみは検出される
#[test]
fn test_non_recursive_detects_top_level_directory_only() {
    let (base, l, r) = temp_pair("rc5");
    write(&l, "left_only_dir/inner.txt", b"x");
    let result = compare(&l, &r, &non_recursive());
    assert!(result.entry_by_path("left_only_dir").unwrap().is_directory);
    assert_eq!(
        result.entry_by_path("left_only_dir").unwrap().status,
        DiffStatus::LeftOnly
    );
    std::fs::remove_dir_all(&base).ok();
}
