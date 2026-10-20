//! コピー計画プレビュー(Swift `SyncPlan` 相当)。読み取り専用。

use crate::models::{ComparisonResult, DiffStatus};
use crate::paths::append_component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDirection {
    LeftToRight,
    RightToLeft,
}

impl SyncDirection {
    fn source_is_left(&self) -> bool {
        matches!(self, SyncDirection::LeftToRight)
    }
    pub fn raw_value(&self) -> &'static str {
        match self {
            Self::LeftToRight => "leftToRight",
            Self::RightToLeft => "rightToLeft",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncAction {
    CopyFile,
    CreateDirectory,
    Conflict,
    PreservedDestination,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncItem {
    pub relative_path: String,
    pub action: SyncAction,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub bytes: i64,
    pub reason: String,
    /// Swift `direction` 相当(id 生成のため方向を保持)
    pub direction_key: &'static str,
}

impl SyncItem {
    pub fn id(&self) -> String {
        format!("{}:{}", self.direction_key, self.relative_path)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub copy_files: usize,
    pub create_directories: usize,
    pub conflicts: usize,
    pub preserved_destinations: usize,
    pub blocked: usize,
    pub total_bytes: i64,
}

impl SyncSummary {
    pub fn total_items(&self) -> usize {
        self.copy_files
            + self.create_directories
            + self.conflicts
            + self.preserved_destinations
            + self.blocked
    }
}

#[derive(Debug, Clone)]
pub struct SyncPlan {
    pub direction: SyncDirection,
    pub source_root: String,
    pub destination_root: String,
    pub items: Vec<SyncItem>,
    pub summary: SyncSummary,
}

impl SyncPlan {
    /// ComparisonResult からコピー計画を生成する純粋関数
    pub fn make(result: &ComparisonResult, direction: SyncDirection) -> SyncPlan {
        let source_is_left = direction.source_is_left();
        let (source_root, destination_root) = if source_is_left {
            (result.left_root.clone(), result.right_root.clone())
        } else {
            (result.right_root.clone(), result.left_root.clone())
        };
        let mut items = Vec::new();
        let mut summary = SyncSummary::default();
        let dk = direction.raw_value();
        for entry in &result.entries {
            match entry.status {
                DiffStatus::Identical => continue,
                DiffStatus::LeftOnly | DiffStatus::RightOnly => {
                    let exists_on_source = (entry.status == DiffStatus::LeftOnly && source_is_left)
                        || (entry.status == DiffStatus::RightOnly && !source_is_left);
                    if exists_on_source {
                        let source = if source_is_left {
                            entry.left_path.as_ref()
                        } else {
                            entry.right_path.as_ref()
                        }
                        .map(|p| p.to_string_lossy().into_owned());
                        if entry.is_directory {
                            items.push(SyncItem {
                                relative_path: entry.relative_path.clone(),
                                action: SyncAction::CreateDirectory,
                                source,
                                destination: Some(append_component(
                                    &destination_root,
                                    &entry.relative_path,
                                )),
                                bytes: 0,
                                reason: "ディレクトリの作成が必要".into(),
                                direction_key: dk,
                            });
                            summary.create_directories += 1;
                        } else {
                            let bytes = entry.left_size.max(entry.right_size);
                            items.push(SyncItem {
                                relative_path: entry.relative_path.clone(),
                                action: SyncAction::CopyFile,
                                source,
                                destination: Some(append_component(
                                    &destination_root,
                                    &entry.relative_path,
                                )),
                                bytes,
                                reason: "コピーが必要".into(),
                                direction_key: dk,
                            });
                            summary.copy_files += 1;
                            summary.total_bytes += bytes;
                        }
                    } else {
                        let dest = if source_is_left {
                            entry.right_path.as_ref()
                        } else {
                            entry.left_path.as_ref()
                        }
                        .map(|p| p.to_string_lossy().into_owned());
                        items.push(SyncItem {
                            relative_path: entry.relative_path.clone(),
                            action: SyncAction::PreservedDestination,
                            source: None,
                            destination: dest,
                            bytes: 0,
                            reason: "反対側のみに存在するため削除せず保持".into(),
                            direction_key: dk,
                        });
                        summary.preserved_destinations += 1;
                    }
                }
                DiffStatus::Different => {
                    let source = if source_is_left {
                        entry.left_path.as_ref()
                    } else {
                        entry.right_path.as_ref()
                    }
                    .map(|p| p.to_string_lossy().into_owned());
                    let dest = if source_is_left {
                        entry.right_path.as_ref()
                    } else {
                        entry.left_path.as_ref()
                    }
                    .map(|p| p.to_string_lossy().into_owned());
                    items.push(SyncItem {
                        relative_path: entry.relative_path.clone(),
                        action: SyncAction::Conflict,
                        source,
                        destination: dest,
                        bytes: 0,
                        reason: "両側に存在し内容が異なるため上書き候補".into(),
                        direction_key: dk,
                    });
                    summary.conflicts += 1;
                }
                DiffStatus::Error => {
                    items.push(SyncItem {
                        relative_path: entry.relative_path.clone(),
                        action: SyncAction::Blocked,
                        source: entry
                            .left_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned()),
                        destination: entry
                            .right_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned()),
                        bytes: 0,
                        reason: entry
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "比較エラーのため実行不可".into()),
                        direction_key: dk,
                    });
                    summary.blocked += 1;
                }
            }
        }
        SyncPlan {
            direction,
            source_root,
            destination_root,
            items,
            summary,
        }
    }
}
