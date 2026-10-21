//! フォルダ比較エンジン(Swift `FolderComparator.swift` 相当)。
//! 走査 → 突き合わせ → SHA-256 並列ハッシュ → 正規化内容比較 →
//! リネーム検出 → ディレクトリ集約 → 結果構築。

use crate::cancel::{CancellationToken, Cancelled, EntryLimitExceeded};
use crate::filter::FilterSet;
use crate::models::*;
use crate::options::ContentComparison;
use crate::unicode::{localized_standard_compare, nfc};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

pub const HASH_CHUNK_SIZE: usize = 65536;

#[derive(Debug)]
pub enum CompareError {
    Cancelled,
    EntryLimitExceeded(EntryLimitExceeded),
    Io(String),
}

impl From<Cancelled> for CompareError {
    fn from(_: Cancelled) -> Self {
        CompareError::Cancelled
    }
}

#[derive(Debug, Clone)]
pub struct CompareOptions {
    /// 既定 true(GUI)。false でルート直下のみ
    pub recursive: bool,
    /// 既定除外(.git / .DS_Store / node_modules)
    pub use_default_excludes: bool,
    /// 件数上限(None で無制限)
    pub max_entries: Option<usize>,
    pub user_exclude_patterns: Vec<String>,
    pub user_include_patterns: Vec<String>,
    pub content_comparison: ContentComparison,
    pub detect_renames: bool,
    /// 正規化一括読込の上限(既定64MiB)
    pub max_normalization_compare_bytes: i64,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            use_default_excludes: true,
            max_entries: None,
            user_exclude_patterns: vec![],
            user_include_patterns: vec![],
            content_comparison: ContentComparison::default(),
            detect_renames: false,
            max_normalization_compare_bytes: 64 * 1024 * 1024,
        }
    }
}

impl CompareOptions {
    pub const FREE_MAX_ENTRIES: usize = 2000;
    fn excluded_names(&self) -> &'static [&'static str] {
        if self.use_default_excludes {
            &[".git", ".DS_Store", "node_modules"]
        } else {
            &[]
        }
    }
}

pub type ProgressHandler = dyn Fn(ComparisonProgress) + Send + Sync;

pub struct FolderComparator;

/// SHA-256 を 64KiB チャンクでストリーミング計算(FFI 経由でも使用するため公開)
pub fn sha256_file(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; HASH_CHUNK_SIZE];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

