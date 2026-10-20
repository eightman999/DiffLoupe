//! Swift `DiffLoupeTests/SyncPlanTests.swift` と `CoreTests/SyncPlanTests.swift` の 1:1 移植

use diffloupe_core::models::{
    ComparedEntry, ComparisonResult, ComparisonSummary, DiffNode, DiffStatus,
};
use diffloupe_core::sync_plan::{SyncAction, SyncDirection, SyncItem, SyncPlan};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn entry(
    path: &str,
    status: DiffStatus,
    is_directory: bool,
    left_size: i64,
    right_size: i64,
    error_message: Option<&str>,
) -> ComparedEntry {
    let name = path.rsplit('/').next().unwrap_or(path).to_string();
    ComparedEntry {
        relative_path: path.to_string(),
        name,
        is_directory,
        status,
        left_path: Some(PathBuf::from(format!("/tmp/left/{}", path))),
        right_path: Some(PathBuf::from(format!("/tmp/right/{}", path))),
        left_size,
        right_size,
        error_message: error_message.map(String::from),
        left_modification_date: None,
        right_modification_date: None,
        left_posix: None,
        right_posix: None,
        rename_candidate: None,
    }
}

fn root_node() -> DiffNode {
    DiffNode {
        id: String::new(),
        name: String::new(),
        is_directory: true,
        status: DiffStatus::Different,
        children: None,
    }
}

fn by_path(plan: &SyncPlan) -> HashMap<&str, &SyncItem> {
    plan.items
        .iter()
        .map(|i| (i.relative_path.as_str(), i))
        .collect()
}

/// DiffLoupeTests/SyncPlanTests.swift(3件)
mod diffloupe_tests {
    use super::*;

    fn make_result() -> ComparisonResult {
        let entries = vec![
            entry("same.txt", DiffStatus::Identical, false, 0, 0, None),
            entry("changed.txt", DiffStatus::Different, false, 100, 100, None),
            entry("left_only.txt", DiffStatus::LeftOnly, false, 42, 0, None),
            entry("right_only.txt", DiffStatus::RightOnly, false, 0, 7, None),
            entry(
                "unreadable.txt",
                DiffStatus::Error,
                false,
                0,
                0,
                Some("読み込み不可"),
            ),
        ];
        ComparisonResult {
            left_root: "/tmp/left".into(),
            right_root: "/tmp/right".into(),
            entries,
            root_node: root_node(),
            summary: ComparisonSummary {
                identical_files: 1,
                different_files: 1,
                left_only_files: 1,
                right_only_files: 1,
                error_files: 1,
            },
        }
    }

    #[test]
    fn test_left_to_right_copies_left_only_and_preserves_right_only() {
        let plan = SyncPlan::make(&make_result(), SyncDirection::LeftToRight);
        let by = by_path(&plan);
        assert_eq!(by["left_only.txt"].action, SyncAction::CopyFile);
        assert_eq!(
            by["right_only.txt"].action,
            SyncAction::PreservedDestination
        );
        assert_eq!(by["changed.txt"].action, SyncAction::Conflict);
        assert_eq!(by["unreadable.txt"].action, SyncAction::Blocked);
        assert!(!by.contains_key("same.txt"));
    }

    #[test]
    fn test_right_to_left_swaps_source_and_destination() {
        let plan = SyncPlan::make(&make_result(), SyncDirection::RightToLeft);
        assert_eq!(plan.source_root, "/tmp/right");
        assert_eq!(plan.destination_root, "/tmp/left");
        let by = by_path(&plan);
        assert_eq!(by["right_only.txt"].action, SyncAction::CopyFile);
        assert_eq!(by["left_only.txt"].action, SyncAction::PreservedDestination);
        assert_eq!(
            by["changed.txt"].source.as_deref(),
            Some("/tmp/right/changed.txt")
        );
        assert_eq!(
            by["changed.txt"].destination.as_deref(),
            Some("/tmp/left/changed.txt")
        );
    }

    #[test]
    fn test_summary_counts_by_action() {
        let summary = SyncPlan::make(&make_result(), SyncDirection::LeftToRight).summary;
        assert_eq!(summary.copy_files, 1);
        assert_eq!(summary.conflicts, 1);
        assert_eq!(summary.preserved_destinations, 1);
        assert_eq!(summary.blocked, 1);
        assert_eq!(summary.total_bytes, 42);
    }
}

/// CoreTests/SyncPlanTests.swift(6件)
mod core_tests {
    use super::*;

    fn make_result() -> ComparisonResult {
        let entries = vec![
            entry("same.txt", DiffStatus::Identical, false, 0, 0, None),
            entry("changed.txt", DiffStatus::Different, false, 100, 100, None),
            entry("left_only.txt", DiffStatus::LeftOnly, false, 42, 0, None),
            entry("left_only_dir", DiffStatus::LeftOnly, true, 0, 0, None),
            entry("right_only.txt", DiffStatus::RightOnly, false, 0, 7, None),
            entry(
                "unreadable.txt",
                DiffStatus::Error,
                false,
                0,
                0,
                Some("読み込み不可"),
            ),
        ];
        ComparisonResult {
            left_root: "/tmp/left".into(),
            right_root: "/tmp/right".into(),
            entries,
            root_node: root_node(),
            summary: ComparisonSummary {
                identical_files: 1,
                different_files: 1,
                left_only_files: 2,
                right_only_files: 1,
                error_files: 1,
            },
        }
    }

    // MARK: - leftToRight

