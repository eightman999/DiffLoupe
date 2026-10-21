//! Swift `DiffLoupeTests/ThreeWayDiffTests.swift` と `CoreTests/ThreeWayDiffTests.swift` の 1:1 移植

use diffloupe_core::options::ContentComparison;
use diffloupe_core::three_way::{ThreeWayDiff, ThreeWayDiffStatus as TWS};

/// Swift の makeText と同等(末尾改行なしの結合)
fn t(lines: &[&str]) -> String {
    lines.join("\n")
}

fn cc() -> ContentComparison {
    ContentComparison::default()
}

/// DiffLoupeTests/ThreeWayDiffTests.swift(15件)
mod diffloupe_tests {
    use super::*;

    #[test]
    fn test_unchanged_all_sides_identical() {
        let text = t(&["a", "b", "c"]);
        let diff = ThreeWayDiff::make(&text, &text, &text, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 3);
        assert!(diff.lines.iter().all(|l| l.status == TWS::Unchanged));
        assert_eq!(diff.total_changed_count(), 0);
    }

    #[test]
    fn test_left_only_change_is_left_changed() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "b", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].left.as_deref(), Some("X"));
        assert_eq!(diff.left_changed_count, 1);
    }

    #[test]
    fn test_right_only_change_is_right_changed() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "b", "c"]),
            t(&["a", "Y", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::RightChanged);
        assert_eq!(diff.right_changed_count, 1);
    }

    #[test]
    fn test_both_changed_same() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "X", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::BothChangedSame);
        assert_eq!(diff.both_changed_same_count, 1);
        assert_eq!(diff.conflict_count, 0);
    }

    #[test]
    fn test_conflict() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "Y", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::Conflict);
        assert_eq!(diff.conflict_count, 1);
    }

    #[test]
    fn test_insert_and_delete() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "x", "b"]),
            t(&["a", "b", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].left.as_deref(), Some("x"));
        assert_eq!(diff.lines[2].status, TWS::Unchanged);
        assert_eq!(diff.lines[2].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[3].status, TWS::LeftChanged);
        assert_eq!(diff.lines[3].base.as_deref(), Some("c"));
        assert_eq!(diff.lines[3].left, None);
        assert_eq!(diff.lines[3].right.as_deref(), Some("c"));
    }

    #[test]
    fn test_overlapping_hunks_delete_both_sides() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B", "D"]),
            t(&["A", "B", "C"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[2].status, TWS::LeftChanged);
        assert_eq!(diff.lines[3].status, TWS::RightChanged);
    }

    // MARK: - ハンク整列の回帰(監査指摘の実反例)

    #[test]
    fn test_replacement_merges_to_conflict_ce1() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B'", "D"]),
            t(&["A", "B", "C'", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("B'"));
        assert_eq!(diff.lines[2].status, TWS::Conflict);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left, None);
        assert_eq!(diff.lines[2].right.as_deref(), Some("C'"));
        assert_eq!(diff.lines[3].status, TWS::Unchanged);
        assert_eq!(diff.conflict_count, 1);
        assert_eq!(diff.both_changed_same_count, 0);
        assert_eq!(diff.right_changed_count, 0);
    }

    #[test]
    fn test_replacement_merges_to_conflict_symmetric_ce2() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B", "C'", "D"]),
            t(&["A", "B'", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[1].status, TWS::RightChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].right.as_deref(), Some("B'"));
        assert_eq!(diff.lines[2].status, TWS::Conflict);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("C'"));
        assert_eq!(diff.lines[2].right, None);
        assert_eq!(diff.conflict_count, 1);
        assert_eq!(diff.both_changed_same_count, 0);
        assert_eq!(diff.left_changed_count, 0);
    }

    #[test]
    fn test_left_delete_right_insert_not_isolated() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "C", "D"]),
            t(&["A", "B", "X", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].left, None);
        assert_eq!(diff.lines[2].status, TWS::RightChanged);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].right.as_deref(), Some("X"));
        assert_eq!(diff.lines[3].status, TWS::Unchanged);
        assert_eq!(diff.right_changed_count, 1);
    }

    #[test]
    fn test_insert_delete_change_combination_preserved() {
        let (base, left, right) = (
            t(&["a", "b", "c", "d"]),
            t(&["a", "X", "c"]),
            t(&["a", "b", "Y", "d"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[0].status, TWS::Unchanged);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("X"));
        assert_eq!(diff.lines[2].status, TWS::RightChanged);
        assert_eq!(diff.lines[2].base.as_deref(), Some("c"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("c"));
        assert_eq!(diff.lines[2].right.as_deref(), Some("Y"));
        assert_eq!(diff.lines[3].status, TWS::LeftChanged);
        assert_eq!(diff.lines[3].base.as_deref(), Some("d"));
        assert_eq!(diff.lines[3].left, None);
        assert_eq!(diff.lines[3].right.as_deref(), Some("d"));
        assert_eq!(diff.left_changed_count, 2);
        assert_eq!(diff.right_changed_count, 1);
        assert_eq!(diff.conflict_count, 0);
    }

    #[test]
    fn test_ignore_case_uses_normalization_for_judging_but_keeps_original_display() {
        let (base, left, right) = (t(&["foo", "bar"]), t(&["FOO", "bar"]), t(&["FOO", "bar"]));
        let options = ContentComparison {
            ignore_case: true,
            ..Default::default()
        };
        let diff = ThreeWayDiff::make(&base, &left, &right, &options);
        assert!(diff.lines.iter().all(|l| l.status == TWS::Unchanged));
        assert_eq!(diff.total_changed_count(), 0);
        assert_eq!(diff.lines[0].left.as_deref(), Some("FOO"));
    }

    #[test]
    fn test_ignore_case_marks_same_normalized_change_as_both_changed_same() {
        let (base, left, right) = (
            t(&["foo", "bar"]),
            t(&["FOO x", "bar"]),
            t(&["FOO x", "bar"]),
        );
        let options = ContentComparison {
            ignore_case: true,
            ..Default::default()
        };
        let diff = ThreeWayDiff::make(&base, &left, &right, &options);
        assert_eq!(diff.lines[0].status, TWS::BothChangedSame);
    }

    #[test]
    fn test_truncated_over_total_line_cap() {
        let line_count = 14_000usize;
        let base = vec!["base"; line_count].join("\n");
        let left = vec!["left"; line_count].join("\n");
        let right = vec!["right"; line_count].join("\n");
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(diff.truncated);
        assert_eq!(diff.lines.len(), 0);
    }

    #[test]
    fn test_not_truncated_at_cap_boundary() {
        let line_count = 13_000usize;
        let mut base = vec!["common".to_string(); line_count];
        let mut left = base.clone();
        let mut right = base.clone();
        base[line_count - 1] = "base end".into();
        left[line_count - 1] = "left end".into();
        right[line_count - 1] = "right end".into();
        let diff = ThreeWayDiff::make(&base.join("\n"), &left.join("\n"), &right.join("\n"), &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines[line_count - 1].status, TWS::Conflict);
    }
}

/// CoreTests/ThreeWayDiffTests.swift(24件)
mod core_tests {
    use super::*;
    use std::collections::HashSet;

    // MARK: - unchanged

    #[test]
    fn test_unchanged_all_sides_identical() {
        let text = t(&["a", "b", "c"]);
        let diff = ThreeWayDiff::make(&text, &text, &text, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 3);
        assert!(diff.lines.iter().all(|l| l.status == TWS::Unchanged));
        assert_eq!(diff.unchanged_count, 3);
        assert_eq!(diff.conflict_count, 0);
        assert_eq!(diff.total_changed_count(), 0);
        // 原文表示と行番号
        assert_eq!(diff.lines[0].base.as_deref(), Some("a"));
        assert_eq!(diff.lines[0].left.as_deref(), Some("a"));
        assert_eq!(diff.lines[0].right.as_deref(), Some("a"));
        assert_eq!(diff.lines[1].base_number, Some(2));
        assert_eq!(diff.lines[1].left_number, Some(2));
        assert_eq!(diff.lines[1].right_number, Some(2));
    }

    #[test]
    fn test_empty_files_are_unchanged() {
        let diff = ThreeWayDiff::make("", "", "", &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 1);
        assert_eq!(diff.lines[0].status, TWS::Unchanged);
    }

    // MARK: - 片側変更

    #[test]
    fn test_left_only_change_is_left_changed() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "b", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines.len(), 3);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("X"));
        assert_eq!(diff.lines[1].right.as_deref(), Some("b"));
        assert_eq!(diff.left_changed_count, 1);
        assert_eq!(diff.right_changed_count, 0);
    }

    #[test]
    fn test_right_only_change_is_right_changed() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "b", "c"]),
            t(&["a", "Y", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::RightChanged);
        assert_eq!(diff.lines[1].right.as_deref(), Some("Y"));
        assert_eq!(diff.right_changed_count, 1);
    }

    // MARK: - 両者同一変更 / conflict

    #[test]
    fn test_both_changed_same() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "X", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::BothChangedSame);
        assert_eq!(diff.both_changed_same_count, 1);
        assert_eq!(diff.conflict_count, 0);
    }

    #[test]
    fn test_conflict() {
        let (base, left, right) = (
            t(&["a", "b", "c"]),
            t(&["a", "X", "c"]),
            t(&["a", "Y", "c"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::Conflict);
        assert_eq!(diff.conflict_count, 1);
        assert_eq!(diff.both_changed_same_count, 0);
    }

    // MARK: - 挿入・削除

    #[test]
    fn test_insert_on_left_only() {
        let (base, left, right) = (t(&["a", "b"]), t(&["a", "x", "b"]), t(&["a", "b"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines.len(), 3);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base, None);
        assert_eq!(diff.lines[1].left.as_deref(), Some("x"));
        assert_eq!(diff.lines[1].right, None);
    }

    #[test]
    fn test_delete_on_left_only() {
        let (base, left, right) = (t(&["a", "b", "c"]), t(&["a", "c"]), t(&["a", "b", "c"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[1].left, None);
        assert_eq!(diff.lines[1].right.as_deref(), Some("b"));
    }

    #[test]
    fn test_both_insert_same_line() {
        let (base, left, right) = (t(&["a", "b"]), t(&["a", "x", "b"]), t(&["a", "x", "b"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::BothChangedSame);
        assert_eq!(diff.both_changed_same_count, 1);
    }

    #[test]
    fn test_insert_conflict() {
        let (base, left, right) = (t(&["a", "b"]), t(&["a", "x", "b"]), t(&["a", "y", "b"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::Conflict);
        assert_eq!(diff.conflict_count, 1);
    }

    #[test]
    fn test_delete_on_both_sides_is_both_changed_same() {
        let (base, left, right) = (t(&["a", "b", "c"]), t(&["a", "c"]), t(&["a", "c"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::BothChangedSame);
        assert_eq!(diff.lines[1].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[1].left, None);
        assert_eq!(diff.lines[1].right, None);
        assert_eq!(diff.both_changed_same_count, 1);
    }

    #[test]
    fn test_overlapping_hunks_delete_both_sides() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B", "D"]),
            t(&["A", "B", "C"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[2].status, TWS::LeftChanged);
        assert_eq!(diff.lines[3].status, TWS::RightChanged);
    }

    #[test]
    fn test_delete_versus_change_is_conflict() {
        let (base, left, right) = (t(&["A", "B", "C"]), t(&["A", "C"]), t(&["A", "X", "C"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[1].status, TWS::Conflict);
        assert_eq!(diff.conflict_count, 1);
    }

    // MARK: - ハンク整列の回帰(監査指摘の実反例)

    #[test]
    fn test_replacement_merges_to_conflict_ce1() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B'", "D"]),
            t(&["A", "B", "C'", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[0].status, TWS::Unchanged);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("B'"));
        assert_eq!(diff.lines[1].right.as_deref(), Some("B"));
        assert_eq!(diff.lines[2].status, TWS::Conflict);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left, None);
        assert_eq!(diff.lines[2].right.as_deref(), Some("C'"));
        assert_eq!(diff.lines[3].status, TWS::Unchanged);
        assert_eq!(diff.lines[3].base.as_deref(), Some("D"));
        assert_eq!(diff.conflict_count, 1);
        assert_eq!(diff.both_changed_same_count, 0);
        assert_eq!(diff.right_changed_count, 0);
    }

    #[test]
    fn test_replacement_merges_to_conflict_symmetric_ce2() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "B", "C'", "D"]),
            t(&["A", "B'", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[1].status, TWS::RightChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].right.as_deref(), Some("B'"));
        assert_eq!(diff.lines[2].status, TWS::Conflict);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("C'"));
        assert_eq!(diff.lines[2].right, None);
        assert_eq!(diff.lines[3].status, TWS::Unchanged);
        assert_eq!(diff.conflict_count, 1);
        assert_eq!(diff.both_changed_same_count, 0);
        assert_eq!(diff.left_changed_count, 0);
    }

    #[test]
    fn test_left_delete_right_insert_not_isolated() {
        let (base, left, right) = (
            t(&["A", "B", "C", "D"]),
            t(&["A", "C", "D"]),
            t(&["A", "B", "X", "D"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("B"));
        assert_eq!(diff.lines[1].left, None);
        assert_eq!(diff.lines[1].right.as_deref(), Some("B"));
        assert_eq!(diff.lines[2].status, TWS::RightChanged);
        assert_eq!(diff.lines[2].base.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("C"));
        assert_eq!(diff.lines[2].right.as_deref(), Some("X"));
        assert_eq!(diff.lines[3].status, TWS::Unchanged);
        assert_eq!(diff.right_changed_count, 1);
    }

    #[test]
    fn test_insert_delete_change_combination_preserved() {
        let (base, left, right) = (
            t(&["a", "b", "c", "d"]),
            t(&["a", "X", "c"]),
            t(&["a", "b", "Y", "d"]),
        );
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), 4);
        assert_eq!(diff.lines[0].status, TWS::Unchanged);
        assert_eq!(diff.lines[1].status, TWS::LeftChanged);
        assert_eq!(diff.lines[1].base.as_deref(), Some("b"));
        assert_eq!(diff.lines[1].left.as_deref(), Some("X"));
        assert_eq!(diff.lines[1].right.as_deref(), Some("b"));
        assert_eq!(diff.lines[2].status, TWS::RightChanged);
        assert_eq!(diff.lines[2].base.as_deref(), Some("c"));
        assert_eq!(diff.lines[2].left.as_deref(), Some("c"));
        assert_eq!(diff.lines[2].right.as_deref(), Some("Y"));
        assert_eq!(diff.lines[3].status, TWS::LeftChanged);
        assert_eq!(diff.lines[3].base.as_deref(), Some("d"));
        assert_eq!(diff.lines[3].left, None);
        assert_eq!(diff.lines[3].right.as_deref(), Some("d"));
        assert_eq!(diff.left_changed_count, 2);
        assert_eq!(diff.right_changed_count, 1);
        assert_eq!(diff.conflict_count, 0);
    }

    // MARK: - 正規化(判定のみに使用・表示は原文)

    #[test]
    fn test_ignore_case_uses_normalization_for_judging_but_keeps_original_display() {
        let (base, left, right) = (t(&["foo", "bar"]), t(&["FOO", "bar"]), t(&["FOO", "bar"]));
        let options = ContentComparison {
            ignore_case: true,
            ..Default::default()
        };
        let diff = ThreeWayDiff::make(&base, &left, &right, &options);
        assert!(diff.lines.iter().all(|l| l.status == TWS::Unchanged));
        assert_eq!(diff.total_changed_count(), 0);
        assert_eq!(diff.lines[0].base.as_deref(), Some("foo"));
        assert_eq!(diff.lines[0].left.as_deref(), Some("FOO"));
        assert_eq!(diff.lines[0].right.as_deref(), Some("FOO"));
    }

    #[test]
    fn test_ignore_case_marks_same_normalized_change_as_both_changed_same() {
        let (base, left, right) = (
            t(&["foo", "bar"]),
            t(&["FOO x", "bar"]),
            t(&["FOO x", "bar"]),
        );
        let options = ContentComparison {
            ignore_case: true,
            ..Default::default()
        };
        let diff = ThreeWayDiff::make(&base, &left, &right, &options);
        assert_eq!(diff.lines[0].status, TWS::BothChangedSame);
        assert_eq!(diff.lines[0].left.as_deref(), Some("FOO x"));
    }

    #[test]
    fn test_ignore_trailing_whitespace_treats_whitespace_only_diff_as_unchanged() {
        let (base, left, right) = (t(&["x ", "y"]), t(&["x", "y"]), t(&["x", "y"]));
        let options = ContentComparison {
            ignore_trailing_whitespace: true,
            ..Default::default()
        };
        let diff = ThreeWayDiff::make(&base, &left, &right, &options);
        assert_eq!(diff.total_changed_count(), 0);
        assert_eq!(diff.lines[0].left.as_deref(), Some("x"));
    }

    #[test]
    fn test_strict_comparison_treats_case_difference_as_change() {
        let (base, left, right) = (t(&["foo"]), t(&["FOO"]), t(&["foo"]));
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        assert_eq!(diff.lines[0].status, TWS::LeftChanged);
    }

    // MARK: - 40k上限

    #[test]
    fn test_truncated_over_total_line_cap() {
        let line_count = 14_000usize;
        let base = vec!["base"; line_count].join("\n");
        let left = vec!["left"; line_count].join("\n");
        let right = vec!["right"; line_count].join("\n");
        let diff = ThreeWayDiff::make(&base, &left, &right, &cc());
        // 14k+14k+14k = 42k > 40k → 中断
        assert!(diff.truncated);
        assert_eq!(diff.lines.len(), 0);
    }

    #[test]
    fn test_not_truncated_at_cap_boundary() {
        let line_count = 13_000usize;
        let mut base = vec!["common".to_string(); line_count];
        let mut left = base.clone();
        let mut right = base.clone();
        base[line_count - 1] = "base end".into();
        left[line_count - 1] = "left end".into();
        right[line_count - 1] = "right end".into();
        let diff = ThreeWayDiff::make(&base.join("\n"), &left.join("\n"), &right.join("\n"), &cc());
        // 39k ≤ 40k → 計算する。最終行は左右が異なる変更で競合
        assert!(!diff.truncated);
        assert_eq!(diff.lines.len(), line_count);
        assert_eq!(diff.lines[line_count - 1].status, TWS::Conflict);
        assert_eq!(diff.conflict_count, 1);
    }

    // MARK: - Sendable / Equatable / Hashable

    #[test]
    fn test_sendable_equatable_hashable() {
        let text = t(&["a", "b"]);
        let diff1 = ThreeWayDiff::make(&text, &text, &text, &cc());
        let diff2 = ThreeWayDiff::make(&text, &text, &text, &cc());
        assert_eq!(diff1, diff2);
        assert_eq!(diff1.lines[0], diff2.lines[0]);
        let line_set: HashSet<_> = diff1.lines.iter().collect();
        assert_eq!(line_set.len(), diff1.lines.len());
        let diff_set: HashSet<_> = [diff1, diff2].into_iter().collect();
        assert_eq!(diff_set.len(), 1);
    }
}
