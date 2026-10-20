//! Swift `CoreTests/DirectoryWatcherTests.swift` の 1:1 移植(15件)

use super::support::{temp_pair, write};
use diffloupe_core::watcher::{DirectorySnapshot, DirectoryWatcher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn scan(dir: &Path) -> DirectorySnapshot {
    DirectorySnapshot::scan(dir, &[]).unwrap()
}

fn has(
    s: &DirectorySnapshot,
    pred: impl Fn(&diffloupe_core::watcher::SnapshotEntry) -> bool,
) -> bool {
    s.entries.iter().any(pred)
}

fn rel(s: &DirectorySnapshot) -> Vec<&str> {
    s.entries.iter().map(|e| e.relative_path.as_str()).collect()
}

// MARK: - スキャン

#[test]
fn test_scan_same_tree_produces_equal_snapshots() {
    let (base, l, _r) = temp_pair("dw1");
    write(&l, "sub/nested.txt", b"AAA");
    write(&l, "top.txt", b"BBB");
    let first = scan(&l);
    let second = scan(&l);
    assert_eq!(first, second);
    assert_eq!(first.entries.len(), 3);
    assert!(has(&first, |e| e.relative_path == "top.txt" && !e.is_directory));
    assert!(has(&first, |e| e.relative_path == "sub" && e.is_directory));
    assert!(has(&first, |e| e.relative_path == "sub/nested.txt"
        && !e.is_directory));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_is_order_independent() {
    let (base, l, _r) = temp_pair("dw2");
    write(&l, "b.txt", b"AAA");
    write(&l, "a.txt", b"BBB");
    let first = scan(&l);
    let second = scan(&l);
    assert_eq!(rel(&first), ["a.txt", "b.txt"]);
    assert_eq!(first, second);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_detects_file_addition() {
    let (base, l, _r) = temp_pair("dw3");
    write(&l, "top.txt", b"AAA");
    let before = scan(&l);
    write(&l, "added.txt", b"BBB");
    let after = scan(&l);
    assert_ne!(before, after);
    assert!(has(&after, |e| e.relative_path == "added.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_detects_file_removal() {
    let (base, l, _r) = temp_pair("dw4");
    write(&l, "top.txt", b"AAA");
    write(&l, "gone.txt", b"BBB");
    let before = scan(&l);
    std::fs::remove_file(l.join("gone.txt")).unwrap();
    let after = scan(&l);
    assert_ne!(before, after);
    assert!(!has(&after, |e| e.relative_path == "gone.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_detects_size_change() {
    let (base, l, _r) = temp_pair("dw5");
    write(&l, "top.txt", b"AAA");
    let before = scan(&l);
    write(&l, "top.txt", b"AAAA");
    let after = scan(&l);
    assert_ne!(before, after);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_detects_modification_date_change() {
    let (base, l, _r) = temp_pair("dw6");
    write(&l, "top.txt", b"AAA");
    let before = scan(&l);
    let future = std::time::SystemTime::now() + Duration::from_secs(60);
    filetime(&l.join("top.txt"), future);
    let after = scan(&l);
    assert_ne!(before, after);
    std::fs::remove_dir_all(&base).ok();
}

/// mtime を任意の時刻へ更新する(std の `File::set_modified` を使うため Unix/Windows 共通)。
fn filetime(path: &Path, t: std::time::SystemTime) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap_or_else(|e| panic!("open for set_modified failed {}: {}", path.display(), e));
    f.set_modified(t)
        .unwrap_or_else(|e| panic!("set_modified failed {}: {}", path.display(), e));
}

#[test]
fn test_scan_detects_directory_addition() {
    let (base, l, _r) = temp_pair("dw7");
    let before = scan(&l);
    write(&l, "newdir/inner.txt", b"x");
    let after = scan(&l);
    assert_ne!(before, after);
    assert!(has(&after, |e| e.relative_path == "newdir" && e.is_directory));
    assert!(has(&after, |e| e.relative_path == "newdir/inner.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
#[cfg(unix)]
fn test_scan_does_not_follow_directory_symlinks() {
    let (base, l, _r) = temp_pair("dw8");
    let (tbase, target, _tr) = temp_pair("dw8t");
    write(&target, "inside.txt", b"x");
    std::os::unix::fs::symlink(&target, l.join("linkdir")).unwrap();
    let snapshot = scan(&l);
    assert!(has(&snapshot, |e| e.relative_path == "linkdir"
        && e.is_symbolic_link));
    assert!(!has(&snapshot, |e| e.relative_path == "linkdir/inside.txt"));
    std::fs::remove_dir_all(&base).ok();
    std::fs::remove_dir_all(&tbase).ok();
}

#[test]
fn test_scan_throws_for_non_directory_root() {
    let (base, l, _r) = temp_pair("dw9");
    let file = l.join("f.txt");
    std::fs::write(&file, b"x").unwrap();
    let result = DirectorySnapshot::scan(&file, &[]);
    assert!(matches!(
        result,
        Err(diffloupe_core::watcher::WatchError::RootIsNotDirectory(_))
    ));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_scan_excludes_named_entries_recursively() {
    let (base, l, _r) = temp_pair("dw10");
    write(&l, ".git/config", b"x");
    write(&l, ".DS_Store", b"x");
    write(&l, "node_modules/pkg/index.js", b"x");
    write(&l, "keep.txt", b"x");
    write(&l, "sub/.DS_Store", b"x");
    write(&l, "sub/keep.txt", b"x");
    let snapshot = DirectorySnapshot::scan(&l, &[".git", ".DS_Store", "node_modules"]).unwrap();
    assert!(has(&snapshot, |e| e.relative_path == "keep.txt"));
    assert!(has(&snapshot, |e| e.relative_path == "sub" && e.is_directory));
    assert!(has(&snapshot, |e| e.relative_path == "sub/keep.txt"));
    for p in [
        ".git",
        ".git/config",
        "node_modules",
        "node_modules/pkg/index.js",
        ".DS_Store",
        "sub/.DS_Store",
    ] {
        assert!(!has(&snapshot, |e| e.relative_path == p));
    }
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - ウォッチャー(タイミング依存・実通知検証)

fn wait_for(flag: &Arc<AtomicBool>, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if flag.load(Ordering::SeqCst) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    flag.load(Ordering::SeqCst)
}

fn dirs2(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let (base, l, r) = temp_pair(tag);
    (base, l, r)
}

#[test]
fn test_watcher_ignores_changes_inside_excluded_directory() {
    let (base, left, right) = dirs2("dw11");
    write(&left, ".git/config", b"x");
    let notified = Arc::new(AtomicBool::new(false));
    let n = notified.clone();
    let watcher = DirectoryWatcher::new(
        vec![left.clone(), right.clone()],
        Duration::from_millis(50),
        vec![".git".into(), ".DS_Store".into(), "node_modules".into()],
        move || n.store(true, Ordering::SeqCst),
    );
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));
    write(&left, ".git/config", b"y");
    // 1秒待っても通知が来ないこと
    assert!(!wait_for(&notified, Duration::from_secs(1)));
    watcher.stop();
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_stop_then_start_reuse_is_safe() {
    let (base, left, right) = dirs2("dw12");
    let notified = Arc::new(AtomicBool::new(false));
    let n = notified.clone();
    let watcher = DirectoryWatcher::new(
        vec![left.clone(), right.clone()],
        Duration::from_millis(50),
        vec![],
        move || n.store(true, Ordering::SeqCst),
    );
    watcher.start();
    watcher.stop();
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));
    write(&left, "added.txt", b"x");
    assert!(wait_for(&notified, Duration::from_secs(3)));
    watcher.stop();
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_stop_during_baseline_is_safe_and_never_notifies() {
    let (base, left, right) = dirs2("dw13");
    let notified = Arc::new(AtomicBool::new(false));
    let n = notified.clone();
    let watcher = DirectoryWatcher::new(
        vec![left.clone(), right.clone()],
        Duration::from_millis(50),
        vec![],
        move || n.store(true, Ordering::SeqCst),
    );
    watcher.start();
    watcher.stop();
    write(&left, "added.txt", b"x");
    assert!(!wait_for(&notified, Duration::from_secs(1)));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_watcher_detects_change_and_notifies() {
    let (base, left, right) = dirs2("dw14");
    let notified = Arc::new(AtomicBool::new(false));
    let n = notified.clone();
    let watcher = DirectoryWatcher::new(
        vec![left.clone(), right.clone()],
        Duration::from_millis(50),
        vec![],
        move || n.store(true, Ordering::SeqCst),
    );
    watcher.start();
    std::thread::sleep(Duration::from_millis(150));
    write(&left, "added.txt", b"x");
    assert!(wait_for(&notified, Duration::from_secs(3)));
    watcher.stop();
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_watcher_notifies_after_stop() {
    let (base, left, right) = dirs2("dw15");
    let notified = Arc::new(AtomicBool::new(false));
    let n = notified.clone();
    let watcher = DirectoryWatcher::new(
        vec![left.clone(), right.clone()],
        Duration::from_millis(50),
        vec![],
        move || n.store(true, Ordering::SeqCst),
    );
    watcher.start();
    std::thread::sleep(Duration::from_millis(100));
    watcher.stop();
    write(&left, "after_stop.txt", b"x");
    assert!(!wait_for(&notified, Duration::from_secs(1)));
    std::fs::remove_dir_all(&base).ok();
}
