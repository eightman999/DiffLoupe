//! ポーリング式ディレクトリ監視(Swift `DirectoryWatcher` 相当)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotEntry {
    pub relative_path: String,
    pub is_directory: bool,
    pub file_size: i64,
    /// mtime(Swift `Date` 相当の高精度値)。ディレクトリは None。
    /// 表示用丸め(ISO8601)とは別に、変更検出は生の解像度で比較する
    pub modification_date: Option<std::time::SystemTime>,
    pub is_symbolic_link: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectorySnapshot {
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Debug)]
pub enum WatchError {
    RootIsNotDirectory(String),
    UnreadableRoot(String),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootIsNotDirectory(p) => write!(f, "root is not a directory: {}", p),
            Self::UnreadableRoot(p) => write!(f, "unreadable root: {}", p),
        }
    }
}
impl std::error::Error for WatchError {}

fn normalize_path(p: &Path) -> String {
    // standardizedFileURL 相当: "." と ".." の解決
    let mut out: Vec<String> = Vec::new();
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    for c in abs.components() {
        use std::path::Component::*;
        match c {
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            RootDir => out.clear(),
            Normal(s) => out.push(s.to_string_lossy().into_owned()),
            Prefix(_) => {}
        }
    }
    format!("/{}", out.join("/"))
}

impl DirectorySnapshot {
    pub fn scan(root: &Path, excluded_names: &[&str]) -> Result<DirectorySnapshot, WatchError> {
        let root_path = normalize_path(root);
        if !root.is_dir() {
            return Err(WatchError::RootIsNotDirectory(root_path));
        }
        let mut entries = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let rd = std::fs::read_dir(&dir)
                .map_err(|_| WatchError::UnreadableRoot(root_path.clone()))?;
            for entry in rd.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                let ft = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let is_symlink = ft.is_symlink();
                let is_dir = ft.is_dir();
                if excluded_names.contains(&name.as_str()) {
                    // ディレクトリ/symlink はサブツリーごと枝刈り
                    continue;
                }
                let md = if is_symlink {
                    std::fs::symlink_metadata(&path).ok()
                } else {
                    entry.metadata().ok()
                };
                // Windows では root_path が "/Users/..."(ドライブ無し)に正規化されるため
                // strip_prefix が必ず失敗し、相対パスが名前だけになっていた。実際の root で再試行する。
                let rel = path
                    .strip_prefix(&root_path)
                    .ok()
                    .or_else(|| {
                        if cfg!(windows) {
                            path.strip_prefix(root).ok()
                        } else {
                            None
                        }
                    })
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|| name.clone());
                entries.push(SnapshotEntry {
                    relative_path: rel,
                    is_directory: is_dir,
                    file_size: if is_dir {
                        0
                    } else {
                        md.as_ref().map(|m| m.len() as i64).unwrap_or(0)
                    },
                    modification_date: if is_dir {
                        None
                    } else {
                        md.and_then(|m| m.modified().ok())
                    },
                    is_symbolic_link: is_symlink,
                });
                if is_dir && !is_symlink {
                    stack.push(path);
                }
            }
        }
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(DirectorySnapshot { entries })
    }
}

/// ポーリング監視器。interval ごとに再スキャンし、差分があれば on_change を一度呼ぶ。
pub struct DirectoryWatcher {
    roots: Vec<PathBuf>,
    interval: Duration,
    excluded_names: Vec<String>,
    on_change: Arc<dyn Fn() + Send + Sync>,
    state: Arc<Mutex<WatchState>>,
    running: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct WatchState {
    last: std::collections::HashMap<String, DirectorySnapshot>,
    baseline_ready: bool,
}

impl DirectoryWatcher {
    pub fn new(
        roots: Vec<PathBuf>,
        interval: Duration,
        excluded_names: Vec<String>,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            roots,
            interval: interval.max(Duration::from_millis(10)),
            excluded_names,
            on_change: Arc::new(on_change),
            state: Arc::new(Mutex::new(WatchState {
                last: Default::default(),
                baseline_ready: false,
            })),
            running: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            handle: Mutex::new(None),
        }
    }

    pub fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        {
            let mut st = self.state.lock().unwrap();
            st.last.clear();
            st.baseline_ready = false;
        }
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let roots = self.roots.clone();
        let excluded = self.excluded_names.clone();
        let state = self.state.clone();
        let running = self.running.clone();
        let generation = self.generation.clone();
        let interval = self.interval;
        let on_change = self.on_change.clone();
        *self.handle.lock().unwrap() = Some(std::thread::spawn(move || {
            // 初回 baseline(非同期)
            {
                let mut snaps = std::collections::HashMap::new();
                for root in &roots {
                    if let Ok(s) = DirectorySnapshot::scan(
                        root,
                        &excluded.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    ) {
                        snaps.insert(root.to_string_lossy().into_owned(), s);
                    }
                }
                let mut st = state.lock().unwrap();
                if !running.load(Ordering::SeqCst) || generation.load(Ordering::SeqCst) != gen {
                    return;
                }
                st.last = snaps;
                st.baseline_ready = true;
            }
            while running.load(Ordering::SeqCst) {
                std::thread::sleep(interval);
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                if generation.load(Ordering::SeqCst) != gen {
                    break;
                }
                if !state.lock().unwrap().baseline_ready {
                    continue;
                }
                let mut changed = false;
                for root in &roots {
                    let ex: Vec<&str> = excluded.iter().map(|s| s.as_str()).collect();
                    let Ok(snap) = DirectorySnapshot::scan(root, &ex) else {
                        continue;
                    };
                    let mut st = state.lock().unwrap();
                    let prev = st
                        .last
                        .insert(root.to_string_lossy().into_owned(), snap.clone());
                    drop(st);
                    if prev != Some(snap) {
                        changed = true;
                    }
                }
                if changed && running.load(Ordering::SeqCst) {
                    on_change();
                }
            }
        }));
    }

    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
        let mut st = self.state.lock().unwrap();
        st.last.clear();
        st.baseline_ready = false;
    }
}

impl Drop for DirectoryWatcher {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}
