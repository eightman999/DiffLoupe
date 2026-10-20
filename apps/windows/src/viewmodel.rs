//! 結果ツリー表示用ビューモデル(Swift `AppModel.ResultFilter` / `filterNode` / `StatusBadge` 相当)

use diffloupe_core::models::{ComparisonResult, ComparisonSummary, DiffNode, DiffStatus};

/// 表示絞り込み(Swift `AppModel.ResultFilter` 相当)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultFilter {
    All,
    Differences,
    Different,
    LeftOnly,
    RightOnly,
    Error,
    Identical,
}

impl ResultFilter {
    pub const ALL: [ResultFilter; 7] = [
        Self::All,
        Self::Differences,
        Self::Different,
        Self::LeftOnly,
        Self::RightOnly,
        Self::Error,
        Self::Identical,
    ];

    pub fn display_name(&self) -> &'static str {
        use crate::strings::*;
        match self {
            Self::All => FILTER_ALL,
            Self::Differences => FILTER_DIFFERENCES,
            Self::Different => FILTER_DIFFERENT,
            Self::LeftOnly => FILTER_LEFT_ONLY,
            Self::RightOnly => FILTER_RIGHT_ONLY,
            Self::Error => FILTER_ERROR,
            Self::Identical => FILTER_IDENTICAL,
        }
    }

    pub fn matches(&self, status: DiffStatus) -> bool {
        match self {
            Self::All => true,
            Self::Differences => status != DiffStatus::Identical,
            Self::Different => status == DiffStatus::Different,
            Self::LeftOnly => status == DiffStatus::LeftOnly,
            Self::RightOnly => status == DiffStatus::RightOnly,
            Self::Error => status == DiffStatus::Error,
            Self::Identical => status == DiffStatus::Identical,
        }
    }
}

/// ステータスバッジの表示文字(Swift `StatusBadge.label` 相当)
pub fn status_badge_label(status: DiffStatus, is_directory: bool) -> &'static str {
    use crate::strings::*;
    match status {
        DiffStatus::Identical => BADGE_IDENTICAL,
        DiffStatus::Different => {
            if is_directory {
                BADGE_DIR_HAS_DIFF
            } else {
                BADGE_DIFFERENT
            }
        }
        DiffStatus::LeftOnly => BADGE_LEFT_ONLY,
        DiffStatus::RightOnly => BADGE_RIGHT_ONLY,
        DiffStatus::Error => BADGE_ERROR,
    }
}

/// 状態×検索で絞り込んだツリー(絞り込みは result を変更しない。
/// ディレクトリは一致する子孫を表示するための祖先として残す)。
/// Swift `AppModel.filterNode` 相当。
pub fn filter_node(node: &DiffNode, filter: ResultFilter, query: &str) -> Option<DiffNode> {
    let matches_query = query.is_empty() || node.id.to_lowercase().contains(&query.to_lowercase());
    if !node.is_directory {
        return if matches_query && filter.matches(node.status) {
            Some(node.clone())
        } else {
            None
        };
    }
    let original = node.children.clone().unwrap_or_default();
    let filtered: Vec<DiffNode> = original
        .iter()
        .filter_map(|c| filter_node(c, filter, query))
        .collect();
    let is_empty_dir = original.is_empty();
    if !filtered.is_empty() || (is_empty_dir && matches_query && filter.matches(node.status)) {
        let mut n = node.clone();
        n.children = if node.children.is_none() {
            None
        } else {
            Some(filtered)
        };
        Some(n)
    } else {
        None
    }
}

/// 表示ノード一覧(Swift `AppModel.displayNodes` 相当)
pub fn display_nodes(
    result: Option<&ComparisonResult>,
    filter: ResultFilter,
    search_text: &str,
) -> Vec<DiffNode> {
    let Some(result) = result else {
        return Vec::new();
    };
    let query = search_text.trim();
    result
        .root_node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|n| filter_node(n, filter, query))
        .collect()
}

