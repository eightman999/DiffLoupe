//! git リポジトリ検出と ref の一時ディレクトリ実体化(Swift `GitSnapshot` 相当)
//!
//! 動作設計:
//! - 外部プロセス(git / tar)を一切起動せず、純 Rust の `gix` で .git を直接読む。
//!   App Sandbox 配下(外部実行不可)でも、git 未インストールの環境でも動く。
//! - リポジトリは「isolated」で開く。ユーザーのグローバル設定(~/.gitconfig、
//!   $XDG_CONFIG_HOME/git)、システム設定、環境変数(GIT_DIR 等)を読まず、
//!   所有者チェック(safe.directory)も行わない(読むだけで、書き込みや hook 実行をしないため)。
//! - `detect` は `git rev-parse --show-toplevel` 相当(bare リポジトリは None)。
//! - `list_refs` は先頭に "HEAD"、続けて `git for-each-ref --format=%(refname:short)` と
//!   同じ集合・同じ並び(refname 昇順)・同じ短縮規則(曖昧なときは長い名前のまま)を返す。
//! - `materialize_snapshot` は ref(ブランチ・タグ・HEAD・コミットハッシュ・`HEAD~1` 等の
//!   revspec)をツリーまで解決し、一時ディレクトリへ書き出す。通常ファイル、実行ビット(unix)、
//!   シンボリックリンク(unix。Windows では内容を通常ファイルとして書く)に対応し、
//!   サブモジュール(gitlink)は空ディレクトリにする。失敗時は作りかけを削除する。
//! - 資源上限: 書き出すエントリ数(200,000)と合計バイト数(8 GiB)に上限を設ける。blob は読み込む前に
//!   ヘッダでサイズを確認し、残り枠を超えるなら読まずに中止する。同じ tree を多重参照して
//!   展開物が指数的に膨らむ細工(git bomb)を止めるため。超過時は GitError で中止し、作りかけを削除する。
//! - 展開先の封じ込め: 名前が空・`.`・`..`・区切り文字を含む、`:` を含む、Windows 予約名
//!   (CON / NUL / COM1 等)、末尾がドットまたは空白のエントリは GitError で中止する。
//!   パス要素が `.git` の変種(大文字小文字、末尾のドット・空白、HFS+ が無視する文字を挟んだもの、
//!   NTFS 短縮名 `git~1`)のエントリも中止する(通常の tree には現れず、展開物を git 作業ツリーへ
//!   同期したとき hooks / config を上書きされないため)。
//! - `cleanup` は、一時ディレクトリ直下の `DiffLoupe-git-` で始まるディレクトリだけを削除する。
//! - `git archive` との既知の差: .gitattributes の `export-ignore`(除外)・`export-subst`
//!   (置換)・`eol` / `text`(改行変換)を適用しない。リポジトリに保存されたままの内容を書き出す。

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GitError(pub String);
impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for GitError {}

#[derive(Debug, Clone, PartialEq)]
pub struct GitRepository {
    pub toplevel: PathBuf,
}

/// 設定ファイル・環境変数・所有者チェックに依存しない形でリポジトリを探して開く
fn discover_isolated(path: &Path) -> Option<gix::Repository> {
    let abs = std::path::absolute(path).ok()?;
    let upwards = gix::discover::upwards::Options {
        trust: gix::discover::upwards::TrustPolicy::Assume(gix::sec::Trust::Full),
        ..Default::default()
    };
    gix::discover_opts(&abs, upwards, gix::open::Options::isolated()).ok()
}

fn open_isolated(toplevel: &Path) -> Result<gix::Repository, GitError> {
    let opts = gix::open::Options::isolated().with(gix::sec::Trust::Full);
    gix::open_opts(toplevel, opts)
        .map_err(|e| GitError(format!("リポジトリを開けませんでした: {}", e)))
}

impl GitRepository {
    /// 指定フォルダが git 作業ツリー内かを検出
    pub fn detect(path: &Path) -> Option<GitRepository> {
        let repo = discover_isolated(path)?;
        let workdir = repo.workdir()?.to_path_buf();
        // git rev-parse --show-toplevel は実パスを返すため、シンボリックリンクを解決する
        // (Windows の canonicalize は \\?\ 付きになるため行わない)
        #[cfg(not(windows))]
        let workdir = workdir.canonicalize().unwrap_or(workdir);
        Some(GitRepository { toplevel: workdir })
    }

