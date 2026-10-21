//! Swift `DiffLoupeTests/InlineDiffTests.swift` の 1:1 移植

use super::support::row;
use diffloupe_core::inline_diff::{self, InlineHighlight};
use diffloupe_core::line_diff::DiffRowKind;

fn hl(l: Option<&str>, r: Option<&str>, ci: bool) -> Option<InlineHighlight> {
    inline_diff::highlight(l, r, ci)
}

// MARK: - 基本

#[test]
fn test_identical_returns_nil() {
    assert!(hl(Some("abc"), Some("abc"), false).is_none());
}

#[test]
fn test_empty_strings_return_nil() {
    assert!(hl(Some(""), Some(""), false).is_none());
}

#[test]
fn test_nil_side_returns_nil() {
    assert!(hl(None, Some("abc"), false).is_none());
    assert!(hl(Some("abc"), None, false).is_none());
}

#[test]
fn test_single_character_change() {
    let h = hl(Some("abc"), Some("axc"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    assert_eq!(h.right_ranges, vec![1..2]);
}

#[test]
fn test_insertion() {
    let h = hl(Some("ab"), Some("aXb"), false).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![1..2]);
}

#[test]
fn test_deletion() {
    let h = hl(Some("aXb"), Some("ab"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    assert_eq!(h.right_ranges, Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn test_empty_left_insertion() {
    let h = hl(Some(""), Some("abc"), false).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![0..3]);
}

#[test]
fn test_empty_right_deletion() {
    let h = hl(Some("abc"), Some(""), false).unwrap();
    assert_eq!(h.left_ranges, vec![0..3]);
    assert_eq!(h.right_ranges, Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn test_insertion_at_both_ends() {
    let h = hl(Some("abc"), Some("XXabcYY"), false).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![0..2, 5..7]);
}

#[test]
fn test_deletion_at_both_ends() {
    let h = hl(Some("XXabcYY"), Some("abc"), false).unwrap();
    assert_eq!(h.left_ranges, vec![0..2, 5..7]);
    assert_eq!(h.right_ranges, Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn test_insertion_in_middle() {
    let h = hl(Some("abcdef"), Some("abxycdef"), false).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![2..4]);
}

#[test]
fn test_deletion_in_middle() {
    let h = hl(Some("abxycdef"), Some("abcdef"), false).unwrap();
    assert_eq!(h.left_ranges, vec![2..4]);
    assert_eq!(h.right_ranges, Vec::<std::ops::Range<usize>>::new());
}

// MARK: - 大文字小文字

#[test]
fn test_case_insensitive_ignores_case_only_difference() {
    assert!(hl(Some("Hello World"), Some("hello world"), true).is_none());
}

#[test]
fn test_case_sensitive_detects_case_difference() {
    let h = hl(Some("Hello World"), Some("hello world"), false).unwrap();
    assert_eq!(h.left_ranges, vec![0..1, 6..7]);
    assert_eq!(h.right_ranges, vec![0..1, 6..7]);
}

#[test]
fn test_case_insensitive_still_finds_real_difference() {
    assert!(hl(Some("Hello"), Some("hEllo"), true).is_none());
    let h = hl(Some("Hello"), Some("hello world"), true).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![5..11]);
}

// MARK: - Unicode

#[test]
fn test_unicode_characters() {
    let h = hl(Some("こんにちは"), Some("こんばんは"), false).unwrap();
    assert_eq!(h.left_ranges, vec![2..4]);
    assert_eq!(h.right_ranges, vec![2..4]);
}

#[test]
fn test_unicode_with_emoji() {
    let h = hl(Some("a🎉b"), Some("a😀b"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    assert_eq!(h.right_ranges, vec![1..2]);
}

// MARK: - 上限保護

#[test]
fn test_too_long_line_returns_nil() {
    let over_half = "x".repeat(inline_diff::MAX_CHARACTERS / 2 + 1);
    assert!(hl(Some(&over_half), Some(&over_half), false).is_none());
    let over_right = "y".repeat(inline_diff::MAX_CHARACTERS + 1);
    assert!(hl(Some("a"), Some(&over_right), false).is_none());
}

#[test]
fn test_at_limit_is_calculated() {
    let half = "x".repeat(inline_diff::MAX_CHARACTERS / 2 - 1);
    assert!(hl(Some(&half), Some(&(half.clone() + "y")), false).is_some());
}

// MARK: - highlightsForRows

#[test]
fn test_highlights_for_rows_only_modified() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Context,
            Some(1),
            Some("same"),
            Some(1),
            Some("same"),
        ),
        row(
            1,
            DiffRowKind::Modified,
            Some(2),
            Some("abc"),
            Some(2),
            Some("axc"),
        ),
        row(2, DiffRowKind::Added, None, None, Some(3), Some("new")),
        row(3, DiffRowKind::Removed, Some(4), Some("old"), None, None),
        row(
            4,
            DiffRowKind::Modified,
            Some(5),
            Some("Hello"),
            Some(5),
            Some("Hello World"),
        ),
    ];
    let highlights = inline_diff::highlights_for_rows(&rows, false);
    assert_eq!(highlights.len(), 2);
    assert!(highlights.contains_key(&1));
    assert!(!highlights.contains_key(&0));
    assert!(!highlights.contains_key(&2));
    assert!(!highlights.contains_key(&3));
    assert!(highlights.contains_key(&4));
}

#[test]
fn test_highlights_for_rows_respects_case_insensitive() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Modified,
            Some(1),
            Some("Hello World"),
            Some(1),
            Some("hello world"),
        ),
        row(
            1,
            DiffRowKind::Modified,
            Some(2),
            Some("abc"),
            Some(2),
            Some("axc"),
        ),
    ];
    let insensitive = inline_diff::highlights_for_rows(&rows, true);
    assert!(!insensitive.contains_key(&0));
    assert!(insensitive.contains_key(&1));
    let sensitive = inline_diff::highlights_for_rows(&rows, false);
    assert!(sensitive.contains_key(&0));
    assert!(sensitive.contains_key(&1));
}
