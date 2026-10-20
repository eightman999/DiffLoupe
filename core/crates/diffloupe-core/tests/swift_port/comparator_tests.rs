//! Swift `DiffLoupeTests/ComparatorTests.swift` の 1:1 移植(8件)

use super::support::{compare, temp_pair, write};
use diffloupe_core::cancel::CancellationToken;
use diffloupe_core::comparator::{CompareError, CompareOptions, FolderComparator};
use diffloupe_core::models::{ComparisonProgress, DiffStatus};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn fixtures_dir() -> PathBuf {
    // このファイルは crates/diffloupe-core/tests/swift_port/ にある。
    // ワークスペースルート .../SwiftUI/TestFixtures を辿る。
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .join("../../../SwiftUI/TestFixtures")
        .canonicalize()
        .unwrap()
}

fn status(result: &diffloupe_core::models::ComparisonResult, p: &str) -> Option<DiffStatus> {
    result.entry_by_path(p).map(|e| e.status)
}

#[test]
fn test_fixture_classification() {
    let fixtures = fixtures_dir();
    let result = compare(
        &fixtures.join("left"),
        &fixtures.join("right"),
        &CompareOptions::default(),
    );
    assert_eq!(status(&result, "same.txt"), Some(DiffStatus::Identical));
    assert_eq!(
        status(&result, "binary_same.bin"),
        Some(DiffStatus::Identical)
    );
    assert_eq!(
        status(&result, "sub/nested_same.txt"),
        Some(DiffStatus::Identical)
    );
    assert_eq!(
        status(&result, "changed_same_size.txt"),
        Some(DiffStatus::Different)
    );
    assert_eq!(
        status(&result, "binary_diff.bin"),
        Some(DiffStatus::Different)
    );
    assert_eq!(
        status(&result, "changed_size.txt"),
        Some(DiffStatus::Different)
    );
    assert_eq!(
        status(&result, "sub/nested_changed.txt"),
        Some(DiffStatus::Different)
    );
    assert_eq!(status(&result, "only_left.txt"), Some(DiffStatus::LeftOnly));
    assert_eq!(
        status(&result, "only_right.txt"),
        Some(DiffStatus::RightOnly)
    );
    assert_eq!(
        status(&result, "sub/deep/only_left_deep.txt"),
        Some(DiffStatus::LeftOnly)
    );
    assert_eq!(
        status(&result, "sub/deep2/only_right_deep.txt"),
        Some(DiffStatus::RightOnly)
    );
    assert_eq!(status(&result, "sub"), Some(DiffStatus::Different));
    assert_eq!(status(&result, "sub/deep"), Some(DiffStatus::LeftOnly));
    assert_eq!(status(&result, "sub/deep2"), Some(DiffStatus::RightOnly));
    assert_eq!(status(&result, "typemix"), Some(DiffStatus::Different));
    assert_eq!(
        status(&result, "typemix/inner.txt"),
        Some(DiffStatus::RightOnly)
    );
    assert_eq!(status(&result, ".hidden.txt"), Some(DiffStatus::Different));
    assert_eq!(result.summary.identical_files, 3);
    assert_eq!(result.summary.different_files, 5);
    assert_eq!(result.summary.left_only_files, 2);
    assert_eq!(result.summary.right_only_files, 3);
    assert_eq!(result.summary.error_files, 0);
}

#[test]
fn test_tree_structure() {
    let fixtures = fixtures_dir();
    let result = compare(
        &fixtures.join("left"),
        &fixtures.join("right"),
        &CompareOptions::default(),
    );
    let root = &result.root_node;
    assert_eq!(root.status, DiffStatus::Different);
    let children = root.children.as_ref().unwrap();
    let sub_index = children.iter().position(|c| c.name == "sub").unwrap();
    let first_file_index = children.iter().position(|c| c.children.is_none()).unwrap();
    assert!(sub_index < first_file_index);
    let sub = &children[sub_index];
    assert_eq!(sub.status, DiffStatus::Different);
    let deep = sub
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|c| c.name == "deep")
        .unwrap();
    assert_eq!(deep.status, DiffStatus::LeftOnly);
    assert_eq!(deep.children.as_ref().unwrap().len(), 1);
}

