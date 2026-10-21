//! 差分ビューの純粋ロジック(Swift `DiffDisplay` 相当)

use crate::line_diff::{DiffRow, DiffRowKind};
use crate::unicode::lowercased;
use std::collections::HashSet;

pub const COLLAPSE_THRESHOLD: usize = 5;

/// クエリを含む行ID(左右どちらかの元テキストが対象)
pub fn search_hit_row_ids(rows: &[DiffRow], query: &str, case_sensitive: bool) -> Vec<usize> {
    if query.is_empty() {
        return vec![];
    }
    let q_lower = lowercased(query);
    rows.iter()
        .filter(|row| {
            let left = row.left_text.as_deref().unwrap_or("");
            let right = row.right_text.as_deref().unwrap_or("");
            if case_sensitive {
                left.contains(query) || right.contains(query)
            } else {
                lowercased(left).contains(&q_lower) || lowercased(right).contains(&q_lower)
            }
        })
        .map(|r| r.id)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldGroup {
    pub id: usize,
    pub row_ids: Vec<usize>,
    pub count: usize,
}

/// 連続するコンテキスト行で threshold 以上のものを折りたたみグループに
pub fn fold_groups(rows: &[DiffRow], threshold: usize) -> Vec<FoldGroup> {
    let mut groups = Vec::new();
    let mut run_start: Option<usize> = None;
    for (idx, row) in rows.iter().enumerate() {
        if row.kind == DiffRowKind::Context {
            if run_start.is_none() {
                run_start = Some(idx);
            }
        } else if let Some(s) = run_start.take() {
            let count = idx - s;
            if count >= threshold {
                groups.push(FoldGroup {
                    id: rows[s].id,
                    row_ids: rows[s..idx].iter().map(|r| r.id).collect(),
                    count,
                });
            }
        }
    }
    if let Some(s) = run_start {
        let count = rows.len() - s;
        if count >= threshold {
            groups.push(FoldGroup {
                id: rows[s].id,
                row_ids: rows[s..].iter().map(|r| r.id).collect(),
                count,
            });
        }
    }
    groups
}

/// 指定行IDを含む折りたたみグループIDの集合
pub fn expanded_group_ids(row_ids: &[usize], rows: &[DiffRow], threshold: usize) -> HashSet<usize> {
    if row_ids.is_empty() {
        return HashSet::new();
    }
    let target: HashSet<usize> = row_ids.iter().copied().collect();
    fold_groups(rows, threshold)
        .into_iter()
        .filter(|g| g.row_ids.iter().any(|id| target.contains(id)))
        .map(|g| g.id)
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItem {
    Row(DiffRow),
    Collapsed(FoldGroup),
}

impl DisplayItem {
    pub fn id(&self) -> usize {
        match self {
            DisplayItem::Row(r) => r.id,
            DisplayItem::Collapsed(g) => g.id,
        }
    }
}

/// 折りたたみ適用後の表示アイテム列
pub fn display_items(
    rows: &[DiffRow],
    expanded: &HashSet<usize>,
    show_all_context_rows: bool,
    threshold: usize,
) -> Vec<DisplayItem> {
    if show_all_context_rows {
        return rows.iter().cloned().map(DisplayItem::Row).collect();
    }
    let groups = fold_groups(rows, threshold);
    let mut by_first: std::collections::HashMap<usize, &FoldGroup> =
        std::collections::HashMap::new();
    for g in &groups {
        if let Some(&first) = g.row_ids.first() {
            by_first.insert(first, g);
        }
    }
    let mut items = Vec::with_capacity(rows.len());
    let mut index = 0usize;
    while index < rows.len() {
        let row = &rows[index];
        if row.kind == DiffRowKind::Context {
            if let Some(g) = by_first.get(&row.id) {
                if expanded.contains(&g.id) {
                    for _ in &g.row_ids {
                        items.push(DisplayItem::Row(rows[index].clone()));
                        index += 1;
                    }
                } else {
                    items.push(DisplayItem::Collapsed((*g).clone()));
                    index += g.count;
                }
                continue;
            }
        }
        items.push(DisplayItem::Row(row.clone()));
        index += 1;
    }
    items
}

/// added/removed/modified の行IDリスト(行順)
pub fn difference_row_ids(rows: &[DiffRow]) -> Vec<usize> {
    rows.iter()
        .filter(|r| r.kind != DiffRowKind::Context)
        .map(|r| r.id)
        .collect()
}
