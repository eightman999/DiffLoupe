//! Swift `DiffLoupeTests/GitSnapshotTests.swift` の 1:1 移植(8件)。
//! 一時 git リポジトリを /usr/bin/git で構築して検証する。

use diffloupe_core::git_snapshot::GitRepository;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);
const GIT: &str = "/usr/bin/git";

fn git_available() -> bool {
    Path::new(GIT).exists()
}

fn run_git(args: &[&str], dir: &Path) {
    let out = Command::new(GIT)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git の起動に失敗");
    assert!(
        out.status.success(),
        "git {} 失敗: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn make_fixture_repo() -> PathBuf {
    let repo = std::env::temp_dir().join(format!(
        "DiffLoupeTests-git-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&repo).unwrap();
    run_git(&["init", "-b", "main"], &repo);
    run_git(&["config", "user.email", "test@diffloupe.example"], &repo);
    run_git(&["config", "user.name", "DiffLoupe Test"], &repo);
    std::fs::write(repo.join("hello.txt"), "hello from main\n").unwrap();
    run_git(&["add", "."], &repo);
    run_git(&["commit", "-m", "initial commit"], &repo);
    run_git(&["branch", "feature"], &repo);
    run_git(&["checkout", "feature"], &repo);
    std::fs::write(repo.join("hello.txt"), "hello from feature\n").unwrap();
    run_git(&["commit", "-am", "feature commit"], &repo);
    run_git(&["checkout", "main"], &repo);
    repo
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_detect_finds_repository_at_toplevel() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let detected = GitRepository::detect(&repo_url);
    assert!(detected.is_some());
    assert_eq!(
        detected.unwrap().toplevel.canonicalize().unwrap(),
        repo_url.canonicalize().unwrap()
    );
}

#[test]
fn test_detect_finds_repository_from_subdirectory() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let subdir = repo_url.join("sub");
    std::fs::create_dir_all(&subdir).unwrap();
    assert!(GitRepository::detect(&subdir).is_some());
}

#[test]
fn test_detect_returns_nil_for_non_repository() {
    if !git_available() {
        return;
    }
    let plain = std::env::temp_dir().join(format!(
        "DiffLoupeTests-plain-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&plain).unwrap();
    let _c = Cleanup(plain.clone());
    // /tmp 配下が git 作業ツリー内にある環境では本テストは成立しない
    assert!(GitRepository::detect(&plain).is_none() || std::env::var("GIT_DIR").is_ok());
}

#[test]
fn test_list_refs_includes_head_and_branches() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let repo = GitRepository::detect(&repo_url).expect("リポジトリ検出に失敗");
    let refs = repo.list_refs();
    assert_eq!(refs.first().map(|s| s.as_str()), Some("HEAD"));
    assert!(refs.iter().any(|r| r == "main"));
    assert!(refs.iter().any(|r| r == "feature"));
}

#[test]
fn test_materialize_snapshot_extracts_branch_content() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let repo = GitRepository::detect(&repo_url).expect("リポジトリ検出に失敗");
    let snapshot = repo.materialize_snapshot("feature").unwrap();
    let content = std::fs::read_to_string(snapshot.join("hello.txt")).unwrap();
    assert_eq!(content, "hello from feature\n");
    GitRepository::cleanup(&snapshot);
}

#[test]
fn test_materialize_snapshot_differs_from_main() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let repo = GitRepository::detect(&repo_url).expect("リポジトリ検出に失敗");
    let snapshot = repo.materialize_snapshot("main").unwrap();
    let content = std::fs::read_to_string(snapshot.join("hello.txt")).unwrap();
    assert_eq!(content, "hello from main\n");
    GitRepository::cleanup(&snapshot);
}

#[test]
fn test_materialize_snapshot_throws_for_unknown_ref() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let repo = GitRepository::detect(&repo_url).expect("リポジトリ検出に失敗");
    let result = repo.materialize_snapshot("does-not-exist-ref");
    assert!(result.is_err());
}

#[test]
fn test_cleanup_removes_snapshot_directory() {
    if !git_available() {
        return;
    }
    let repo_url = make_fixture_repo();
    let _c = Cleanup(repo_url.clone());
    let repo = GitRepository::detect(&repo_url).expect("リポジトリ検出に失敗");
    let snapshot = repo.materialize_snapshot("main").unwrap();
    assert!(snapshot.exists());
    GitRepository::cleanup(&snapshot);
    assert!(!snapshot.exists());
}