#[test]
fn test_default_excludes() {
    let (base, l, r) = temp_pair("cmp1");
    write(&l, ".git/config", b"ref");
    write(&l, ".DS_Store", b"junk");
    write(&l, "node_modules/pkg/index.js", b"module");
    write(&l, "keep.txt", b"keep");
    let excluded = compare(&l, &r, &CompareOptions::default());
    let paths: Vec<&str> = excluded
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["keep.txt"]);
    assert_eq!(
        excluded.entry_by_path("keep.txt").unwrap().status,
        DiffStatus::LeftOnly
    );
    let included = compare(
        &l,
        &r,
        &CompareOptions {
            use_default_excludes: false,
            ..Default::default()
        },
    );
    assert!(included.entry_by_path(".git").is_some());
    assert!(included.entry_by_path(".git/config").is_some());
    assert!(included.entry_by_path(".DS_Store").is_some());
    assert!(included
        .entry_by_path("node_modules/pkg/index.js")
        .is_some());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_empty_directories() {
    let (base, l, r) = temp_pair("cmp2");
    std::fs::create_dir_all(l.join("both_empty")).unwrap();
    std::fs::create_dir_all(r.join("both_empty")).unwrap();
    std::fs::create_dir_all(l.join("left_only_dir")).unwrap();
    let result = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        result.entry_by_path("both_empty").unwrap().status,
        DiffStatus::Identical
    );
    assert_eq!(
        result.entry_by_path("left_only_dir").unwrap().status,
        DiffStatus::LeftOnly
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
#[cfg(unix)]
fn test_symlink_handling() {
    let (base, l, r) = temp_pair("cmp3");
    write(&l, "real_dir/file.txt", b"real content");
    std::os::unix::fs::symlink(l.join("real_dir"), l.join("dir_link")).unwrap();
    write(&l, "target.txt", b"link target");
    std::os::unix::fs::symlink(l.join("target.txt"), l.join("file_link.txt")).unwrap();
    write(&r, "file_link.txt", b"link target");
    std::os::unix::fs::symlink(l.join("nonexistent"), l.join("broken.txt")).unwrap();
    std::os::unix::fs::symlink(r.join("nonexistent"), r.join("broken.txt")).unwrap();
    let result = compare(&l, &r, &CompareOptions::default());
    assert!(result.entry_by_path("dir_link").is_none());
    assert!(result.entry_by_path("dir_link/file.txt").is_none());
    assert_eq!(
        result.entry_by_path("file_link.txt").unwrap().status,
        DiffStatus::Identical
    );
    assert_eq!(
        result.entry_by_path("broken.txt").unwrap().status,
        DiffStatus::Error
    );
    assert_eq!(result.summary.error_files, 1);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_cancellation() {
    let (base, l, r) = temp_pair("cmp4");
    let payload = "x".repeat(4096);
    for i in 0..200 {
        write(&l, &format!("file{}.txt", i), payload.as_bytes());
        write(&r, &format!("file{}.txt", i), payload.as_bytes());
    }
    let token = CancellationToken::new();
    token.cancel();
    let result = FolderComparator::compare(&l, &r, &CompareOptions::default(), None, Some(&token));
    assert!(matches!(result, Err(CompareError::Cancelled)));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_progress_reporting() {
    let (base, l, r) = temp_pair("cmp5");
    for i in 0..50 {
        let c = format!("content {}", i);
        write(&l, &format!("f{}.txt", i), c.as_bytes());
        write(&r, &format!("f{}.txt", i), c.as_bytes());
    }
    let boxx = Arc::new(Mutex::new((false, 0usize, 0usize))); // scanned, comparedTotal, lastDone
    let bx = boxx.clone();
    let progress = move |p: ComparisonProgress| {
        let mut g = bx.lock().unwrap();
        match p {
            ComparisonProgress::Scanning { .. } => g.0 = true,
            ComparisonProgress::Comparing { done, total } => {
                g.1 = total;
                g.2 = g.2.max(done);
            }
            ComparisonProgress::Renames { .. } => {}
        }
    };
    let _ = FolderComparator::compare(&l, &r, &CompareOptions::default(), Some(&progress), None)
        .unwrap();
    let (scanned, total, last_done) = *boxx.lock().unwrap();
    assert!(scanned);
    assert_eq!(total, 50);
    assert_eq!(last_done, 50);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_entry_limit_stops_scanning() {
    let (base, l, r) = temp_pair("cmp6");
    write(&l, "a.txt", b"1");
    write(&l, "b.txt", b"2");
    let result = FolderComparator::compare(
        &l,
        &r,
        &CompareOptions {
            max_entries: Some(1),
            ..Default::default()
        },
        None,
        None,
    );
    match result {
        Err(CompareError::EntryLimitExceeded(e)) => {
            assert_eq!(e.limit, 1);
            assert_eq!(e.observed, 2);
        }
        _ => panic!("上限超過で停止するべき"),
    }
    std::fs::remove_dir_all(&base).ok();
}