fn posix_meta(path: &Path) -> Option<PosixBasicMetadata> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let md = std::fs::metadata(path).ok()?;
        Some(PosixBasicMetadata {
            permissions: Some(md.mode() & 0o7777),
            owner_id: Some(md.uid()),
            group_id: Some(md.gid()),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

fn mtime_secs(path: &Path) -> Option<f64> {
    let md = std::fs::metadata(path).ok()?;
    md.modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

/// ワーカープールで targets を並列処理し、完了順に (idx, item) を流す
/// (std::thread::scope を使うため job/on_item は自由に借用できる)
fn run_parallel<T: Send>(
    total: usize,
    workers: usize,
    job: &(dyn Fn(usize) -> T + Send + Sync),
    on_item: &mut (dyn FnMut(usize, T) + Send),
) {
    let idx = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<(usize, T)>();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            let idx = &idx;
            let tx = tx.clone();
            scope.spawn(move || loop {
                let i = idx.fetch_add(1, Ordering::Relaxed);
                if i >= total {
                    break;
                }
                let r = job(i);
                if tx.send((i, r)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for (i, r) in rx {
            on_item(i, r);
        }
    });
}

impl FolderComparator {
    /// 左右2フォルダを比較する。
    /// `cancel` を立てると `CompareError::Cancelled` で中断する。
    pub fn compare(
        left: &Path,
        right: &Path,
        options: &CompareOptions,
        progress: Option<&ProgressHandler>,
        cancel: Option<&CancellationToken>,
    ) -> Result<ComparisonResult, CompareError> {
        let check = |r: Result<(), Cancelled>| r.map_err(CompareError::from);

        // フェーズ1: 走査
        let mut found = 0usize;
        let left_map =
            Self::scan_tree(left, options, &mut found, progress, cancel).map_err(|e| match e {
                CompareError::Io(s) => CompareError::Io(s),
                other => other,
            })?;
        let right_map = Self::scan_tree(right, options, &mut found, progress, cancel)?;
        if let Some(p) = progress {
            p(ComparisonProgress::Scanning { found });
        }

        // フェーズ2: 突き合わせと一次分類
        let mut status_by_path: HashMap<String, DiffStatus> = HashMap::new();
        let mut error_by_path: HashMap<String, String> = HashMap::new();
        let mut hash_targets: Vec<(String, PathBuf, PathBuf)> = Vec::new();
        let mut content_targets: Vec<(String, PathBuf, PathBuf, i64, i64)> = Vec::new();
        let strict = options.content_comparison.is_strict();

        let all_keys: std::collections::BTreeSet<&String> =
            left_map.keys().chain(right_map.keys()).collect();
        for key in all_keys {
            match (left_map.get(key), right_map.get(key)) {
                (Some(_), None) => {
                    status_by_path.insert(key.clone(), DiffStatus::LeftOnly);
                }
                (None, Some(_)) => {
                    status_by_path.insert(key.clone(), DiffStatus::RightOnly);
                }
                (Some(l), Some(r)) => {
                    if l.is_directory != r.is_directory {
                        status_by_path.insert(key.clone(), DiffStatus::Different);
                    } else if l.is_directory {
                        status_by_path.insert(key.clone(), DiffStatus::Identical);
                    } else if strict {
                        if l.size != r.size {
                            status_by_path.insert(key.clone(), DiffStatus::Different);
                        } else {
                            hash_targets.push((key.clone(), l.path.clone(), r.path.clone()));
                        }
                    } else {
                        content_targets.push((
                            key.clone(),
                            l.path.clone(),
                            r.path.clone(),
                            l.size,
                            r.size,
                        ));
                    }
                }
                (None, None) => {}
            }
        }

        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);

        // フェーズ3a: ハッシュ比較
        if !hash_targets.is_empty() {
            if let Some(p) = progress {
                p(ComparisonProgress::Comparing {
                    done: 0,
                    total: hash_targets.len(),
                });
            }
            let total = hash_targets.len();
            let report_interval = (total / 100).max(1);
            let targets_ref = &hash_targets;
            let cancel_opt = cancel;
            let mut results: Vec<Option<(DiffStatus, Option<String>)>> = vec![None; total];
            {
                let results_cell = std::sync::Mutex::new(&mut results);
                let mut done = 0usize;
                run_parallel(
                    total,
                    workers,
                    &|i| {
                        if let Some(c) = cancel_opt {
                            if c.is_cancelled() {
                                return None;
                            }
                        }
                        let (_, l, r) = &targets_ref[i];
                        let r = (|| -> Result<DiffStatus, String> {
                            let lh = sha256_file(l).map_err(|e| e.to_string())?;
                            let rh = sha256_file(r).map_err(|e| e.to_string())?;
                            Ok(if lh == rh {
                                DiffStatus::Identical
                            } else {
                                DiffStatus::Different
                            })
                        })();
                        Some(match r {
                            Ok(s) => (s, None),
                            Err(msg) => (DiffStatus::Error, Some(msg)),
                        })
                    },
                    &mut |i, item| {
                        results_cell.lock().unwrap()[i] = item;
                        done += 1;
                        if done.is_multiple_of(report_interval) || done == total {
                            if let Some(p) = progress {
                                p(ComparisonProgress::Comparing { done, total });
                            }
                        }
                    },
                );
            }
            if let Some(c) = cancel_opt {
                c.check().map_err(CompareError::from)?;
            }
            for (i, item) in results.into_iter().enumerate() {
                if let Some((s, msg)) = item {
                    status_by_path.insert(hash_targets[i].0.clone(), s);
                    if let Some(m) = msg {
                        error_by_path.insert(hash_targets[i].0.clone(), m);
                    }
                }
            }
        }

        // フェーズ3b: 正規化内容比較
        if !content_targets.is_empty() {
            if let Some(p) = progress {
                p(ComparisonProgress::Comparing {
                    done: 0,
                    total: content_targets.len(),
                });
            }
            let total = content_targets.len();
            let report_interval = (total / 100).max(1);
            let cc = options.content_comparison;
            let max_bytes = options.max_normalization_compare_bytes;
            let targets_ref = &content_targets;
            let cancel_opt = cancel;
            let mut results: Vec<Option<(DiffStatus, Option<String>)>> = vec![None; total];
            {
                let results_cell = std::sync::Mutex::new(&mut results);
                let mut done = 0usize;
                run_parallel(
                    total,
                    workers,
                    &move |i| {
                        if let Some(c) = cancel_opt {
                            if c.is_cancelled() {
                                return None;
                            }
                        }
                        let (_, l, r, lsize, rsize) = &targets_ref[i];
                        let res = (|| -> Result<DiffStatus, String> {
                            if *lsize == *rsize && *lsize >= 0 {
                                let lh = sha256_file(l).map_err(|e| e.to_string())?;
                                let rh = sha256_file(r).map_err(|e| e.to_string())?;
                                if lh == rh {
                                    return Ok(DiffStatus::Identical);
                                }
                                if cc.is_strict() || *lsize > max_bytes {
                                    return Ok(DiffStatus::Different);
                                }
                            } else if cc.is_strict() || (*lsize).max(*rsize) > max_bytes {
                                return Ok(DiffStatus::Different);
                            }
                            let ldata = std::fs::read(l).map_err(|e| e.to_string())?;
                            let rdata = std::fs::read(r).map_err(|e| e.to_string())?;
                            let lstr = l.to_string_lossy().into_owned();
                            let rstr = r.to_string_lossy().into_owned();
                            if let Some((lt, rt)) = cc.structured_canonical_pair(
                                &ldata,
                                Some(&lstr),
                                &rdata,
                                Some(&rstr),
                            ) {
                                let mut c2 = cc;
                                c2.structured_mode =
                                    crate::structured::StructuredComparisonMode::Disabled;
                                let ln = c2.normalized_text(&lt).into_bytes();
                                let rn = c2.normalized_text(&rt).into_bytes();
                                return Ok(if ln == rn {
                                    DiffStatus::Identical
                                } else {
                                    DiffStatus::Different
                                });
                            }
                            let mut fb = cc;
                            fb.structured_mode =
                                crate::structured::StructuredComparisonMode::Disabled;
                            match (fb.normalized_data(&ldata), fb.normalized_data(&rdata)) {
                                (Some(a), Some(b)) => Ok(if a == b {
                                    DiffStatus::Identical
                                } else {
                                    DiffStatus::Different
                                }),
                                _ => Ok(if ldata == rdata {
                                    DiffStatus::Identical
                                } else {
                                    DiffStatus::Different
                                }),
                            }
                        })();
                        Some(match res {
                            Ok(s) => (s, None),
                            Err(msg) => (DiffStatus::Error, Some(msg)),
                        })
                    },
                    &mut |i, item| {
                        results_cell.lock().unwrap()[i] = item;
                        done += 1;
                        if done.is_multiple_of(report_interval) || done == total {
                            if let Some(p) = progress {
                                p(ComparisonProgress::Comparing { done, total });
                            }
                        }
                    },
                );
            }
            if let Some(c) = cancel_opt {
                c.check().map_err(CompareError::from)?;
            }
            for (i, item) in results.into_iter().enumerate() {
                if let Some((s, msg)) = item {
                    status_by_path.insert(content_targets[i].0.clone(), s);
                    if let Some(m) = msg {
                        error_by_path.insert(content_targets[i].0.clone(), m);
                    }
                }
            }
        }

        // フェーズ3.5: リネーム検出
        let mut rename_by_path: HashMap<String, RenameCandidate> = HashMap::new();
        if options.detect_renames {
            rename_by_path = Self::detect_renames(
                &left_map,
                &right_map,
                &status_by_path,
                progress,
                cancel,
                workers,
            )?;
        }

        if let Some(c) = cancel {
            check(c.check())?;
        }
        Self::propagate_to_ancestors(&mut status_by_path);

        Ok(Self::build_result(
            left,
            right,
            &left_map,
            &right_map,
            status_by_path,
            error_by_path,
            rename_by_path,
        ))
    }

    fn scan_tree(
        root: &Path,
        options: &CompareOptions,
        found: &mut usize,
        progress: Option<&ProgressHandler>,
        cancel: Option<&CancellationToken>,
    ) -> Result<HashMap<String, FileEntryMeta>, CompareError> {
        let filter = FilterSet::new(
            options.user_include_patterns.clone(),
            options.user_exclude_patterns.clone(),
        );
        let excluded = options.excluded_names();
        let mut result = HashMap::new();
        let mut stack: Vec<(PathBuf, String)> = vec![(root.to_path_buf(), String::new())];

        while let Some((dir, rel)) = stack.pop() {
            if let Some(c) = cancel {
                c.check()?;
            }
            let entries = std::fs::read_dir(&dir).map_err(|e| CompareError::Io(e.to_string()))?;
            for entry in entries {
                let entry = entry.map_err(|e| CompareError::Io(e.to_string()))?;
                let child = entry.path();
                let name_os = entry.file_name();
                let name = name_os.to_string_lossy().into_owned();
                if excluded.contains(&name.as_str()) {
                    continue;
                }
                let normalized_name = nfc(&name);
                let child_rel = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", rel, name)
                };
                let normalized_rel = nfc(&child_rel);
                if filter.is_excluded(&normalized_name, &normalized_rel) {
                    continue;
                }

                let ft = entry
                    .file_type()
                    .map_err(|e| CompareError::Io(e.to_string()))?;
                let is_symlink = ft.is_symlink();
                let posix = posix_meta(&child);
                let mut mtime = mtime_secs(&child);

                if is_symlink {
                    // リンク先がディレクトリなら記録しない
                    let target_md = std::fs::metadata(&child).ok();
                    if target_md.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    if !filter.allows_file(&normalized_name, &normalized_rel) {
                        continue;
                    }
                    let size = target_md.map(|m| m.len() as i64).unwrap_or(-1);
                    result.insert(
                        normalized_rel.clone(),
                        FileEntryMeta {
                            path: child,
                            is_directory: false,
                            size,
                            is_symlink: true,
                            mtime,
                            posix,
                        },
                    );
                } else if ft.is_dir() {
                    result.insert(
                        normalized_rel.clone(),
                        FileEntryMeta {
                            path: child.clone(),
                            is_directory: true,
                            size: -1,
                            is_symlink: false,
                            mtime,
                            posix,
                        },
                    );
                    if options.recursive {
                        stack.push((child, normalized_rel));
                    }
                } else {
                    if !filter.allows_file(&normalized_name, &normalized_rel) {
                        continue;
                    }
                    let size = entry.metadata().map(|m| m.len() as i64).unwrap_or(-1);
                    result.insert(
                        normalized_rel.clone(),
                        FileEntryMeta {
                            path: child,
                            is_directory: false,
                            size,
                            is_symlink: false,
                            mtime,
                            posix,
                        },
                    );
                }

                *found += 1;
                if let Some(max) = options.max_entries {
                    if *found > max {
                        return Err(CompareError::EntryLimitExceeded(EntryLimitExceeded {
                            limit: max,
                            observed: *found,
                        }));
                    }
                }
                if (*found).is_multiple_of(256) {
                    if let Some(p) = progress {
                        p(ComparisonProgress::Scanning { found: *found });
                    }
                }
                let _ = &mut mtime;
            }
        }
        Ok(result)
    }

    fn detect_renames(
        left_map: &HashMap<String, FileEntryMeta>,
        right_map: &HashMap<String, FileEntryMeta>,
        status_by_path: &HashMap<String, DiffStatus>,
        progress: Option<&ProgressHandler>,
        cancel: Option<&CancellationToken>,
        workers: usize,
    ) -> Result<HashMap<String, RenameCandidate>, CompareError> {
        let left_files: Vec<&String> = left_map
            .keys()
            .filter(|k| {
                status_by_path.get(*k) == Some(&DiffStatus::LeftOnly) && !left_map[*k].is_directory
            })
            .collect();
        let right_files: Vec<&String> = right_map
            .keys()
            .filter(|k| {
                status_by_path.get(*k) == Some(&DiffStatus::RightOnly)
                    && !right_map[*k].is_directory
            })
            .collect();
        let mut result = HashMap::new();
        if left_files.is_empty() || right_files.is_empty() {
            return Ok(result);
        }

        let mut targets: Vec<(bool, String, PathBuf)> = Vec::new();
        for k in &left_files {
            targets.push((true, (*k).clone(), left_map[*k].path.clone()));
        }
        for k in &right_files {
            targets.push((false, (*k).clone(), right_map[*k].path.clone()));
        }
        let total = targets.len();
        if let Some(p) = progress {
            p(ComparisonProgress::Renames { done: 0, total });
        }
        let report_interval = (total / 100).max(1);
        let cancel_opt = cancel;
        type HashJob = (bool, String, Option<[u8; 32]>);
        let mut hashes: Vec<Option<HashJob>> = vec![None; total];
        {
            let cell = std::sync::Mutex::new(&mut hashes);
            let mut done = 0usize;
            run_parallel(
                total,
                workers,
                &|i| {
                    if let Some(c) = cancel_opt {
                        if c.is_cancelled() {
                            return None;
                        }
                    }
                    let (is_left, key, path) = &targets[i];
                    Some((*is_left, key.clone(), sha256_file(path).ok()))
                },
                &mut |i, item| {
                    cell.lock().unwrap()[i] = item;
                    done += 1;
                    if done.is_multiple_of(report_interval) || done == total {
                        if let Some(p) = progress {
                            p(ComparisonProgress::Renames { done, total });
                        }
                    }
                },
            );
        }
        if let Some(c) = cancel_opt {
            c.check()?;
        }

        let mut left_by_hash: BTreeMap<[u8; 32], Vec<String>> = BTreeMap::new();
        let mut right_by_hash: BTreeMap<[u8; 32], Vec<String>> = BTreeMap::new();
        for item in hashes.into_iter().flatten() {
            let (is_left, key, hash) = item;
            if let Some(h) = hash {
                if is_left {
                    left_by_hash.entry(h).or_default().push(key);
                } else {
                    right_by_hash.entry(h).or_default().push(key);
                }
            }
        }
        for (h, lps) in &left_by_hash {
            let Some(rps) = right_by_hash.get(h) else {
                continue;
            };
            if rps.is_empty() {
                continue;
            }
            let unique = lps.len() == 1 && rps.len() == 1;
            let mut sl = lps.clone();
            sl.sort();
            let mut sr = rps.clone();
            sr.sort();
            for lp in &sl {
                result.insert(
                    lp.clone(),
                    RenameCandidate {
                        counterpart_path: sr[0].clone(),
                        is_unique_match: unique,
                        total_matches: sr.len(),
                    },
                );
            }
            for rp in &sr {
                result.insert(
                    rp.clone(),
                    RenameCandidate {
                        counterpart_path: sl[0].clone(),
                        is_unique_match: unique,
                        total_matches: sl.len(),
                    },
                );
            }
        }
        Ok(result)
    }

    fn propagate_to_ancestors(status_by_path: &mut HashMap<String, DiffStatus>) {
        let snapshot: Vec<String> = status_by_path
            .iter()
            .filter(|(_, s)| **s != DiffStatus::Identical)
            .map(|(k, _)| k.clone())
            .collect();
        for path in snapshot {
            let mut current = path.clone();
            while let Some(idx) = current.rfind('/') {
                current.truncate(idx);
                match status_by_path.get(&current) {
                    None => break,
                    Some(&DiffStatus::Identical) => {
                        status_by_path.insert(current.clone(), DiffStatus::Different);
                    }
                    Some(_) => break,
                }
            }
        }
    }

    fn build_result(
        left: &Path,
        right: &Path,
        left_map: &HashMap<String, FileEntryMeta>,
        right_map: &HashMap<String, FileEntryMeta>,
        status_by_path: HashMap<String, DiffStatus>,
        error_by_path: HashMap<String, String>,
        rename_by_path: HashMap<String, RenameCandidate>,
    ) -> ComparisonResult {
        let mut entries = Vec::with_capacity(status_by_path.len());
        let mut summary = ComparisonSummary::default();
        for (path, status) in &status_by_path {
            let lm = left_map.get(path);
            let rm = right_map.get(path);
            let is_dir = lm.map(|m| m.is_directory).unwrap_or(false)
                || rm.map(|m| m.is_directory).unwrap_or(false);
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            entries.push(ComparedEntry {
                relative_path: path.clone(),
                name,
                is_directory: is_dir,
                status: *status,
                left_path: lm.map(|m| m.path.clone()),
                right_path: rm.map(|m| m.path.clone()),
                left_size: lm.map(|m| m.size).unwrap_or(-1),
                right_size: rm.map(|m| m.size).unwrap_or(-1),
                error_message: error_by_path.get(path).cloned(),
                left_modification_date: lm.and_then(|m| m.mtime),
                right_modification_date: rm.and_then(|m| m.mtime),
                left_posix: lm.and_then(|m| m.posix),
                right_posix: rm.and_then(|m| m.posix),
                rename_candidate: rename_by_path.get(path).cloned(),
            });
            if !is_dir {
                match status {
                    DiffStatus::Identical => summary.identical_files += 1,
                    DiffStatus::Different => summary.different_files += 1,
                    DiffStatus::LeftOnly => summary.left_only_files += 1,
                    DiffStatus::RightOnly => summary.right_only_files += 1,
                    DiffStatus::Error => summary.error_files += 1,
                }
            }
        }
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let root_node = Self::build_tree(&entries);
        ComparisonResult {
            left_root: crate::paths::canonical_display(left),
            right_root: crate::paths::canonical_display(right),
            entries,
            root_node,
            summary,
        }
    }

    /// フラットエントリからツリー構築。同一階層はディレクトリ優先・名前昇順
    pub fn build_tree(entries: &[ComparedEntry]) -> DiffNode {
        #[derive(Default)]
        struct Builder {
            name: String,
            path: String,
            status: Option<DiffStatus>,
            is_directory: bool,
            children: BTreeMap<String, Builder>,
        }
        let mut root = Builder::default();
        for e in entries {
            let mut node = &mut root;
            let mut path_so_far = String::new();
            for comp in e.relative_path.split('/') {
                if !path_so_far.is_empty() {
                    path_so_far.push('/');
                }
                path_so_far.push_str(comp);
                let p = path_so_far.clone();
                let c = comp.to_string();
                node = node.children.entry(c.clone()).or_insert_with(|| Builder {
                    name: c,
                    path: p,
                    ..Default::default()
                });
            }
            node.status = Some(e.status);
            node.is_directory = e.is_directory;
        }
        fn convert(b: Builder) -> DiffNode {
            let has_children = !b.children.is_empty();
            let is_dir = b.is_directory || has_children;
            let children = if is_dir || has_children {
                let mut v: Vec<DiffNode> = b.children.into_values().map(convert).collect();
                v.sort_by(|a, b| {
                    a.is_directory
                        .cmp(&b.is_directory)
                        .reverse()
                        .then_with(|| localized_standard_compare(&a.name, &b.name))
                });
                Some(v)
            } else {
                None
            };
            DiffNode {
                id: b.path,
                name: b.name,
                is_directory: is_dir,
                status: b.status.unwrap_or(DiffStatus::Different),
                children,
            }
        }
        let mut children: Vec<DiffNode> = root.children.into_values().map(convert).collect();
        children.sort_by(|a, b| {
            a.is_directory
                .cmp(&b.is_directory)
                .reverse()
                .then_with(|| localized_standard_compare(&a.name, &b.name))
        });
        let root_status = if children.iter().all(|c| c.status == DiffStatus::Identical) {
            DiffStatus::Identical
        } else {
            DiffStatus::Different
        };
        DiffNode {
            id: String::new(),
            name: String::new(),
            is_directory: true,
            status: root_status,
            children: Some(children),
        }
    }
}
