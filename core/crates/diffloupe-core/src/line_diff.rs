//! 行単位差分(Swift `LineDiff` 相当)

use crate::myers::diff_offsets;
use crate::options::ContentComparison;
use crate::unicode::nfc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffRowKind {
    Context,
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffRow {
    pub id: usize,
    pub kind: DiffRowKind,
    pub left_number: Option<usize>,
    pub left_text: Option<String>,
    pub right_number: Option<usize>,
    pub right_text: Option<String>,
}

pub const MAX_TOTAL_LINES: usize = 40_000;

/// LF基準の行分割(CRLF→LF 正規化後に分割)
pub fn split_lines(text: &str) -> Vec<String> {
    let normalized: String = if text.contains("\r\n") {
        text.replace("\r\n", "\n")
    } else {
        text.to_string()
    };
    normalized.split('\n').map(|s| s.to_string()).collect()
}

/// 行末CRを保持した行分割(表示用)。CRLF は「行末CR+改行」に分解
pub fn split_lines_keeping_cr(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut prev_cr = false;
    for ch in text.chars() {
        if prev_cr {
            // 前の文字がCR。今の文字がLFなら行末CRとして残し改行確定
            prev_cr = false;
            if ch == '\n' {
                current.push('\r');
                lines.push(std::mem::take(&mut current));
                continue;
            }
        }
        if ch == '\r' {
            prev_cr = true;
            continue;
        }
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            continue;
        }
        current.push(ch);
    }
    if prev_cr {
        current.push('\r');
    }
    lines.push(current);
    lines
}

/// 横並び表示用の整列済み行リスト。上限超過時は None。
/// 差分計算には正規化行、表示には元テキストを使う。
pub fn rows(
    left_text: &str,
    right_text: &str,
    content_comparison: &ContentComparison,
) -> Option<Vec<DiffRow>> {
    let display_left = split_lines_keeping_cr(left_text);
    let display_right = split_lines_keeping_cr(right_text);
    if display_left.len() + display_right.len() > MAX_TOTAL_LINES {
        return None;
    }
    let (compare_left, compare_right) = if content_comparison.is_strict() {
        (split_lines(left_text), split_lines(right_text))
    } else {
        (
            content_comparison
                .normalized_text(left_text)
                .split('\n')
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            content_comparison
                .normalized_text(right_text)
                .split('\n')
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
    };
    if compare_left.len() > MAX_TOTAL_LINES
        || compare_right.len() > MAX_TOTAL_LINES
        || compare_left.len() != display_left.len()
        || compare_right.len() != display_right.len()
    {
        return None;
    }
    // Swift の差分は NFC 等価でない厳密な String 比較。grapheme 等価の
    // 揺れを吸収するため NFC 正規化後の行で比較する。
    let cl: Vec<String> = compare_left.iter().map(|s| nfc(s)).collect();
    let cr: Vec<String> = compare_right.iter().map(|s| nfc(s)).collect();
    let (removed, inserted) = diff_offsets(&cl, &cr);

    let mut out = Vec::with_capacity(display_left.len().max(display_right.len()));
    let (mut i, mut j) = (0usize, 0usize);
    macro_rules! push {
        ($kind:expr, $li:expr, $ri:expr) => {
            out.push(DiffRow {
                id: out.len(),
                kind: $kind,
                left_number: $li.map(|x: usize| x + 1),
                left_text: $li.map(|x: usize| display_left[x].clone()),
                right_number: $ri.map(|x: usize| x + 1),
                right_text: $ri.map(|x: usize| display_right[x].clone()),
            })
        };
    }
    while i < display_left.len() || j < display_right.len() {
        if i < display_left.len() && j < display_right.len() && !removed[i] && !inserted[j] {
            push!(DiffRowKind::Context, Some(i), Some(j));
            i += 1;
            j += 1;
            continue;
        }
        let mut removed_run = Vec::new();
        while i < display_left.len() && removed[i] {
            removed_run.push(i);
            i += 1;
        }
        let mut inserted_run = Vec::new();
        while j < display_right.len() && inserted[j] {
            inserted_run.push(j);
            j += 1;
        }
        if removed_run.is_empty() && inserted_run.is_empty() {
            if i < display_left.len() && j < display_right.len() {
                push!(DiffRowKind::Context, Some(i), Some(j));
                i += 1;
                j += 1;
            } else if i < display_left.len() {
                push!(DiffRowKind::Removed, Some(i), None);
                i += 1;
            } else if j < display_right.len() {
                push!(DiffRowKind::Added, None, Some(j));
                j += 1;
            }
            continue;
        }
        let paired = removed_run.len().min(inserted_run.len());
        for k in 0..paired {
            push!(
                DiffRowKind::Modified,
                Some(removed_run[k]),
                Some(inserted_run[k])
            );
        }
        for &li in &removed_run[paired..] {
            push!(DiffRowKind::Removed, Some(li), None);
        }
        for &ri in &inserted_run[paired..] {
            push!(DiffRowKind::Added, None, Some(ri));
        }
    }
    Some(out)
}
