//! カスタム描画グリッドのモデル層。
//! テキスト差分(2列)・Hex(5列)・3-way(3列)・コピー計画(表)を
//! 同じ描画部品で扱うための UI 非依存モデル。ホスト OS でテスト可能。

use diffloupe_core::diff_display::DisplayItem;
use diffloupe_core::hex_diff::HexComparison;
use diffloupe_core::inline_diff::{self, InlineHighlight};
use diffloupe_core::line_diff::{DiffRow, DiffRowKind};
use diffloupe_core::three_way::{ThreeWayDiff, ThreeWayDiffStatus};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// セルの配色種別(実際の色は UI 層のテーマが決める)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellTone {
    Normal,
    Added,
    Removed,
    Modified,
    /// 折りたたみヘッダ・情報行
    Info,
}

/// テキスト1セル。`gutter` はセル左端の行番号相当領域に右寄せで表示する文字列。
#[derive(Debug, Clone, PartialEq)]
pub struct GridCell {
    pub text: String,
    pub gutter: Option<String>,
    pub tone: CellTone,
    /// 強調表示する UTF-16 コードユニット範囲
    pub highlights: Vec<Range<usize>>,
}

impl GridCell {
    pub fn new(text: impl Into<String>, gutter: Option<String>, tone: CellTone) -> Self {
        Self {
            text: text.into(),
            gutter,
            tone,
            highlights: Vec::new(),
        }
    }
    pub fn blank(tone: CellTone) -> Self {
        Self::new("", None, tone)
    }
}

/// グリッドの1行
#[derive(Debug, Clone, PartialEq)]
pub enum GridItem {
    /// 通常行。cells.len() は列数と一致する。
    Row {
        cells: Vec<GridCell>,
        /// 差分行ID(ナビゲーション・検索ヒット判定用)
        source_id: usize,
    },
    /// 折りたたまれたコンテキストのヘッダ行(クリックで展開)
    FoldHeader { group_id: usize, count: usize },
}

