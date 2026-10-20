//! 比較履歴(Swift `RecentComparisonStore` 相当)。
//! `$XDG_CONFIG_HOME/diffloupe/history.json` に ISO8601 日時付き JSON で保存。
//! 最大 10 件、左右パス一致の同一ペアは先頭へ移動、壊れたデータは空履歴として無視。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::settings::config_dir;

pub const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentComparison {
    #[serde(rename = "leftPath")]
    pub left_path: String,
    #[serde(rename = "rightPath")]
    pub right_path: String,
    #[serde(rename = "leftGitRef", skip_serializing_if = "Option::is_none")]
    pub left_git_ref: Option<String>,
    #[serde(rename = "rightGitRef", skip_serializing_if = "Option::is_none")]
    pub right_git_ref: Option<String>,
    /// ISO8601 (UTC)
    pub date: String,
}

fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

fn file_name(p: &str) -> &str {
    p.rsplit('/').find(|s| !s.is_empty()).unwrap_or(p)
}

impl RecentComparison {
    /// 履歴メニューの表示形式: `左名[@ ref] ⇄ 右名[@ ref]`
    pub fn display_label(&self) -> String {
        let ref_suffix =
            |r: &Option<String>| r.as_ref().map(|r| format!(" @ {r}")).unwrap_or_default();
        format!(
            "{}{} ⇄ {}{}",
            file_name(&self.left_path),
            ref_suffix(&self.left_git_ref),
            file_name(&self.right_path),
            ref_suffix(&self.right_git_ref)
        )
    }

    /// 復元可否: 左右が実在するディレクトリか(git ref 比較の復元元は作業ツリー)。
    pub fn is_restorable(&self) -> bool {
        std::path::Path::new(&self.left_path).is_dir()
            && std::path::Path::new(&self.right_path).is_dir()
    }
}

pub fn load() -> Vec<RecentComparison> {
    let Ok(data) = std::fs::read(history_path()) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

pub fn record(entry: RecentComparison) {
    let mut list = load();
    // 左右パス一致(順序込み)の同一ペアは先頭へ移動
    list.retain(|e| !(e.left_path == entry.left_path && e.right_path == entry.right_path));
    list.insert(0, entry);
    list.truncate(MAX_RECENT);
    let path = history_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_vec(&list) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_label_format() {
        let r = RecentComparison {
            left_path: "/a/left".into(),
            right_path: "/b/right".into(),
            left_git_ref: Some("main".into()),
            right_git_ref: None,
            date: "2026-10-20T00:00:00Z".into(),
        };
        assert_eq!(r.display_label(), "left @ main ⇄ right");
    }
}
