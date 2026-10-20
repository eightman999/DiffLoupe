//! 比較履歴(Swift `RecentComparison` / `RecentComparisonStore` 相当)。
//! 設定 JSON(`%APPDATA%\DiffLoupe\settings.json`)内に保存する。
//! 最大10件・先頭が最新・同一ペアは先頭へ移動・壊れたデータは空扱い。

use serde::{Deserialize, Serialize};

pub const MAX_RECENT: usize = 10;

/// 実行した1回の比較の履歴。`date` は UNIX epoch 秒(f64)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentComparison {
    pub left_path: String,
    pub right_path: String,
    #[serde(default)]
    pub left_git_ref: Option<String>,
    #[serde(default)]
    pub right_git_ref: Option<String>,
    /// ISO8601 文字列(UTC)。読めない場合は epoch 秒として解釈を試みる。
    #[serde(default)]
    pub date: String,
}

impl RecentComparison {
    /// 同一ペア判定用の識別子(左右パスを順序込みで結合)
    pub fn id(&self) -> String {
        format!("{}|{}", self.left_path, self.right_path)
    }
}

/// 履歴一覧へ記録する純粋関数。同一ペアを除いて先頭へ挿入し、最大件数に切り詰める。
/// 保存後の一覧を返す。
pub fn record(list: &[RecentComparison], item: RecentComparison) -> Vec<RecentComparison> {
    let mut out: Vec<RecentComparison> = list
        .iter()
        .filter(|c| c.left_path != item.left_path || c.right_path != item.right_path)
        .cloned()
        .collect();
    out.insert(0, item);
    out.truncate(MAX_RECENT);
    out
}

/// 現在時刻を ISO8601(UTC)文字列で返す(履歴の日時用)。
pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    diffloupe_core::report::iso8601(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(l: &str, r: &str, date: &str) -> RecentComparison {
        RecentComparison {
            left_path: l.into(),
            right_path: r.into(),
            left_git_ref: None,
            right_git_ref: None,
            date: date.into(),
        }
    }

    #[test]
    fn record_inserts_at_front() {
        let list = vec![item("/a", "/b", "2026-01-01T00:00:00Z")];
        let out = record(&list, item("/c", "/d", "2026-01-02T00:00:00Z"));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].left_path, "/c");
        assert_eq!(out[1].left_path, "/a");
    }

    #[test]
    fn record_moves_same_pair_to_front() {
        let list = vec![item("/a", "/b", "1"), item("/c", "/d", "2")];
        let out = record(&list, item("/c", "/d", "3"));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id(), "/c|/d");
        assert_eq!(out[0].date, "3");
    }

    #[test]
    fn record_truncates_to_max() {
        let mut list = Vec::new();
        for i in 0..MAX_RECENT {
            list.push(item(&format!("/l{}", i), "/r", "x"));
        }
        let out = record(&list, item("/new", "/r", "x"));
        assert_eq!(out.len(), MAX_RECENT);
        assert_eq!(out[0].left_path, "/new");
        assert!(out.iter().all(|c| c.left_path != "/l9"));
    }

    #[test]
    fn swapped_order_is_different_pair() {
        let list = vec![item("/a", "/b", "1")];
        let out = record(&list, item("/b", "/a", "2"));
        assert_eq!(out.len(), 2);
    }
}
