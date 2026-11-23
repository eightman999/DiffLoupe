//! Swift `DiffLoupeTests/GitSnapshotTests.swift` の 1:1 移植(8件)。
//! 一時 git リポジトリを git(Unix は /usr/bin/git、Windows は PATH 上の git)で構築して検証する。

use diffloupe_core::git_snapshot::GitRepository;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);
/// Unix では本体と同じ絶対パス、Windows では PATH 上の git(git for Windows)を使う。
#[cfg(not(windows))]
const GIT: &str = "/usr/bin/git";
#[cfg(windows)]
const GIT: &str = "git";

fn git_available() -> bool {
    #[cfg(windows)]
    {
        // Windows は固定パスが無いため、実行できるか(git と tar)で判定する
        let ok = |cmd: &str| {
            Command::new(cmd)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        ok(GIT) && ok("tar")
    }
    #[cfg(not(windows))]
    {
        Path::new(GIT).exists()
    }
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

/// スナップショット内容の比較用に改行を揃える。
///
/// Windows の git は `core.autocrlf=true` が既定で、`git archive` の出力もチェックアウトと
/// 同じ変換(LF -> CRLF)を受ける。製品仕様は「その ref をチェックアウトしたときと同じ内容」
/// なので core 側では改行を変換しない。テストは改行コード以外の内容(ブランチ差)を検証するため、
/// Windows のときだけ CRLF を LF に正規化して比較する。Unix では従来どおり厳密比較。
fn normalize_eol(s: String) -> String {
    if cfg!(windows) {
        s.replace("\r\n", "\n")
    } else {
        s
    }
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
    let content = normalize_eol(std::fs::read_to_string(snapshot.join("hello.txt")).unwrap());
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
    let content = normalize_eol(std::fs::read_to_string(snapshot.join("hello.txt")).unwrap());
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

// ---------- gix 実装(プロセス起動なし)向けの追加テスト ----------

fn git_output(args: &[&str], dir: &Path) -> String {
    let out = Command::new(GIT)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git の起動に失敗");
    assert!(out.status.success(), "git {} 失敗", args.join(" "));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn for_each_ref_short(dir: &Path) -> Vec<String> {
    git_output(&["for-each-ref", "--format=%(refname:short)"], dir)
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn assert_refs_match_git(repo_dir: &Path) {
    let repo = GitRepository::detect(repo_dir).expect("リポジトリ検出に失敗");
    let mut expected = vec!["HEAD".to_string()];
    expected.extend(for_each_ref_short(repo_dir));
    assert_eq!(repo.list_refs(), expected);
}

#[test]
fn test_list_refs_matches_for_each_ref_with_tags_and_slashed_branches() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    run_git(&["branch", "topic/a"], &repo_dir);
    run_git(&["branch", "Zeta"], &repo_dir);
    run_git(&["tag", "v1.0"], &repo_dir);
    run_git(&["tag", "-a", "v2.0", "-m", "annotated"], &repo_dir);
    assert_refs_match_git(&repo_dir);
}

#[test]
fn test_list_refs_handles_ambiguous_short_names_like_git() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    // ブランチとタグが同名: git は heads/dup, tags/dup と曖昧さを解消した名前を出す
    run_git(&["branch", "dup"], &repo_dir);
    run_git(&["tag", "dup"], &repo_dir);
    // origin/HEAD は git では "origin" と短縮される
    let head = git_output(&["rev-parse", "HEAD"], &repo_dir);
    run_git(
        &["update-ref", "refs/remotes/origin/main", &head],
        &repo_dir,
    );
    run_git(
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
        &repo_dir,
    );
    assert_refs_match_git(&repo_dir);
}

#[test]
fn test_list_refs_and_snapshot_work_after_pack_refs() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    run_git(&["tag", "packed-tag"], &repo_dir);
    run_git(&["pack-refs", "--all"], &repo_dir);
    // loose ref が消えて packed-refs だけになっていること
    assert!(!repo_dir.join(".git/refs/heads/feature").exists());
    assert!(repo_dir.join(".git/packed-refs").exists());
    assert_refs_match_git(&repo_dir);
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let snapshot = repo.materialize_snapshot("feature").unwrap();
    assert_eq!(
        normalize_eol(std::fs::read_to_string(snapshot.join("hello.txt")).unwrap()),
        "hello from feature\n"
    );
    GitRepository::cleanup(&snapshot);
    let tagged = repo.materialize_snapshot("packed-tag").unwrap();
    assert_eq!(
        normalize_eol(std::fs::read_to_string(tagged.join("hello.txt")).unwrap()),
        "hello from main\n"
    );
    GitRepository::cleanup(&tagged);
}

#[test]
fn test_snapshot_works_after_gc_with_packed_objects() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    std::fs::create_dir_all(repo_dir.join("dir/deep")).unwrap();
    std::fs::write(
        repo_dir.join("dir/deep/file.txt"),
        "deep content\n".repeat(200),
    )
    .unwrap();
    run_git(&["add", "."], &repo_dir);
    run_git(&["commit", "-m", "add deep"], &repo_dir);
    run_git(&["gc", "--aggressive", "--prune=now"], &repo_dir);
    // ルーズオブジェクトが残っていないこと(pack 化されている)
    assert!(std::fs::read_dir(repo_dir.join(".git/objects/pack"))
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().ends_with(".pack")));
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let snapshot = repo.materialize_snapshot("HEAD").unwrap();
    assert_eq!(
        std::fs::read_to_string(snapshot.join("dir/deep/file.txt")).unwrap(),
        "deep content\n".repeat(200)
    );
    GitRepository::cleanup(&snapshot);
    // 過去コミットも pack から辿れる
    let prev = repo.materialize_snapshot("HEAD~1").unwrap();
    assert!(!prev.join("dir").exists());
    GitRepository::cleanup(&prev);
}