    #[test]
    fn test_left_to_right_maps_each_status() {
        let plan = SyncPlan::make(&make_result(), SyncDirection::LeftToRight);
        assert_eq!(plan.direction, SyncDirection::LeftToRight);
        assert_eq!(plan.source_root, "/tmp/left");
        assert_eq!(plan.destination_root, "/tmp/right");
        let by = by_path(&plan);

        // identicalは対象外
        assert!(!by.contains_key("same.txt"));

        // コピー元のみファイル → copyFile
        let copy = by["left_only.txt"];
        assert_eq!(copy.action, SyncAction::CopyFile);
        assert_eq!(copy.source.as_deref(), Some("/tmp/left/left_only.txt"));
        assert_eq!(
            copy.destination.as_deref(),
            Some("/tmp/right/left_only.txt")
        );
        assert_eq!(copy.bytes, 42);

        // コピー元のみディレクトリ → createDirectory
        let dir = by["left_only_dir"];
        assert_eq!(dir.action, SyncAction::CreateDirectory);
        assert_eq!(dir.source.as_deref(), Some("/tmp/left/left_only_dir"));
        assert_eq!(dir.destination.as_deref(), Some("/tmp/right/left_only_dir"));

        // コピー先のみ → preservedDestination(source nil・削除しない)
        let preserved = by["right_only.txt"];
        assert_eq!(preserved.action, SyncAction::PreservedDestination);
        assert!(preserved.source.is_none());
        assert_eq!(
            preserved.destination.as_deref(),
            Some("/tmp/right/right_only.txt")
        );

        // 差分ファイル → conflict
        let conflict = by["changed.txt"];
        assert_eq!(conflict.action, SyncAction::Conflict);
        assert_eq!(conflict.source.as_deref(), Some("/tmp/left/changed.txt"));
        assert_eq!(
            conflict.destination.as_deref(),
            Some("/tmp/right/changed.txt")
        );

        // 比較エラー → blocked(reasonはエラーメッセージ)
        let blocked = by["unreadable.txt"];
        assert_eq!(blocked.action, SyncAction::Blocked);
        assert_eq!(blocked.reason, "読み込み不可");
    }

    #[test]
    fn test_left_to_right_summary() {
        let summary = SyncPlan::make(&make_result(), SyncDirection::LeftToRight).summary;
        assert_eq!(summary.copy_files, 1);
        assert_eq!(summary.create_directories, 1);
        assert_eq!(summary.conflicts, 1);
        assert_eq!(summary.preserved_destinations, 1);
        assert_eq!(summary.blocked, 1);
        assert_eq!(summary.total_bytes, 42);
        assert_eq!(summary.total_items(), 5);
    }

    // MARK: - rightToLeft(方向反転)

    #[test]
    fn test_right_to_left_maps_each_status() {
        let plan = SyncPlan::make(&make_result(), SyncDirection::RightToLeft);
        assert_eq!(plan.direction, SyncDirection::RightToLeft);
        assert_eq!(plan.source_root, "/tmp/right");
        assert_eq!(plan.destination_root, "/tmp/left");
        let by = by_path(&plan);

        // 右のみ(コピー元のみ扱いになる) → copyFile
        let copy = by["right_only.txt"];
        assert_eq!(copy.action, SyncAction::CopyFile);
        assert_eq!(copy.source.as_deref(), Some("/tmp/right/right_only.txt"));
        assert_eq!(
            copy.destination.as_deref(),
            Some("/tmp/left/right_only.txt")
        );
        assert_eq!(copy.bytes, 7);

        // 左のみ → preservedDestination
        assert_eq!(by["left_only.txt"].action, SyncAction::PreservedDestination);
        assert!(by["left_only.txt"].source.is_none());

        // 差分 → conflict(source/destinationが反転)
        let conflict = by["changed.txt"];
        assert_eq!(conflict.action, SyncAction::Conflict);
        assert_eq!(conflict.source.as_deref(), Some("/tmp/right/changed.txt"));
        assert_eq!(
            conflict.destination.as_deref(),
            Some("/tmp/left/changed.txt")
        );

        // エラー → blocked
        assert_eq!(by["unreadable.txt"].action, SyncAction::Blocked);
    }

    #[test]
    fn test_right_to_left_summary_bytes() {
        let summary = SyncPlan::make(&make_result(), SyncDirection::RightToLeft).summary;
        assert_eq!(summary.copy_files, 1);
        assert_eq!(summary.preserved_destinations, 2);
        assert_eq!(summary.total_bytes, 7);
    }

    // MARK: - Identifiable/Equatable/Hashable

    #[test]
    fn test_items_are_identifiable_and_direction_aware() {
        let plan = SyncPlan::make(&make_result(), SyncDirection::LeftToRight);
        let ids: Vec<String> = plan.items.iter().map(|i| i.id()).collect();
        let unique: HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "idはdirection込みで一意であるべき");
        let other = SyncPlan::make(&make_result(), SyncDirection::RightToLeft);
        let right_ids: HashSet<String> = other.items.iter().map(|i| i.id()).collect();
        assert!(ids.iter().all(|id| !right_ids.contains(id)));
    }

    #[test]
    fn test_sync_item_is_equatable_and_hashable() {
        let a = SyncItem {
            direction_key: "leftToRight",
            relative_path: "x.txt".into(),
            action: SyncAction::CopyFile,
            source: Some("/a/x.txt".into()),
            destination: Some("/b/x.txt".into()),
            bytes: 1,
            reason: String::new(),
        };
        let b = a.clone();
        assert_eq!(a, b);
        let set: HashSet<SyncItem> = [a, b].into_iter().collect();
        assert_eq!(set.len(), 1);
    }
}
