//! Swift `DiffLoupeTests/DiffDisplayTests.swift` の 1:1 移植

use super::support::row;
use diffloupe_core::diff_display::{self as dd, DisplayItem};
use diffloupe_core::line_diff::{DiffRow, DiffRowKind};
use std::collections::HashSet;

/// Swift の makeRows と同一構成:
/// 0..<2: context 2連続 / 2: modified / 3..<10: context 7連続 / 10: added / 11..<14: context 3連続
fn make_rows() -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut id = 0usize;
    let mut add = |kind: DiffRowKind, l: Option<&str>, r: Option<&str>, id: &mut usize| {
        let i = *id;
        rows.push(row(i, kind, Some(i + 1), l, Some(i + 1), r));
        *id += 1;
    };
    add(DiffRowKind::Context, Some("c1"), Some("c1"), &mut id);
    add(DiffRowKind::Context, Some("c2"), Some("c2"), &mut id);
    add(DiffRowKind::Modified, Some("m1"), Some("m1'"), &mut id);
    for i in 0..7 {
        let s = format!("g{}", i);
        add(DiffRowKind::Context, Some(&s.clone()), Some(&s), &mut id);
    }
    add(DiffRowKind::Added, None, Some("new"), &mut id);
    for i in 1..=3 {
        let s = format!("x{}", i);
        add(DiffRowKind::Context, Some(&s.clone()), Some(&s), &mut id);
    }
    rows
}

fn set(ids: &[usize]) -> HashSet<usize> {
    ids.iter().copied().collect()
}

// MARK: - 検索

#[test]
fn test_search_hit_row_ids() {
    let rows = make_rows();
    assert_eq!(
        dd::search_hit_row_ids(&rows, "g", true),
        (3..=9).collect::<Vec<_>>()
    );
    assert_eq!(dd::search_hit_row_ids(&rows, "g0", true), vec![3]);
    assert_eq!(dd::search_hit_row_ids(&rows, "new", true), vec![10]);
    assert_eq!(dd::search_hit_row_ids(&rows, "m1", true), vec![2]);
    assert_eq!(dd::search_hit_row_ids(&rows, "x", true), vec![11, 12, 13]);
    assert_eq!(
        dd::search_hit_row_ids(&rows, "z", true),
        Vec::<usize>::new()
    );
}

#[test]
fn test_search_case_insensitive() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Context,
            Some(1),
            Some("Hello"),
            Some(1),
            Some("Hello"),
        ),
        row(
            1,
            DiffRowKind::Context,
            Some(2),
            Some("world"),
            Some(2),
            Some("WORLD"),
        ),
    ];
    assert_eq!(dd::search_hit_row_ids(&rows, "hello", false), vec![0]);
    assert_eq!(dd::search_hit_row_ids(&rows, "world", false), vec![1]);
    assert_eq!(
        dd::search_hit_row_ids(&rows, "hello", true),
        Vec::<usize>::new()
    );
    assert_eq!(
        dd::search_hit_row_ids(&rows, "none", false),
        Vec::<usize>::new()
    );
}

#[test]
fn test_empty_query_returns_empty() {
    let rows = make_rows();
    assert_eq!(
        dd::search_hit_row_ids(&rows, "", false),
        Vec::<usize>::new()
    );
    assert_eq!(dd::search_hit_row_ids(&rows, "", true), Vec::<usize>::new());
}

#[test]
fn test_search_uses_non_null_side() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Added,
            None,
            None,
            Some(1),
            Some("added-content"),
        ),
        row(
            1,
            DiffRowKind::Removed,
            Some(2),
            Some("removed-content"),
            None,
            None,
        ),
    ];
    assert_eq!(dd::search_hit_row_ids(&rows, "added", true), vec![0]);
    assert_eq!(dd::search_hit_row_ids(&rows, "removed", true), vec![1]);
    assert_eq!(dd::search_hit_row_ids(&rows, "content", true), vec![0, 1]);
}

// MARK: - 折りたたみ

#[test]
fn test_fold_groups() {
    let rows = make_rows();
    let groups = dd::fold_groups(&rows, dd::COLLAPSE_THRESHOLD);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].count, 7);
    assert_eq!(groups[0].row_ids, (3..=9).collect::<Vec<_>>());
}

#[test]
fn test_fold_groups_below_threshold() {
    let rows = make_rows();
    assert_eq!(dd::fold_groups(&rows, 4).len(), 1);
    let small = dd::fold_groups(&rows, 2);
    assert_eq!(small.len(), 3);
    assert_eq!(
        small.iter().map(|g| g.count).collect::<Vec<_>>(),
        vec![2, 7, 3]
    );
    assert_eq!(dd::fold_groups(&rows, 8).len(), 0);
}

#[test]
fn test_fold_groups_splits_at_difference_rows() {
    let mut rows: Vec<DiffRow> = Vec::new();
    for i in 0..5 {
        let s = format!("a{}", i);
        rows.push(row(
            i,
            DiffRowKind::Context,
            Some(i + 1),
            Some(&s),
            Some(i + 1),
            Some(&s),
        ));
    }
    rows.push(row(5, DiffRowKind::Added, None, None, Some(6), Some("new")));
    for i in 6..11 {
        let s = format!("b{}", i);
        rows.push(row(
            i,
            DiffRowKind::Context,
            Some(i + 1),
            Some(&s),
            Some(i + 1),
            Some(&s),
        ));
    }
    let groups = dd::fold_groups(&rows, dd::COLLAPSE_THRESHOLD);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].row_ids, (0..5).collect::<Vec<_>>());
    assert_eq!(groups[1].row_ids, (6..11).collect::<Vec<_>>());
}