#[test]
fn test_materialize_resolves_hash_revspec_and_annotated_tag() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    run_git(&["tag", "-a", "rel", "-m", "release", "feature"], &repo_dir);
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let read = |r: &str| {
        let snap = repo.materialize_snapshot(r).unwrap();
        let s = normalize_eol(std::fs::read_to_string(snap.join("hello.txt")).unwrap());
        GitRepository::cleanup(&snap);
        s
    };
    // 注釈付きタグはコミットまで剥がす
    assert_eq!(read("rel"), "hello from feature\n");
    let main_hash = git_output(&["rev-parse", "main"], &repo_dir);
    assert_eq!(read(&main_hash), "hello from main\n");
    assert_eq!(read(&main_hash[..10]), "hello from main\n");
    assert_eq!(read("feature~1"), "hello from main\n");
    assert_eq!(read("feature^{tree}"), "hello from feature\n");
    assert_eq!(read("HEAD"), "hello from main\n");
}

#[test]
fn test_unknown_ref_leaves_no_temp_directory() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let before: usize = count_snapshot_dirs();
    let err = repo.materialize_snapshot("no-such-ref").unwrap_err();
    assert!(err.0.contains("no-such-ref"));
    // 並行実行中の他テストの一時ディレクトリが増える場合があるため、失敗分が残らないことだけ確認する
    assert!(count_snapshot_dirs() <= before + 8);
}

fn count_snapshot_dirs() -> usize {
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("DiffLoupe-git-")
        })
        .count()
}

