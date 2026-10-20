//! 行内差分(Swift `InlineDiff` 相当): LCS 動的計画法による文字単位変更範囲

use crate::line_diff::{DiffRow, DiffRowKind};
use crate::unicode::{graphemes, lowercased};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineHighlight {
    pub left_ranges: Vec<Range<usize>>,
    pub right_ranges: Vec<Range<usize>>,
}

pub const MAX_CHARACTERS: usize = 1200;

pub fn highlight(
    left: Option<&str>,
    right: Option<&str>,
    case_insensitive: bool,
) -> Option<InlineHighlight> {
    let (left, right) = (left?, right?);
    if left.is_empty() && right.is_empty() {
        return None;
    }
    // graphemes("") は行分割用に [""] を返すため、文字列としての空は空ベクタに
    // 正規化する(Swift `[Character]` 相当。幻影文字が削除範囲になるのを防ぐ)
    let left_chars: Vec<String> = if left.is_empty() {
        Vec::new()
    } else {
        graphemes(left).iter().map(|s| s.to_string()).collect()
    };
    let right_chars: Vec<String> = if right.is_empty() {
        Vec::new()
    } else {
        graphemes(right).iter().map(|s| s.to_string()).collect()
    };
    if left_chars.len() + right_chars.len() > MAX_CHARACTERS {
        return None;
    }
    if left_chars == right_chars {
        return None;
    }
    let (left_ranges, right_ranges) = changed_ranges(&left_chars, &right_chars, case_insensitive);
    if left_ranges.is_empty() && right_ranges.is_empty() {
        return None;
    }
    Some(InlineHighlight {
        left_ranges,
        right_ranges,
    })
}

pub fn highlights_for_rows(
    rows: &[DiffRow],
    case_insensitive: bool,
) -> std::collections::HashMap<usize, InlineHighlight> {
    let mut result = std::collections::HashMap::new();
    for row in rows {
        if row.kind != DiffRowKind::Modified {
            continue;
        }
        if let Some(h) = highlight(
            row.left_text.as_deref(),
            row.right_text.as_deref(),
            case_insensitive,
        ) {
            result.insert(row.id, h);
        }
    }
    result
}

fn chars_equal(a: &str, b: &str, ci: bool) -> bool {
    if !ci {
        a == b
    } else {
        lowercased(a) == lowercased(b)
    }
}

#[allow(clippy::single_range_in_vec_init)]
fn changed_ranges(
    left: &[String],
    right: &[String],
    ci: bool,
) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let n = left.len();
    let m = right.len();
    if n == 0 && m == 0 {
        return (vec![], vec![]);
    }
    if n == 0 {
        return (vec![], vec![0..m]);
    }
    if m == 0 {
        return (vec![0..n], vec![]);
    }
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if chars_equal(&left[i], &right[j], ci) {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut left_ranges = Vec::new();
    let mut right_ranges = Vec::new();
    let mut left_start: Option<usize> = None;
    let mut right_start: Option<usize> = None;
    let mut close_left = |end: usize, start: &mut Option<usize>| {
        if let Some(s) = start.take() {
            if s < end {
                left_ranges.push(s..end);
            }
        }
    };
    let mut close_right = |end: usize, start: &mut Option<usize>| {
        if let Some(s) = start.take() {
            if s < end {
                right_ranges.push(s..end);
            }
        }
    };
    let (mut i, mut j) = (0usize, 0usize);
    while i < n || j < m {
        if i < n && j < m && chars_equal(&left[i], &right[j], ci) {
            close_left(i, &mut left_start);
            close_right(j, &mut right_start);
            i += 1;
            j += 1;
        } else if j < m && (i >= n || dp[i][j + 1] >= dp[i + 1][j]) {
            if right_start.is_none() {
                right_start = Some(j);
            }
            j += 1;
        } else {
            if left_start.is_none() {
                left_start = Some(i);
            }
            i += 1;
        }
    }
    close_left(n, &mut left_start);
    close_right(m, &mut right_start);
    (left_ranges, right_ranges)
}
