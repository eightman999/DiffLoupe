//! 3-way 差分(Swift `ThreeWayDiff` 相当)。読み取り専用。

use crate::line_diff::{split_lines, split_lines_keeping_cr};
use crate::myers::diff_offsets;
use crate::options::ContentComparison;

pub const MAX_TOTAL_LINES: usize = 40_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreeWayDiffStatus {
    Unchanged,
    LeftChanged,
    RightChanged,
    BothChangedSame,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreeWayLine {
    pub id: usize,
    pub status: ThreeWayDiffStatus,
    pub base: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
    pub base_number: Option<usize>,
    pub left_number: Option<usize>,
    pub right_number: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreeWayDiff {
    pub lines: Vec<ThreeWayLine>,
    pub truncated: bool,
    pub unchanged_count: usize,
    pub left_changed_count: usize,
    pub right_changed_count: usize,
    pub both_changed_same_count: usize,
    pub conflict_count: usize,
}

impl ThreeWayDiff {
    pub fn total_changed_count(&self) -> usize {
        self.left_changed_count
            + self.right_changed_count
            + self.both_changed_same_count
            + self.conflict_count
    }

    fn truncated_result() -> Self {
        Self {
            lines: vec![],
            truncated: true,
            unchanged_count: 0,
            left_changed_count: 0,
            right_changed_count: 0,
            both_changed_same_count: 0,
            conflict_count: 0,
        }
    }

    pub fn make(base: &str, left: &str, right: &str, cc: &ContentComparison) -> ThreeWayDiff {
        let display_base = split_lines_keeping_cr(base);
        let display_left = split_lines_keeping_cr(left);
        let display_right = split_lines_keeping_cr(right);
        let total = display_base.len() + display_left.len() + display_right.len();
        if total > MAX_TOTAL_LINES
            || display_base.len() > MAX_TOTAL_LINES
            || display_left.len() > MAX_TOTAL_LINES
            || display_right.len() > MAX_TOTAL_LINES
        {
            return Self::truncated_result();
        }
        let norm = |t: &str| -> Vec<String> {
            if cc.is_strict() {
                let v = split_lines(t);
                if v.is_empty() {
                    vec![String::new()]
                } else {
                    v
                }
            } else {
                let n = cc.normalized_text(t);
                if n.is_empty() {
                    vec![String::new()]
                } else {
                    n.split('\n').map(|s| s.to_string()).collect()
                }
            }
        };
        let (compare_base, compare_left, compare_right) = (norm(base), norm(left), norm(right));
        if compare_base.len() != display_base.len()
            || compare_left.len() != display_left.len()
            || compare_right.len() != display_right.len()
        {
            return Self::truncated_result();
        }

        let (l_removed, l_inserted) = diff_offsets(&compare_base, &compare_left);
        let (r_removed, r_inserted) = diff_offsets(&compare_base, &compare_right);
        let left_units = build_units(
            compare_base.len(),
            compare_left.len(),
            &l_removed,
            &l_inserted,
        );
        let right_units = build_units(
            compare_base.len(),
            compare_right.len(),
            &r_removed,
            &r_inserted,
        );

        let lines = merge_units(
            &left_units,
            &right_units,
            &display_base,
            &compare_base,
            &display_left,
            &compare_left,
            &display_right,
            &compare_right,
        );
        let mut d = ThreeWayDiff {
            lines,
            truncated: false,
            unchanged_count: 0,
            left_changed_count: 0,
            right_changed_count: 0,
            both_changed_same_count: 0,
            conflict_count: 0,
        };
        for l in &d.lines {
            match l.status {
                ThreeWayDiffStatus::Unchanged => d.unchanged_count += 1,
                ThreeWayDiffStatus::LeftChanged => d.left_changed_count += 1,
                ThreeWayDiffStatus::RightChanged => d.right_changed_count += 1,
                ThreeWayDiffStatus::BothChangedSame => d.both_changed_same_count += 1,
                ThreeWayDiffStatus::Conflict => d.conflict_count += 1,
            }
        }
        d
    }
}

fn classify(
    base: Option<&String>,
    left: Option<&String>,
    right: Option<&String>,
) -> ThreeWayDiffStatus {
    match (base, left, right) {
        (None, Some(l), Some(r)) => {
            if l == r {
                ThreeWayDiffStatus::BothChangedSame
            } else {
                ThreeWayDiffStatus::Conflict
            }
        }
        (None, Some(_), None) => ThreeWayDiffStatus::LeftChanged,
        (None, None, Some(_)) => ThreeWayDiffStatus::RightChanged,
        (Some(b), Some(l), Some(r)) => {
            let ls = l == b;
            let rs = r == b;
            if ls && rs {
                ThreeWayDiffStatus::Unchanged
            } else if ls {
                ThreeWayDiffStatus::RightChanged
            } else if rs {
                ThreeWayDiffStatus::LeftChanged
            } else if l == r {
                ThreeWayDiffStatus::BothChangedSame
            } else {
                ThreeWayDiffStatus::Conflict
            }
        }
        (Some(b), Some(l), None) => {
            if l == b {
                ThreeWayDiffStatus::RightChanged
            } else {
                ThreeWayDiffStatus::Conflict
            }
        }
        (Some(b), None, Some(r)) => {
            if r == b {
                ThreeWayDiffStatus::LeftChanged
            } else {
                ThreeWayDiffStatus::Conflict
            }
        }
        (Some(_), None, None) => ThreeWayDiffStatus::BothChangedSame,
        (None, None, None) => ThreeWayDiffStatus::Unchanged,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SideUnit {
    Aligned(usize, usize),
    Removed(usize),
    Inserted(usize),
}

impl SideUnit {
    fn base_index(&self) -> Option<usize> {
        match self {
            Self::Aligned(b, _) | Self::Removed(b) => Some(*b),
            Self::Inserted(_) => None,
        }
    }
    fn side_index(&self) -> Option<usize> {
        match self {
            Self::Aligned(_, s) | Self::Inserted(s) => Some(*s),
            Self::Removed(_) => None,
        }
    }
}

fn build_units(
    base_len: usize,
    side_len: usize,
    removed: &[bool],
    inserted: &[bool],
) -> Vec<SideUnit> {
    let mut units = Vec::with_capacity(base_len + side_len);
    let (mut i, mut j) = (0usize, 0usize);
    while i < base_len || j < side_len {
        if i < base_len && j < side_len && !removed[i] && !inserted[j] {
            units.push(SideUnit::Aligned(i, j));
            i += 1;
            j += 1;
            continue;
        }
        let mut removed_run = Vec::new();
        while i < base_len && removed[i] {
            removed_run.push(i);
            i += 1;
        }
        let mut inserted_run = Vec::new();
        while j < side_len && inserted[j] {
            inserted_run.push(j);
            j += 1;
        }
        if removed_run.is_empty() && inserted_run.is_empty() {
            break;
        }
        let paired = removed_run.len().min(inserted_run.len());
        for k in 0..paired {
            units.push(SideUnit::Aligned(removed_run[k], inserted_run[k]));
        }
        for &b in &removed_run[paired..] {
            units.push(SideUnit::Removed(b));
        }
        for &s in &inserted_run[paired..] {
            units.push(SideUnit::Inserted(s));
        }
    }
    units
}

#[allow(clippy::too_many_arguments)]
fn merge_units(
    left_units: &[SideUnit],
    right_units: &[SideUnit],
    display_base: &[String],
    compare_base: &[String],
    display_left: &[String],
    compare_left: &[String],
    display_right: &[String],
    compare_right: &[String],
) -> Vec<ThreeWayLine> {
    let mut lines = Vec::new();
    let (mut li, mut ri) = (0usize, 0usize);
    while li < left_units.len() || ri < right_units.len() {
        let lu = left_units.get(li).copied();
        let ru = right_units.get(ri).copied();
        if let (Some(SideUnit::Inserted(ls)), Some(SideUnit::Inserted(rs))) = (lu, ru) {
            let status = classify(None, Some(&compare_left[ls]), Some(&compare_right[rs]));
            lines.push(ThreeWayLine {
                id: lines.len(),
                status,
                base: None,
                base_number: None,
                left: Some(display_left[ls].clone()),
                left_number: Some(ls + 1),
                right: Some(display_right[rs].clone()),
                right_number: Some(rs + 1),
            });
            li += 1;
            ri += 1;
            continue;
        }
        if let Some(SideUnit::Inserted(ls)) = lu {
            lines.push(ThreeWayLine {
                id: lines.len(),
                status: ThreeWayDiffStatus::LeftChanged,
                base: None,
                base_number: None,
                left: Some(display_left[ls].clone()),
                left_number: Some(ls + 1),
                right: None,
                right_number: None,
            });
            li += 1;
            continue;
        }
        if let Some(SideUnit::Inserted(rs)) = ru {
            lines.push(ThreeWayLine {
                id: lines.len(),
                status: ThreeWayDiffStatus::RightChanged,
                base: None,
                base_number: None,
                left: None,
                left_number: None,
                right: Some(display_right[rs].clone()),
                right_number: Some(rs + 1),
            });
            ri += 1;
            continue;
        }
        let (Some(lu2), Some(ru2)) = (lu, ru) else {
            break;
        };
        if lu2.base_index() != ru2.base_index() {
            break;
        }
        let b = lu2.base_index().unwrap();
        let ls = lu2.side_index();
        let rs = ru2.side_index();
        let status = classify(
            Some(&compare_base[b]),
            ls.map(|s| &compare_left[s]),
            rs.map(|s| &compare_right[s]),
        );
        lines.push(ThreeWayLine {
            id: lines.len(),
            status,
            base: Some(display_base[b].clone()),
            base_number: Some(b + 1),
            left: ls.map(|s| display_left[s].clone()),
            left_number: ls.map(|s| s + 1),
            right: rs.map(|s| display_right[s].clone()),
            right_number: rs.map(|s| s + 1),
        });
        li += 1;
        ri += 1;
    }
    lines
}
