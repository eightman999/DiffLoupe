//! git リポジトリ検出と ref の一時ディレクトリ実体化(Swift `GitSnapshot` 相当)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// git/tar の実行ファイル名。macOS/Linux では Swift 版と同じ絶対パスを使う。
/// Windows では /usr/bin 配下は存在しないため PATH 検索に委ねる
/// (git for Windows の git.exe、Windows 10 以降標準の bsdtar (tar.exe) を想定)。
#[cfg(not(windows))]
const GIT_EXECUTABLE: &str = "/usr/bin/git";
#[cfg(windows)]
const GIT_EXECUTABLE: &str = "git";
#[cfg(not(windows))]
const TAR_EXECUTABLE: &str = "/usr/bin/tar";
#[cfg(windows)]
const TAR_EXECUTABLE: &str = "tar";

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

impl GitRepository {
    /// 指定フォルダが git 作業ツリー内かを検出
    pub fn detect(path: &Path) -> Option<GitRepository> {
        // Windows では GIT_EXECUTABLE は PATH 上のコマンド名であり、
        // 絶対パスではないため存在確認をスキップする(起動失敗時は None になる)
        #[cfg(not(windows))]
        if !Path::new(GIT_EXECUTABLE).exists() {
            return None;
        }
        let out = run_git(
            &[
                "-C",
                &path.to_string_lossy(),
                "rev-parse",
                "--show-toplevel",
            ],
            path,
        )?;
        let trimmed = out.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(GitRepository {
            toplevel: PathBuf::from(trimmed),
        })
    }

    /// ブランチ・タグ一覧(先頭に HEAD)
    pub fn list_refs(&self) -> Vec<String> {
        let mut refs = vec!["HEAD".to_string()];
        if let Some(out) = run_git(
            &[
                "-C",
                &self.toplevel.to_string_lossy(),
                "for-each-ref",
                "--format=%(refname:short)",
            ],
            &self.toplevel,
        ) {
            refs.extend(
                out.lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty()),
            );
        }
        refs
    }

    /// `git archive <ref> | tar -x` で一時ディレクトリに実体化
    pub fn materialize_snapshot(&self, git_ref: &str) -> Result<PathBuf, GitError> {
        let temp = std::env::temp_dir().join(format!("DiffLoupe-git-{}", uuid_simple()));
        std::fs::create_dir_all(&temp)
            .map_err(|e| GitError(format!("一時ディレクトリの作成に失敗しました: {}", e)))?;
        match self.archive_to_tar(git_ref, &temp) {
            Ok(()) => Ok(temp),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&temp);
                Err(e)
            }
        }
    }

    fn archive_to_tar(&self, git_ref: &str, destination: &Path) -> Result<(), GitError> {
        // git archive → stdout 取得 → tar stdin へ(シェル非経由)
        let archive = Command::new(GIT_EXECUTABLE)
            .args(["-C", &self.toplevel.to_string_lossy(), "archive", git_ref])
            .current_dir(&self.toplevel)
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| GitError(format!("gitまたはtarの起動に失敗しました: {}", e)))?;
        if !archive.status.success() {
            let message = String::from_utf8_lossy(&archive.stderr).trim().to_string();
            return Err(GitError(format!(
                "ref '{}' の取得に失敗しました。{}",
                git_ref,
                if message.is_empty() {
                    "存在しないref、またはgit archiveに失敗した可能性があります。"
                } else {
                    &message
                }
            )));
        }
        let mut tar = Command::new(TAR_EXECUTABLE)
            .args(["-x", "-C", &destination.to_string_lossy()])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GitError(format!("gitまたはtarの起動に失敗しました: {}", e)))?;
        tar.stdin
            .take()
            .unwrap()
            .write_all(&archive.stdout)
            .map_err(|e| GitError(format!("スナップショットの展開に失敗しました。{}", e)))?;
        let out = tar
            .wait_with_output()
            .map_err(|e| GitError(format!("スナップショットの展開に失敗しました。{}", e)))?;
        if !out.status.success() {
            let message = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(GitError(format!(
                "スナップショットの展開に失敗しました。{}",
                message
            )));
        }
        Ok(())
    }

    pub fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }
}

fn run_git(args: &[&str], cwd: &Path) -> Option<String> {
    let out = Command::new(GIT_EXECUTABLE)
        .args(args)
        .current_dir(cwd)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}{:x}", t, std::process::id())
}