#[test]
fn test_fold_groups_only_context_rows() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Modified,
            Some(1),
            Some("m"),
            Some(1),
            Some("m'"),
        ),
        row(1, DiffRowKind::Removed, Some(2), Some("r"), None, None),
    ];
    assert_eq!(dd::fold_groups(&rows, dd::COLLAPSE_THRESHOLD).len(), 0);
}

// MARK: - 表示アイテム

#[test]
fn test_display_items_collapsed_by_default() {
    let rows = make_rows();
    let items = dd::display_items(&rows, &set(&[]), false, dd::COLLAPSE_THRESHOLD);
    // 2連続・3連続は非折りたたみ、7連続のみヘッダに集約される
    assert_eq!(items.len(), 8);
    let collapsed = items
        .iter()
        .filter(|i| matches!(i, DisplayItem::Collapsed(_)))
        .count();
    assert_eq!(collapsed, 1);
    let row_count = items
        .iter()
        .filter(|i| matches!(i, DisplayItem::Row(_)))
        .count();
    assert_eq!(row_count, 7);
}

#[test]
fn test_display_items_expanded_shows_all_rows() {
    let rows = make_rows();
    let items = dd::display_items(&rows, &set(&[3]), false, dd::COLLAPSE_THRESHOLD);
    assert_eq!(items.len(), 14);
    assert!(items.iter().all(|i| matches!(i, DisplayItem::Row(_))));
}

#[test]
fn test_display_items_show_all_context_rows() {
    let rows = make_rows();
    let items = dd::display_items(&rows, &set(&[]), true, dd::COLLAPSE_THRESHOLD);
    assert_eq!(items.len(), 14);
    assert!(items.iter().all(|i| matches!(i, DisplayItem::Row(_))));
}

#[test]
fn test_display_items_collapsed_group_has_first_row_id() {
    let rows = make_rows();
    let items = dd::display_items(&rows, &set(&[]), false, dd::COLLAPSE_THRESHOLD);
    let collapsed = items
        .iter()
        .find_map(|i| match i {
            DisplayItem::Collapsed(g) => Some(g.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(collapsed.id, 3);
    assert_eq!(collapsed.count, 7);
}

// MARK: - 検索ヒットの折りたたみ自動展開

#[test]
fn test_expanded_group_ids_containing_row_ids() {
    let rows = make_rows();
    assert_eq!(
        dd::expanded_group_ids(&[5], &rows, dd::COLLAPSE_THRESHOLD),
        set(&[3])
    );
    assert_eq!(
        dd::expanded_group_ids(&[3, 9], &rows, dd::COLLAPSE_THRESHOLD),
        set(&[3])
    );
    assert_eq!(
        dd::expanded_group_ids(&[0], &rows, dd::COLLAPSE_THRESHOLD),
        HashSet::new()
    );
    assert_eq!(
        dd::expanded_group_ids(&[], &rows, dd::COLLAPSE_THRESHOLD),
        HashSet::new()
    );
    assert_eq!(
        dd::expanded_group_ids(&[2, 10], &rows, dd::COLLAPSE_THRESHOLD),
        HashSet::new()
    );
}

#[test]
fn test_expanded_group_ids_across_multiple_groups() {
    let rows = make_rows();
    let ids: Vec<usize> = dd::fold_groups(&rows, 2).iter().map(|g| g.id).collect();
    assert_eq!(ids, vec![0, 3, 11]);
    assert_eq!(dd::expanded_group_ids(&[4, 12], &rows, 2), set(&[3, 11]));
    assert_eq!(dd::expanded_group_ids(&[1], &rows, 2), set(&[0]));
}

#[test]
fn test_expanded_group_ids_reveal_hit_rows_in_display_items() {
    let rows = make_rows();
    let hit_ids = dd::search_hit_row_ids(&rows, "g3", true);
    assert_eq!(hit_ids, vec![6]);
    let expanded = dd::expanded_group_ids(&hit_ids, &rows, dd::COLLAPSE_THRESHOLD);
    let items = dd::display_items(&rows, &expanded, false, dd::COLLAPSE_THRESHOLD);
    assert!(items
        .iter()
        .any(|i| matches!(i, DisplayItem::Row(r) if r.id == 6)));
    let collapsed_items = dd::display_items(&rows, &set(&[]), false, dd::COLLAPSE_THRESHOLD);
    assert!(!collapsed_items
        .iter()
        .any(|i| matches!(i, DisplayItem::Row(r) if r.id == 6)));
}

// MARK: - 差分行ナビゲーション

#[test]
fn test_difference_row_ids() {
    let rows = make_rows();
    assert_eq!(dd::difference_row_ids(&rows), vec![2, 10]);
}

#[test]
fn test_difference_row_ids_all_kinds() {
    let rows = vec![
        row(
            0,
            DiffRowKind::Context,
            Some(1),
            Some("c"),
            Some(1),
            Some("c"),
        ),
        row(1, DiffRowKind::Added, None, None, Some(2), Some("a")),
        row(2, DiffRowKind::Removed, Some(3), Some("r"), None, None),
        row(
            3,
            DiffRowKind::Modified,
            Some(4),
            Some("m"),
            Some(4),
            Some("m'"),
        ),
    ];
    assert_eq!(dd::difference_row_ids(&rows), vec![1, 2, 3]);
}

#[test]
fn test_difference_row_ids_empty() {
    assert_eq!(dd::difference_row_ids(&[]), Vec::<usize>::new());
}