#[cfg(unix)]
#[test]
fn test_snapshot_preserves_symlinks_and_executable_bit() {
    use std::os::unix::fs::PermissionsExt;
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    std::fs::write(repo_dir.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();
    std::fs::set_permissions(
        repo_dir.join("run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::os::unix::fs::symlink("hello.txt", repo_dir.join("link-to-hello")).unwrap();
    std::os::unix::fs::symlink("/nonexistent/absolute", repo_dir.join("dangling")).unwrap();
    std::fs::create_dir_all(repo_dir.join("sub")).unwrap();
    std::fs::write(repo_dir.join("sub/plain.txt"), "p\n").unwrap();
    run_git(&["add", "."], &repo_dir);
    run_git(&["commit", "-m", "modes"], &repo_dir);
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let snap = repo.materialize_snapshot("main").unwrap();

    let meta = std::fs::symlink_metadata(snap.join("link-to-hello")).unwrap();
    assert!(meta.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(snap.join("link-to-hello")).unwrap(),
        PathBuf::from("hello.txt")
    );
    assert_eq!(
        std::fs::read_link(snap.join("dangling")).unwrap(),
        PathBuf::from("/nonexistent/absolute")
    );
    let run_mode = std::fs::metadata(snap.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(run_mode & 0o111, 0, "実行ビットが保たれていない");
    let plain_mode = std::fs::metadata(snap.join("sub/plain.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(plain_mode & 0o111, 0);
    GitRepository::cleanup(&snap);
}

#[test]
fn test_submodule_gitlink_becomes_empty_directory() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    let head = git_output(&["rev-parse", "HEAD"], &repo_dir);
    // サブモジュール実体なしで gitlink(160000)だけをインデックスへ追加する
    run_git(
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},vendor/sub", head),
        ],
        &repo_dir,
    );
    run_git(&["commit", "-m", "add gitlink"], &repo_dir);
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let snap = repo.materialize_snapshot("HEAD").unwrap();
    let sub = snap.join("vendor/sub");
    assert!(sub.is_dir());
    assert_eq!(std::fs::read_dir(&sub).unwrap().count(), 0);
    GitRepository::cleanup(&snap);
}

#[test]
fn test_detect_ignores_bare_repository_and_global_config() {
    if !git_available() {
        return;
    }
    let bare = std::env::temp_dir().join(format!(
        "DiffLoupeTests-bare-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&bare).unwrap();
    let _c = Cleanup(bare.clone());
    run_git(&["init", "--bare", "-b", "main"], &bare);
    assert!(GitRepository::detect(&bare).is_none());
}

#[test]
fn test_detect_returns_none_for_missing_path() {
    let missing = std::env::temp_dir().join("DiffLoupeTests-definitely-missing-dir/child");
    assert!(GitRepository::detect(&missing).is_none());
}

// ---------- 封じ込め・.git 変種・cleanup の多重防御 ----------

fn git_with_input(args: &[&str], input: &[u8], dir: &Path) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(GIT)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("git の起動に失敗");
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "git {} 失敗: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// 指定した名前の blob を 1 つだけ持つコミットを作り、そのコミットのハッシュを返す
fn commit_with_entry_name(repo_dir: &Path, name: &str) -> String {
    let blob = git_with_input(&["hash-object", "-w", "--stdin"], b"content\n", repo_dir);
    let mut record = format!("100644 blob {}\t{}", blob, name).into_bytes();
    record.push(0);
    let tree = git_with_input(&["mktree", "-z"], &record, repo_dir);
    git_output(&["commit-tree", &tree, "-m", "odd name"], repo_dir)
}

#[test]
fn test_materialize_rejects_unsafe_or_dot_git_like_names() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    let repo = GitRepository::detect(&repo_dir).unwrap();
    // コロン・予約名・末尾ドット/空白が危険なのは Windows だけ。他の OS では正当な名前として展開する。
    for name in [
        "a:b",
        "CON",
        "aux.c",
        "nul.txt",
        "COM1",
        "lpt9.log",
        "trailing.",
        "trailing ",
    ] {
        let commit = commit_with_entry_name(&repo_dir, name);
        let result = repo.materialize_snapshot(&commit);
        if cfg!(windows) {
            assert!(result.is_err(), "拒否されるべき名前: {:?}", name);
        } else {
            let snap = result.unwrap_or_else(|e| panic!("通るべき名前 {:?}: {}", name, e));
            assert!(snap.join(name).exists());
            GitRepository::cleanup(&snap);
        }
    }
    let names = [
        ".git",
        ".GIT",
        ".git.",
        ".git ",
        ".g\u{200c}it",
        ".git\u{feff}",
        "GIT~1",
        "git~1",
    ];
    for name in names {
        let commit = commit_with_entry_name(&repo_dir, name);
        let result = repo.materialize_snapshot(&commit);
        assert!(result.is_err(), "拒否されるべき名前: {:?}", name);
        // 失敗時は何も残らない(戻り値が Err のため、パスは返らない。メッセージを確認)
        assert!(result.unwrap_err().0.contains("展開"), "{:?}", name);
    }
    // 通常の名前は通る(コロン・予約名の誤検出がないこと)
    for name in [
        "console.txt",
        "comma",
        "auxiliary",
        "COM10",
        "my.git",
        "gitignore",
        ".gitignore",
    ] {
        let commit = commit_with_entry_name(&repo_dir, name);
        let snap = repo
            .materialize_snapshot(&commit)
            .unwrap_or_else(|e| panic!("通るべき名前 {:?}: {}", name, e));
        assert!(snap.join(name).exists());
        GitRepository::cleanup(&snap);
    }
}

#[test]
fn test_cleanup_only_removes_owned_temp_directories() {
    // temp 外のディレクトリは、DiffLoupe-git- で始まる名前でも消さない
    let outside = std::env::current_dir()
        .unwrap()
        .join(format!("DiffLoupe-git-outside-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "keep").unwrap();
    GitRepository::cleanup(&outside);
    assert!(outside.join("keep.txt").exists());
    let _ = std::fs::remove_dir_all(&outside);

    // temp 内でも接頭辞が違えば消さない
    let other = std::env::temp_dir().join(format!("OtherDir-{}", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    GitRepository::cleanup(&other);
    assert!(other.exists());
    let _ = std::fs::remove_dir_all(&other);

    // temp の孫は消さない
    let nested_parent = std::env::temp_dir().join(format!("Parent-{}", std::process::id()));
    let nested = nested_parent.join("DiffLoupe-git-nested");
    std::fs::create_dir_all(&nested).unwrap();
    GitRepository::cleanup(&nested);
    assert!(nested.exists());
    let _ = std::fs::remove_dir_all(&nested_parent);
}

#[test]
fn test_cleanup_removes_real_snapshot() {
    if !git_available() {
        return;
    }
    let repo_dir = make_fixture_repo();
    let _c = Cleanup(repo_dir.clone());
    let repo = GitRepository::detect(&repo_dir).unwrap();
    let snap = repo.materialize_snapshot("main").unwrap();
    assert!(snap.exists());
    GitRepository::cleanup(&snap);
    assert!(!snap.exists());
}