    /// ブランチ・タグ一覧(先頭に HEAD)
    pub fn list_refs(&self) -> Vec<String> {
        let mut refs = vec!["HEAD".to_string()];
        if let Ok(repo) = open_isolated(&self.toplevel) {
            refs.extend(collect_short_refs(&repo));
        }
        refs
    }

    /// ref をツリーへ解決して一時ディレクトリに実体化
    pub fn materialize_snapshot(&self, git_ref: &str) -> Result<PathBuf, GitError> {
        let temp = crate::archive::create_unique_temp_dir("DiffLoupe-git-")
            .map_err(|e| GitError(format!("一時ディレクトリの作成に失敗しました: {}", e)))?;
        match self.write_tree(git_ref, &temp) {
            Ok(()) => Ok(temp),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&temp);
                Err(e)
            }
        }
    }

    fn write_tree(&self, git_ref: &str, destination: &Path) -> Result<(), GitError> {
        let repo = open_isolated(&self.toplevel)?;
        let fail = |detail: String| {
            GitError(format!(
                "ref '{}' の取得に失敗しました。{}",
                git_ref, detail
            ))
        };
        let id = repo
            .rev_parse_single(git_ref)
            .map_err(|_| fail("存在しないref、または解決できないrefです。".to_string()))?;
        let tree = id
            .object()
            .map_err(|e| fail(e.to_string()))?
            .peel_to_tree()
            .map_err(|e| fail(e.to_string()))?;
        let mut budget = Budget::new(crate::archive::MAX_ENTRIES, crate::archive::MAX_TOTAL_BYTES);
        write_tree_entries(&repo, &tree, destination, 0, &mut budget)
    }

    pub fn cleanup(path: &Path) {
        if crate::archive::is_owned_temp_dir(path, "DiffLoupe-git-") {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

const MAX_TREE_DEPTH: usize = 512;

/// 書き出したエントリ数・バイト数の合計と、その上限(git bomb 対策)
pub(crate) struct Budget {
    max_entries: usize,
    max_bytes: u64,
    entries: usize,
    bytes: u64,
}

impl Budget {
    pub(crate) fn new(max_entries: usize, max_bytes: u64) -> Self {
        Budget {
            max_entries,
            max_bytes,
            entries: 0,
            bytes: 0,
        }
    }

    fn add_entry(&mut self) -> Result<(), GitError> {
        self.entries += 1;
        if self.entries > self.max_entries {
            return Err(extract_error(format!(
                "エントリ数が多すぎます(上限 {} 件)",
                self.max_entries
            )));
        }
        Ok(())
    }

    /// 書き出す前にサイズを予約する。残り枠を超えるなら読み込まずに失敗する。
    fn reserve_bytes(&mut self, size: u64) -> Result<(), GitError> {
        if size > self.max_bytes - self.bytes {
            return Err(extract_error(format!(
                "展開後のサイズが大きすぎます(上限 {} バイト)",
                self.max_bytes
            )));
        }
        self.bytes += size;
        Ok(())
    }
}

fn extract_error(detail: impl std::fmt::Display) -> GitError {
    GitError(format!("スナップショットの展開に失敗しました。{}", detail))
}

/// blob をヘッダのサイズ確認を経てから読み込む
fn read_blob_within_budget(
    repo: &gix::Repository,
    oid: gix::ObjectId,
    budget: &mut Budget,
) -> Result<Vec<u8>, GitError> {
    let header = repo.find_header(oid).map_err(extract_error)?;
    budget.reserve_bytes(header.size())?;
    let blob = repo.find_blob(oid).map_err(extract_error)?;
    Ok(blob.data.clone())
}

pub(crate) fn write_tree_entries(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    dir: &Path,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), GitError> {
    use gix::object::tree::EntryKind;
    if depth > MAX_TREE_DEPTH {
        return Err(extract_error("ディレクトリの階層が深すぎます。"));
    }
    for entry in tree.iter() {
        let entry = entry.map_err(extract_error)?;
        budget.add_entry()?;
        let name = gix::path::from_bstr(entry.filename())
            .map_err(|_| extract_error("ファイル名をパスに変換できません。"))?;
        // パス区切り・"."・".." を含む名前は展開先の外へ出得るため拒否する
        let name_str = name.to_string_lossy();
        if name_str.is_empty()
            || name_str == "."
            || name_str == ".."
            || name_str.contains(['/', '\0'])
            || (cfg!(windows) && name_str.contains('\\'))
        {
            return Err(extract_error(format!("不正なファイル名です: {}", name_str)));
        }
        if crate::archive::is_dot_git_variant(&name_str) {
            return Err(extract_error(format!(
                ".git と紛らわしい名前のエントリがあります: {}",
                name_str
            )));
        }
        if let Some(reason) = crate::archive::unsafe_name_reason(&name_str) {
            return Err(extract_error(format!(
                "展開できない名前のエントリです({}): {}",
                reason, name_str
            )));
        }
        let target = dir.join(&*name);
        // 展開先ちょうど 1 階層下の通常の名前であること
        let contained = target.strip_prefix(dir).is_ok_and(|rest| {
            let mut comps = rest.components();
            matches!(comps.next(), Some(std::path::Component::Normal(_))) && comps.next().is_none()
        });
        if !contained {
            return Err(extract_error(format!(
                "展開先の外を指すファイル名です: {}",
                name_str
            )));
        }
        let oid = entry.oid().to_owned();
        match entry.mode().kind() {
            EntryKind::Tree => {
                std::fs::create_dir(&target).map_err(extract_error)?;
                let sub = repo
                    .find_object(oid)
                    .map_err(extract_error)?
                    .try_into_tree()
                    .map_err(extract_error)?;
                write_tree_entries(repo, &sub, &target, depth + 1, budget)?;
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let data = read_blob_within_budget(repo, oid, budget)?;
                write_file(
                    &target,
                    &data,
                    matches!(entry.mode().kind(), EntryKind::BlobExecutable),
                )?;
            }
            EntryKind::Link => {
                let data = read_blob_within_budget(repo, oid, budget)?;
                write_symlink(&target, &data)?;
            }
            // サブモジュール(gitlink)は空ディレクトリ
            EntryKind::Commit => {
                std::fs::create_dir(&target).map_err(extract_error)?;
            }
        }
    }
    Ok(())
}

/// 既存パスを辿らない(`create_new`)ように書き出す
fn write_file(target: &Path, data: &[u8], executable: bool) -> Result<(), GitError> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(if executable { 0o755 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    let mut file = opts.open(target).map_err(extract_error)?;
    file.write_all(data).map_err(extract_error)
}

#[cfg(unix)]
fn write_symlink(target: &Path, link_content: &[u8]) -> Result<(), GitError> {
    use std::os::unix::ffi::OsStrExt;
    let link_target = std::ffi::OsStr::from_bytes(link_content);
    std::os::unix::fs::symlink(link_target, target).map_err(extract_error)
}

/// Windows ではシンボリックリンク作成に権限が要るため、リンク先文字列を内容とする通常ファイルにする
#[cfg(not(unix))]
fn write_symlink(target: &Path, link_content: &[u8]) -> Result<(), GitError> {
    write_file(target, link_content, false)
}

/// `git for-each-ref --format=%(refname:short)` 相当(refname 昇順)
fn collect_short_refs(repo: &gix::Repository) -> Vec<String> {
    let mut full: Vec<String> = Vec::new();
    if let Ok(platform) = repo.references() {
        if let Ok(iter) = platform.all() {
            for r in iter.flatten() {
                full.push(r.name().as_bstr().to_string());
            }
        }
    }
    full.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    full.dedup();
    let set: std::collections::HashSet<&str> = full.iter().map(|s| s.as_str()).collect();
    full.iter().map(|name| shorten_ref(name, &set)).collect()
}

/// git の `shorten_unambiguous_ref`(strict。for-each-ref の既定)相当。短縮名が別の ref としても
/// 解釈できる場合は、より長い名前を使う。
fn shorten_ref(full: &str, all: &std::collections::HashSet<&str>) -> String {
    // 先頭は "%s"(そのまま)。末尾の規則から順に試す。
    const PREFIXES: [(&str, &str); 6] = [
        ("", ""),
        ("refs/", ""),
        ("refs/tags/", ""),
        ("refs/heads/", ""),
        ("refs/remotes/", ""),
        ("refs/remotes/", "/HEAD"),
    ];
    for i in (1..PREFIXES.len()).rev() {
        let (prefix, suffix) = PREFIXES[i];
        let Some(rest) = full.strip_prefix(prefix) else {
            continue;
        };
        let Some(short) = (if suffix.is_empty() {
            Some(rest)
        } else {
            rest.strip_suffix(suffix)
        }) else {
            continue;
        };
        if short.is_empty() {
            continue;
        }
        // for-each-ref は strict 判定: 自分自身以外の全規則で別 ref に解釈できれば曖昧
        let ambiguous = PREFIXES.iter().enumerate().any(|(j, (p, s))| {
            if j == i {
                return false;
            }
            let candidate = format!("{}{}{}", p, short, s);
            all.contains(candidate.as_str())
        });
        if !ambiguous {
            return short.to_string();
        }
    }
    full.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[cfg(not(windows))]
    const GIT: &str = "/usr/bin/git";
    #[cfg(windows)]
    const GIT: &str = "git";

    fn git(args: &[&str], dir: &Path) -> String {
        let out = Command::new(GIT)
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git の起動に失敗");
        assert!(out.status.success(), "git {} 失敗", args.join(" "));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// 100 バイトのファイルを 3 つ持つリポジトリで、上限を小さくして展開する
    #[test]
    fn budget_limits_stop_extraction() {
        if Command::new(GIT).arg("--version").output().is_err() {
            return;
        }
        let repo_dir = crate::archive::create_unique_temp_dir("DiffLoupeUnit-git-").unwrap();
        git(&["init", "-b", "main"], &repo_dir);
        git(&["config", "user.email", "t@example.com"], &repo_dir);
        git(&["config", "user.name", "t"], &repo_dir);
        for n in ["a", "b", "c"] {
            std::fs::write(repo_dir.join(n), "x".repeat(100)).unwrap();
        }
        git(&["add", "."], &repo_dir);
        git(&["commit", "-m", "c"], &repo_dir);
        let repo = open_isolated(&repo_dir).unwrap();
        let tree = repo
            .rev_parse_single("HEAD")
            .unwrap()
            .object()
            .unwrap()
            .peel_to_tree()
            .unwrap();
        let extract = |max_entries: usize, max_bytes: u64| {
            let dest = crate::archive::create_unique_temp_dir("DiffLoupeUnit-out-").unwrap();
            let mut budget = Budget::new(max_entries, max_bytes);
            let r = write_tree_entries(&repo, &tree, &dest, 0, &mut budget);
            let written = std::fs::read_dir(&dest).unwrap().count();
            let _ = std::fs::remove_dir_all(&dest);
            (r, written)
        };
        // 上限ちょうどは通る
        assert!(extract(3, 300).0.is_ok());
        // エントリ数超過
        let (r, written) = extract(2, 10_000);
        assert!(r.unwrap_err().0.contains("エントリ数が多すぎます"));
        assert!(written <= 2);
        // バイト数超過: 3 つ目の blob(残り枠 99 バイト)は読み込まずに中止
        let (r, written) = extract(100, 299);
        assert!(r.unwrap_err().0.contains("展開後のサイズが大きすぎます"));
        assert_eq!(written, 2);
        let _ = std::fs::remove_dir_all(&repo_dir);
    }

    /// 同じ tree を 2 回ずつ参照して 20 段重ねる(展開すると 2^20 ファイル)git bomb は、
    /// 小さいエントリ上限で早期に中止され、巨大な展開をしない。
    #[test]
    fn git_bomb_stops_at_entry_limit() {
        use std::io::Write;
        use std::process::Stdio;
        if Command::new(GIT).arg("--version").output().is_err() {
            return;
        }
        let repo_dir = crate::archive::create_unique_temp_dir("DiffLoupeUnit-bomb-").unwrap();
        git(&["init", "-b", "main"], &repo_dir);
        let with_input = |args: &[&str], input: String| {
            let mut child = Command::new(GIT)
                .args(args)
                .current_dir(&repo_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let blob = with_input(&["hash-object", "-w", "--stdin"], "x".to_string());
        let mut tree = with_input(
            &["mktree"],
            format!("100644 blob {b}\ta\n100644 blob {b}\tb\n", b = blob),
        );
        for _ in 0..20 {
            tree = with_input(
                &["mktree"],
                format!("040000 tree {t}\ta\n040000 tree {t}\tb\n", t = tree),
            );
        }
        let repo = open_isolated(&repo_dir).unwrap();
        let oid = gix::ObjectId::from_hex(tree.as_bytes()).unwrap();
        let tree = repo.find_object(oid).unwrap().try_into_tree().unwrap();
        let dest = crate::archive::create_unique_temp_dir("DiffLoupeUnit-bombout-").unwrap();
        let mut budget = Budget::new(1000, u64::MAX / 2);
        let started = std::time::Instant::now();
        let r = write_tree_entries(&repo, &tree, &dest, 0, &mut budget);
        assert!(r.unwrap_err().0.contains("エントリ数が多すぎます"));
        assert!(started.elapsed().as_secs() < 10);
        let _ = std::fs::remove_dir_all(&dest);
        let _ = std::fs::remove_dir_all(&repo_dir);
    }
}