/// ツリーが空のときのメッセージ(Swift `DiffTreeView.emptyMessage` 相当)
pub fn empty_tree_message(
    has_result: bool,
    search_text: &str,
    filter: ResultFilter,
) -> &'static str {
    use crate::strings::*;
    if !has_result {
        return EMPTY_RESULT;
    }
    let has_search = !search_text.trim().is_empty();
    if !has_search && filter == ResultFilter::Differences {
        return EMPTY_NO_DIFF;
    }
    if has_search || filter != ResultFilter::All {
        return EMPTY_FILTERED;
    }
    EMPTY_NO_FILES
}

/// ステータスバーのサマリ表示要素(Swift `StatusBar` 相当)
pub fn summary_labels(summary: &ComparisonSummary) -> Vec<(&'static str, usize)> {
    use crate::strings::*;
    let mut v = vec![
        (BADGE_IDENTICAL, summary.identical_files),
        (BADGE_DIFFERENT, summary.different_files),
        (BADGE_LEFT_ONLY, summary.left_only_files),
        (BADGE_RIGHT_ONLY, summary.right_only_files),
    ];
    if summary.error_files > 0 {
        v.push((BADGE_ERROR, summary.error_files));
    }
    v
}

/// バイト数の人間向け表示(ByteCountFormatter(.file) 相当の簡易版)
pub fn byte_count_text(bytes: i64) -> String {
    if bytes < 0 {
        return "—".into();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} B", bytes)
    } else if v >= 100.0 {
        format!("{:.0} {}", v, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffloupe_core::models::DiffNode;

    fn node(
        id: &str,
        is_dir: bool,
        status: DiffStatus,
        children: Option<Vec<DiffNode>>,
    ) -> DiffNode {
        DiffNode {
            id: id.into(),
            name: id.rsplit('/').next().unwrap_or(id).into(),
            is_directory: is_dir,
            status,
            children,
        }
    }

    #[test]
    fn filter_matches_status() {
        assert!(ResultFilter::Differences.matches(DiffStatus::Different));
        assert!(ResultFilter::Differences.matches(DiffStatus::LeftOnly));
        assert!(!ResultFilter::Differences.matches(DiffStatus::Identical));
        assert!(ResultFilter::All.matches(DiffStatus::Identical));
    }

    #[test]
    fn filter_node_keeps_ancestor_of_match() {
        let tree = node(
            "dir",
            true,
            DiffStatus::Different,
            Some(vec![
                node("dir/a.txt", false, DiffStatus::Different, None),
                node("dir/b.txt", false, DiffStatus::Identical, None),
            ]),
        );
        let out = filter_node(&tree, ResultFilter::Different, "").unwrap();
        let kids = out.children.unwrap();
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].id, "dir/a.txt");
    }

    #[test]
    fn filter_node_search_and_status_are_and() {
        let f = node("foo/b.txt", false, DiffStatus::Different, None);
        assert!(filter_node(&f, ResultFilter::Different, "b.txt").is_some());
        assert!(filter_node(&f, ResultFilter::Different, "zzz").is_none());
        assert!(filter_node(&f, ResultFilter::Identical, "b.txt").is_none());
        // 大文字小文字は区別しない
        assert!(filter_node(&f, ResultFilter::Different, "B.TXT").is_some());
    }

    #[test]
    fn empty_directory_uses_own_status_and_query() {
        let dir = node("empty", true, DiffStatus::LeftOnly, Some(vec![]));
        assert!(filter_node(&dir, ResultFilter::LeftOnly, "").is_some());
        assert!(filter_node(&dir, ResultFilter::RightOnly, "").is_none());
    }

    #[test]
    fn badge_label_directory_diff() {
        assert_eq!(status_badge_label(DiffStatus::Different, true), "差分あり");
        assert_eq!(status_badge_label(DiffStatus::Different, false), "相違");
    }

    #[test]
    fn byte_count_format() {
        assert_eq!(byte_count_text(-1), "—");
        assert_eq!(byte_count_text(512), "512 B");
        assert_eq!(byte_count_text(2048), "2.0 KB");
        assert_eq!(byte_count_text(1_500_000), "1.5 MB");
    }
}
