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
    /// Swift `SyncDirection.displayName`
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::LeftToRight => "左 → 右",
            Self::RightToLeft => "右 → 左",
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

impl SyncAction {
    /// Swift `SyncAction.rawValue`
    pub fn raw_value(&self) -> &'static str {
        match self {
            Self::CopyFile => "copyFile",
            Self::CreateDirectory => "createDirectory",
            Self::Conflict => "conflict",
            Self::PreservedDestination => "preservedDestination",
            Self::Blocked => "blocked",
        }
    }
    /// Swift `SyncAction.displayName`
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CopyFile => "コピー",
            Self::CreateDirectory => "ディレクトリ作成",
            Self::Conflict => "差分(上書き候補)",
            Self::PreservedDestination => "保持",
            Self::Blocked => "エラー",
        }
    }
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
    /// 実際に書き込む先のパス(計画 `item.destination` は変えない追加 API)。
    ///
    /// 計画は相対パスを Unicode 正規化(NFC)して持つため、そのまま使うと正規化を保つ
    /// ファイルシステム(APFS 等)で NFD のファイル名が NFC に変わる。コピー・ディレクトリ作成では
    /// コピー元の実パス末尾コンポーネントを使い、元のファイル名の表記(NFC/NFD)を引き継ぐ。
    /// 上書き・削除など既存の宛先は計画のパスをそのまま返す。宛先未定義は None。
    pub fn destination_preserving_source_name(&self, item: &SyncItem) -> Option<String> {
        use std::path::{Component, Path};
        let planned = item.destination.as_deref()?;
        if !matches!(
            item.action,
            SyncAction::CopyFile | SyncAction::CreateDirectory
        ) {
            return Some(planned.to_string());
        }
        let Some(src) = item.source.as_deref() else {
            return Some(planned.to_string());
        };
        let n = item
            .relative_path
            .split('/')
            .filter(|s| !s.is_empty())
            .count();
        let comps: Vec<_> = Path::new(src)
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s),
                _ => None,
            })
            .collect();
        if n == 0 || comps.len() < n {
            return Some(planned.to_string());
        }
        // 計画の destination と同じく `/` で連結する(Windows でも表記を揃える)
        let tail: Vec<String> = comps[comps.len() - n..]
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .collect();
        Some(append_component(&self.destination_root, &tail.join("/")))
    }

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