/// 列定義。weight はテキスト列の幅の配分率。
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSpec {
    pub title: String,
    /// 0.0 なら内容に合わせた固定列、正なら残り幅を比率で分割
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct GridModel {
    pub columns: Vec<ColumnSpec>,
    pub items: Vec<GridItem>,
}

impl GridModel {
    /// 差分行(セルに tone が付く)の source_id 一覧。検索・ナビ用。
    pub fn row_item_index_by_source(&self, source_id: usize) -> Option<usize> {
        self.items.iter().position(|i| match i {
            GridItem::Row { source_id: id, .. } => *id == source_id,
            _ => false,
        })
    }
}

fn row_to_item(row: &DiffRow, highlights: Option<&InlineHighlight>) -> GridItem {
    let tone = match row.kind {
        DiffRowKind::Context => CellTone::Normal,
        DiffRowKind::Added => CellTone::Added,
        DiffRowKind::Removed => CellTone::Removed,
        DiffRowKind::Modified => CellTone::Modified,
    };
    let gutter = |n: Option<usize>| n.map(|v| v.to_string());
    let mut left = GridCell::new(
        row.left_text.clone().unwrap_or_default(),
        gutter(row.left_number),
        if row.left_text.is_some() {
            tone
        } else {
            CellTone::Normal
        },
    );
    let mut right = GridCell::new(
        row.right_text.clone().unwrap_or_default(),
        gutter(row.right_number),
        if row.right_text.is_some() {
            tone
        } else {
            CellTone::Normal
        },
    );
    if let Some(h) = highlights {
        if let Some(t) = row.left_text.as_deref() {
            left.highlights = inline_diff::grapheme_ranges_to_utf16(t, &h.left_ranges);
        }
        if let Some(t) = row.right_text.as_deref() {
            right.highlights = inline_diff::grapheme_ranges_to_utf16(t, &h.right_ranges);
        }
    }
    GridItem::Row {
        cells: vec![left, right],
        source_id: row.id,
    }
}

/// テキスト差分の表示アイテム列 → グリッド行。
/// `display_items` は `DiffNav::display_items` の結果。
pub fn build_diff_items(
    display_items: &[DisplayItem],
    highlights: &HashMap<usize, InlineHighlight>,
) -> Vec<GridItem> {
    display_items
        .iter()
        .map(|item| match item {
            DisplayItem::Row(row) => row_to_item(row, highlights.get(&row.id)),
            DisplayItem::Collapsed(g) => GridItem::FoldHeader {
                group_id: g.id,
                count: g.count,
            },
        })
        .collect()
}

/// 差分グリッドの列定義(左番号+左本文 / 右番号+右本文 はセル内ガターとして描画)
pub fn diff_columns(left_title: &str, right_title: &str) -> Vec<ColumnSpec> {
    vec![
        ColumnSpec {
            title: left_title.into(),
            weight: 1.0,
        },
        ColumnSpec {
            title: right_title.into(),
            weight: 1.0,
        },
    ]
}

/// Hex 比較 → グリッド行(5列: オフセット / 左Hex / 左ASCII / 右Hex / 右ASCII)。
/// 差分バイトは highlights(UTF-16範囲)で示す。
pub fn build_hex_items(comp: &HexComparison) -> Vec<GridItem> {
    let mut items = Vec::with_capacity(comp.row_count());
    for i in 0..comp.row_count() {
        let row = comp.row(i);
        let offset = format!("{:08X}", row.offset);
        let hex = |bytes: &Option<Vec<u8>>| -> String {
            match bytes {
                None => " ".repeat(16 * 3 - 1),
                Some(b) => b
                    .iter()
                    .map(|v| format!("{:02X}", v))
                    .collect::<Vec<_>>()
                    .join(" "),
            }
        };
        let ascii = |s: &Option<String>| s.clone().unwrap_or_else(|| " ".repeat(16));
        let mut left_hex = GridCell::new(hex(&row.left_bytes), None, CellTone::Normal);
        let mut right_hex = GridCell::new(hex(&row.right_bytes), None, CellTone::Normal);
        let mut left_ascii = GridCell::new(ascii(&row.left_ascii), None, CellTone::Normal);
        let mut right_ascii = GridCell::new(ascii(&row.right_ascii), None, CellTone::Normal);
        // Hex 文字列は "xx " の繰り返しなので、バイト i の範囲は 3i..3i+2
        for &b in &row.differing_byte_offsets {
            left_hex.highlights.push(b * 3..b * 3 + 2);
            right_hex.highlights.push(b * 3..b * 3 + 2);
            left_ascii.highlights.push(b..b + 1);
            right_ascii.highlights.push(b..b + 1);
        }
        items.push(GridItem::Row {
            cells: vec![
                GridCell::new(offset, None, CellTone::Info),
                left_hex,
                left_ascii,
                right_hex,
                right_ascii,
            ],
            source_id: i,
        });
    }
    items
}

pub fn hex_columns() -> Vec<ColumnSpec> {
    use crate::strings::*;
    [
        HEX_OFFSET,
        HEX_LEFT,
        HEX_LEFT_ASCII,
        HEX_RIGHT,
        HEX_RIGHT_ASCII,
    ]
    .iter()
    .map(|t| ColumnSpec {
        title: t.to_string(),
        weight: 0.0,
    })
    .collect()
}

/// 3-way 差分 → グリッド行(3列: ベース / 左 / 右)
pub fn build_threeway_items(diff: &ThreeWayDiff) -> Vec<GridItem> {
    diff.lines
        .iter()
        .map(|l| {
            let tone = |side_present: bool, changed: bool| {
                if !side_present {
                    CellTone::Normal
                } else if changed {
                    CellTone::Modified
                } else {
                    CellTone::Normal
                }
            };
            let (l_changed, r_changed, both_same) = match l.status {
                ThreeWayDiffStatus::Unchanged => (false, false, false),
                ThreeWayDiffStatus::LeftChanged => (true, false, false),
                ThreeWayDiffStatus::RightChanged => (false, true, false),
                ThreeWayDiffStatus::BothChangedSame => (true, true, true),
                ThreeWayDiffStatus::Conflict => (true, true, false),
            };
            let _ = both_same; // 同値変更は両側とも Modified 扱い(色で区別はしない)
            let gutter = |n: Option<usize>| n.map(|v| v.to_string());
            GridItem::Row {
                cells: vec![
                    GridCell::new(
                        l.base.clone().unwrap_or_default(),
                        gutter(l.base_number),
                        CellTone::Normal,
                    ),
                    GridCell::new(
                        l.left.clone().unwrap_or_default(),
                        gutter(l.left_number),
                        tone(l.left.is_some(), l_changed),
                    ),
                    GridCell::new(
                        l.right.clone().unwrap_or_default(),
                        gutter(l.right_number),
                        tone(l.right.is_some(), r_changed),
                    ),
                ],
                source_id: l.id,
            }
        })
        .collect()
}

pub fn threeway_columns() -> Vec<ColumnSpec> {
    use crate::strings::*;
    [COL_BASE, COL_LEFT, COL_RIGHT]
        .iter()
        .map(|t| ColumnSpec {
            title: t.to_string(),
            weight: 1.0,
        })
        .collect()
}

/// 差分ソース行ID集合(ナビ・検索ハイライト用)
pub fn marked_row_ids(items: &[GridItem]) -> HashSet<usize> {
    items
        .iter()
        .filter_map(|i| match i {
            GridItem::Row { source_id, .. } => Some(*source_id),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffloupe_core::diff_display as dd;
    use diffloupe_core::options::ContentComparison;
    use diffloupe_core::{hex_diff, line_diff, three_way::ThreeWayDiff};

    #[test]
    fn diff_items_include_fold_header() {
        let l = "d\n1\n2\n3\n4\n5\n6\n7";
        let r = "x\n1\n2\n3\n4\n5\n6\n7";
        let rows = line_diff::rows(l, r, &ContentComparison::default()).unwrap();
        let items = dd::display_items(&rows, &HashSet::new(), false, dd::COLLAPSE_THRESHOLD);
        let grid = build_diff_items(&items, &HashMap::new());
        assert!(grid
            .iter()
            .any(|i| matches!(i, GridItem::FoldHeader { count: 7, .. })));
    }

    #[test]
    fn diff_item_tones() {
        let rows = line_diff::rows("a\nb", "a\nc", &ContentComparison::default()).unwrap();
        let items = dd::display_items(&rows, &HashSet::new(), true, dd::COLLAPSE_THRESHOLD);
        let grid = build_diff_items(&items, &HashMap::new());
        match &grid[1] {
            GridItem::Row { cells, .. } => {
                assert_eq!(cells[0].tone, CellTone::Modified);
                assert_eq!(cells[1].tone, CellTone::Modified);
            }
            _ => panic!("expected row"),
        }
    }

    #[test]
    fn hex_items_have_byte_highlights() {
        let comp = hex_diff::compare(&[0u8, 1, 2], &[0u8, 9, 2], 16);
        let items = build_hex_items(&comp);
        match &items[0] {
            GridItem::Row { cells, .. } => {
                assert_eq!(cells[1].highlights, vec![3..5]); // バイト1 → "01 "→ 範囲 3..5
                assert_eq!(cells[2].highlights, vec![1..2]); // ASCII 列
            }
            _ => panic!(),
        }
    }

    #[test]
    fn threeway_items_color_changed_sides() {
        let d = ThreeWayDiff::make("a\nb", "a\nL", "a\nR", &ContentComparison::default());
        let items = build_threeway_items(&d);
        // 行1: ベース b、左 L、右 R → conflict
        match &items[1] {
            GridItem::Row { cells, .. } => {
                assert_eq!(cells[1].tone, CellTone::Modified);
                assert_eq!(cells[2].tone, CellTone::Modified);
            }
            _ => panic!(),
        }
    }
}
