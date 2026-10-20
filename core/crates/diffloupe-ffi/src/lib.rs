//! diffloupe-ffi: `diffloupe-core` の UniFFI バインディング。
//! Swift `DiffLoupeCore` モジュールが Swift UI/CLI/テスト/Pro に提供している
//! API をすべてカバーする(比較・フィルタ・テキスト/行内/表示差分・構造化・
//! Hex・コピー計画・3-way・git・監視・レポート)。
//! 関数は `diffloupe_*` プレフィックスで、Swift 側ファサードから呼ばれる。

pub mod types;
pub use types::*;

use diffloupe_core::{
    cancel::CancellationToken,
    comparator::{CompareError, FolderComparator, ProgressHandler},
    diff_display, filter, git_snapshot, glob::GlobPattern, hex_diff, inline_diff, line_diff,
    report, structured, sync_plan, text_loader, three_way, watcher,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

uniffi::setup_scaffolding!();

// ==================== エラー型 ====================

/// フォルダ比較の失敗。Cancelled / EntryLimitExceeded / Io。
#[derive(Debug, uniffi::Error)]
pub enum FfiCompareError {
    Cancelled,
    EntryLimitExceeded { limit: u64, observed: u64 },
    Io { message: String },
}

impl std::fmt::Display for FfiCompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "cancelled"),
            Self::EntryLimitExceeded { limit, observed } => {
                write!(f, "entry limit exceeded: limit={} observed={}", limit, observed)
            }
            Self::Io { message } => write!(f, "{}", message),
        }
    }
}
impl std::error::Error for FfiCompareError {}

impl From<CompareError> for FfiCompareError {
    fn from(e: CompareError) -> Self {
        match e {
            CompareError::Cancelled => Self::Cancelled,
            CompareError::EntryLimitExceeded(x) => Self::EntryLimitExceeded {
                limit: x.limit as u64,
                observed: x.observed as u64,
            },
            CompareError::Io(s) => Self::Io { message: s },
        }
    }
}

/// 汎用 IO 失敗(テキスト読み込み・Hex・sha256 等)
#[derive(Debug, uniffi::Error)]
pub enum FfiIoError {
    Io { message: String },
}
impl std::fmt::Display for FfiIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { message } => write!(f, "{}", message),
        }
    }
}
impl std::error::Error for FfiIoError {}
impl FfiIoError {
    fn io(e: std::io::Error) -> Self {
        Self::Io {
            message: e.to_string(),
        }
    }
}

/// git 操作の失敗(メッセージはユーザー向け)
#[derive(Debug, uniffi::Error)]
pub enum FfiGitError {
    Git { message: String },
}
impl std::fmt::Display for FfiGitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git { message } => write!(f, "{}", message),
        }
    }
}
impl std::error::Error for FfiGitError {}
impl From<git_snapshot::GitError> for FfiGitError {
    fn from(e: git_snapshot::GitError) -> Self {
        Self::Git { message: e.0 }
    }
}

/// スナップショット走査の失敗(Swift `DirectoryWatcherError` 相当)
#[derive(Debug, uniffi::Error)]
pub enum FfiWatchError {
    RootIsNotDirectory { path: String },
    UnreadableRoot { path: String },
}
impl std::fmt::Display for FfiWatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootIsNotDirectory { path } => write!(f, "root is not a directory: {}", path),
            Self::UnreadableRoot { path } => write!(f, "unreadable root: {}", path),
        }
    }
}
impl std::error::Error for FfiWatchError {}
impl From<watcher::WatchError> for FfiWatchError {
    fn from(e: watcher::WatchError) -> Self {
        match e {
            watcher::WatchError::RootIsNotDirectory(p) => Self::RootIsNotDirectory { path: p },
            watcher::WatchError::UnreadableRoot(p) => Self::UnreadableRoot { path: p },
        }
    }
}

// ==================== キャンセル ====================

#[derive(uniffi::Object)]
pub struct FfiCancellationToken {
    inner: CancellationToken,
}

#[uniffi::export]
impl FfiCancellationToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: CancellationToken::new(),
        })
    }

    pub fn cancel(&self) {
        self.inner.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

// ==================== 進捗コールバック ====================

/// 比較進捗の通知先。Swift 側でクロージャを包むクラスが実装する。
#[uniffi::export(with_foreign)]
pub trait FfiProgressCallback: Send + Sync {
    fn on_progress(&self, progress: FfiComparisonProgress);
}

// ==================== フォルダ比較 ====================

/// 左右2フォルダを比較する(Swift `FolderComparator.compare` 相当)。
/// 呼び出しスレッドをブロックする同期関数。Swift 側はバックグラウンドで呼ぶ。
#[uniffi::export]
pub fn diffloupe_compare_folders(
    left_path: String,
    right_path: String,
    options: FfiCompareOptions,
    progress: Option<Arc<dyn FfiProgressCallback>>,
    cancel: Option<Arc<FfiCancellationToken>>,
) -> Result<FfiComparisonResult, FfiCompareError> {
    let opts: diffloupe_core::comparator::CompareOptions = options.into();
    let progress_handler: Option<Box<ProgressHandler>> = progress.map(|cb| {
        Box::new(move |p: diffloupe_core::models::ComparisonProgress| {
            cb.on_progress(p.into())
        }) as Box<ProgressHandler>
    });
    let handler: Option<&ProgressHandler> = progress_handler.as_deref();
    let result = FolderComparator::compare(
        Path::new(&left_path),
        Path::new(&right_path),
        &opts,
        handler,
        cancel.as_deref().map(|t| &t.inner),
    )?;
    Ok(result.into())
}

/// `FolderComparator.Options.freeMaxEntries`(2000)
#[uniffi::export]
pub fn diffloupe_free_max_entries() -> u64 {
    diffloupe_core::comparator::CompareOptions::FREE_MAX_ENTRIES as u64
}

/// `FolderComparator.Options.defaultExcludedNames`
#[uniffi::export]
pub fn diffloupe_default_excluded_names() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".DS_Store".to_string(),
        "node_modules".to_string(),
    ]
}

/// フラットなエントリ列からツリーを構築(`FolderComparator.buildTree` 相当)
#[uniffi::export]
pub fn diffloupe_build_diff_tree(entries: Vec<FfiComparedEntry>) -> FfiDiffNode {
    let entries: Vec<diffloupe_core::models::ComparedEntry> =
        entries.into_iter().map(Into::into).collect();
    FolderComparator::build_tree(&entries).into()
}

// ==================== テキスト読み込み ====================

/// `TextFileLoader.load(url:)` 相当
#[uniffi::export]
pub fn diffloupe_load_text(path: String) -> Result<FfiTextLoadResult, FfiIoError> {
    let r = text_loader::load(Path::new(&path)).map_err(FfiIoError::io)?;
    Ok(match r {
        text_loader::TextLoadResult::Text(text, name) => FfiTextLoadResult::Text {
            text,
            encoding_name: name.to_string(),
        },
        text_loader::TextLoadResult::Binary => FfiTextLoadResult::Binary,
        text_loader::TextLoadResult::TooLarge => FfiTextLoadResult::TooLarge,
    })
}

/// `TextFileLoader.decode(_:)` 相当
#[uniffi::export]
pub fn diffloupe_decode_text(data: Vec<u8>) -> Option<FfiDecodedText> {
    text_loader::decode(&data).map(|(text, name)| FfiDecodedText {
        text,
        encoding_name: name.to_string(),
    })
}

// ==================== 行差分 ====================

#[uniffi::export]
pub fn diffloupe_split_lines(text: String) -> Vec<String> {
    line_diff::split_lines(&text)
}

#[uniffi::export]
pub fn diffloupe_split_lines_keeping_cr(text: String) -> Vec<String> {
    line_diff::split_lines_keeping_cr(&text)
}

/// `LineDiff.rows` 相当(上限超過は None)
#[uniffi::export]
pub fn diffloupe_line_diff_rows(
    left_text: String,
    right_text: String,
    content_comparison: FfiContentComparison,
) -> Option<Vec<FfiDiffRow>> {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    line_diff::rows(&left_text, &right_text, &cc)
        .map(|rows| rows.into_iter().map(Into::into).collect())
}

// ==================== 行内差分 ====================

#[uniffi::export]
pub fn diffloupe_inline_highlight(
    left: Option<String>,
    right: Option<String>,
    case_insensitive: bool,
) -> Option<FfiInlineHighlight> {
    inline_diff::highlight(left.as_deref(), right.as_deref(), case_insensitive).map(|h| {
        FfiInlineHighlight {
            left_ranges: h
                .left_ranges
                .iter()
                .map(|r| FfiIndexRange {
                    start: r.start as u64,
                    end: r.end as u64,
                })
                .collect(),
            right_ranges: h
                .right_ranges
                .iter()
                .map(|r| FfiIndexRange {
                    start: r.start as u64,
                    end: r.end as u64,
                })
                .collect(),
        }
    })
}

/// `InlineDiff.highlightsForRows` 相当(modified 行のみ)
#[uniffi::export]
pub fn diffloupe_inline_highlights_for_rows(
    rows: Vec<FfiDiffRow>,
    case_insensitive: bool,
) -> Vec<FfiRowHighlight> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    inline_diff::highlights_for_rows(&rows, case_insensitive)
        .into_iter()
        .map(|(id, h)| FfiRowHighlight {
            row_id: id as u64,
            highlight: FfiInlineHighlight {
                left_ranges: h
                    .left_ranges
                    .iter()
                    .map(|r| FfiIndexRange {
                        start: r.start as u64,
                        end: r.end as u64,
                    })
                    .collect(),
                right_ranges: h
                    .right_ranges
                    .iter()
                    .map(|r| FfiIndexRange {
                        start: r.start as u64,
                        end: r.end as u64,
                    })
                    .collect(),
            },
        })
        .collect()
}

// ==================== 表示ロジック(DiffDisplay) ====================

#[uniffi::export]
pub fn diffloupe_search_hit_row_ids(
    rows: Vec<FfiDiffRow>,
    query: String,
    case_sensitive: bool,
) -> Vec<u64> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    diff_display::search_hit_row_ids(&rows, &query, case_sensitive)
        .into_iter()
        .map(|v| v as u64)
        .collect()
}

#[uniffi::export]
pub fn diffloupe_fold_groups(rows: Vec<FfiDiffRow>, threshold: u64) -> Vec<FfiFoldGroup> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    diff_display::fold_groups(&rows, threshold as usize)
        .into_iter()
        .map(Into::into)
        .collect()
}

#[uniffi::export]
pub fn diffloupe_expanded_group_ids(
    row_ids: Vec<u64>,
    rows: Vec<FfiDiffRow>,
    threshold: u64,
) -> Vec<u64> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    let ids: Vec<usize> = row_ids.iter().map(|&v| v as usize).collect();
    diff_display::expanded_group_ids(&ids, &rows, threshold as usize)
        .into_iter()
        .map(|v| v as u64)
        .collect()
}

#[uniffi::export]
pub fn diffloupe_display_items(
    rows: Vec<FfiDiffRow>,
    expanded_group_ids: Vec<u64>,
    show_all_context_rows: bool,
    threshold: u64,
) -> Vec<FfiDisplayItem> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    let expanded: std::collections::HashSet<usize> =
        expanded_group_ids.iter().map(|&v| v as usize).collect();
    diff_display::display_items(&rows, &expanded, show_all_context_rows, threshold as usize)
        .into_iter()
        .map(Into::into)
        .collect()
}

#[uniffi::export]
pub fn diffloupe_difference_row_ids(rows: Vec<FfiDiffRow>) -> Vec<u64> {
    let rows: Vec<line_diff::DiffRow> = rows.into_iter().map(Into::into).collect();
    diff_display::difference_row_ids(&rows)
        .into_iter()
        .map(|v| v as u64)
        .collect()
}

// ==================== 正規化(ContentComparison) ====================

#[uniffi::export]
pub fn diffloupe_normalized_text(content_comparison: FfiContentComparison, text: String) -> String {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    cc.normalized_text(&text)
}

#[uniffi::export]
pub fn diffloupe_normalized_line(content_comparison: FfiContentComparison, line: String) -> String {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    cc.normalized_line(&line)
}

#[uniffi::export]
pub fn diffloupe_normalized_data(
    content_comparison: FfiContentComparison,
    data: Vec<u8>,
) -> Option<Vec<u8>> {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    cc.normalized_data(&data)
}

#[uniffi::export]
pub fn diffloupe_structured_canonical_pair(
    content_comparison: FfiContentComparison,
    left: Vec<u8>,
    left_url: Option<String>,
    right: Vec<u8>,
    right_url: Option<String>,
) -> Option<FfiTextPair> {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    cc.structured_canonical_pair(&left, left_url.as_deref(), &right, right_url.as_deref())
        .map(|(l, r)| FfiTextPair {
            left_text: l,
            right_text: r,
        })
}

// ==================== 構造化正規化 ====================

/// `StructuredNormalizer.normalize` 相当。`url` は拡張子判定用(ファイルパス)。
#[uniffi::export]
pub fn diffloupe_structured_normalize(
    data: Vec<u8>,
    url: Option<String>,
    mode: FfiStructuredComparisonMode,
) -> FfiStructuredNormalizationResult {
    let r = structured::normalize(&data, url.as_deref(), mode.into());
    FfiStructuredNormalizationResult {
        did_normalize: r.did_normalize,
        format: r.format.map(Into::into),
        normalized_text: r.normalized_text,
        notice: r.notice,
    }
}

/// 構造化パースの失敗(Swift `StructuredNormalizeError` 相当)
#[derive(Debug, uniffi::Error)]
pub enum FfiStructuredError {
    Json { message: String },
    Xml { message: String },
    Csv { message: String },
}
impl std::fmt::Display for FfiStructuredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json { message } | Self::Xml { message } | Self::Csv { message } => {
                write!(f, "{}", message)
            }
        }
    }
}
impl std::error::Error for FfiStructuredError {}

/// `StructuredNormalizer.detectCSVDelimiter` 相当。返値は1文字の文字列。
#[uniffi::export]
pub fn diffloupe_detect_csv_delimiter(text: String) -> String {
    structured::detect_csv_delimiter(&text).to_string()
}

/// `StructuredNormalizer.parseCSV` 相当。delimiter は1文字の文字列。
#[uniffi::export]
pub fn diffloupe_parse_csv(
    text: String,
    delimiter: String,
) -> Result<Vec<Vec<String>>, FfiStructuredError> {
    let delim = delimiter.chars().next().unwrap_or(',');
    structured::parse_csv(&text, delim)
        .map_err(|e| FfiStructuredError::Csv {
            message: e.to_string(),
        })
}

// ==================== Hex ====================

/// `HexDiff.compare(leftURL:rightURL:maxBytes:)` 相当
#[uniffi::export]
pub fn diffloupe_hex_compare_files(
    left_path: String,
    right_path: String,
    max_bytes: u64,
) -> Result<FfiHexComparison, FfiIoError> {
    hex_diff::compare_urls(
        Path::new(&left_path),
        Path::new(&right_path),
        max_bytes as usize,
    )
    .map(Into::into)
    .map_err(FfiIoError::io)
}

/// `HexDiff.compare(left:right:maxBytes:)`(メモリ上のデータ)
#[uniffi::export]
pub fn diffloupe_hex_compare_data(
    left: Vec<u8>,
    right: Vec<u8>,
    max_bytes: u64,
) -> FfiHexComparison {
    hex_diff::compare(&left, &right, max_bytes as usize).into()
}

/// `HexDiff.ascii` 相当
#[uniffi::export]
pub fn diffloupe_hex_ascii(bytes: Vec<u8>) -> String {
    hex_diff::ascii(&bytes)
}

// ==================== コピー計画 / 3-way / レポート ====================

/// `SyncPlan.make` 相当(純粋関数)
#[uniffi::export]
pub fn diffloupe_sync_plan_make(
    result: FfiComparisonResult,
    direction: FfiSyncDirection,
) -> FfiSyncPlan {
    let result: diffloupe_core::models::ComparisonResult = result.into();
    sync_plan::SyncPlan::make(&result, direction.into()).into()
}

/// `ThreeWayDiff.make` 相当
#[uniffi::export]
pub fn diffloupe_three_way_make(
    base: String,
    left: String,
    right: String,
    content_comparison: FfiContentComparison,
) -> FfiThreeWayDiff {
    let cc: diffloupe_core::options::ContentComparison = content_comparison.into();
    three_way::ThreeWayDiff::make(&base, &left, &right, &cc).into()
}

/// `ComparisonReport.render` 相当。
/// `generated_at` は UNIX epoch 秒(f64)。表示丸めは Swift の
/// ISO8601DateFormatter と同じ規則(最近接ミリ秒丸め→秒切り捨て)を適用する。
#[uniffi::export]
pub fn diffloupe_report_render(
    result: FfiComparisonResult,
    format: FfiReportFormat,
    generated_at: f64,
    left_git_ref: Option<String>,
    right_git_ref: Option<String>,
) -> String {
    let result: diffloupe_core::models::ComparisonResult = result.into();
    let ms = (generated_at * 1000.0).round_ties_even();
    let secs = (ms / 1000.0).floor() as i64;
    let mut report = report::ComparisonReport::new(&result, secs);
    report.left_git_ref = left_git_ref;
    report.right_git_ref = right_git_ref;
    let fmt = match format {
        FfiReportFormat::Html => report::ReportFormat::Html,
        FfiReportFormat::Csv => report::ReportFormat::Csv,
        FfiReportFormat::Json => report::ReportFormat::Json,
        FfiReportFormat::Markdown => report::ReportFormat::Markdown,
    };
    report.render(fmt)
}

// ==================== フィルタ / glob ====================

/// `GlobPattern(pattern).matches` 相当
#[uniffi::export]
pub fn diffloupe_glob_matches(pattern: String, name: String, relative_path: String) -> bool {
    GlobPattern::new(&pattern).matches(&name, &relative_path)
}

/// `FilterSet.allowsFile` 相当。パターンは初期化時に正規化される
#[uniffi::export]
pub fn diffloupe_filter_allows_file(
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    name: String,
    relative_path: String,
) -> bool {
    let fs = filter::FilterSet::new(include_patterns, exclude_patterns);
    fs.allows_file(&name, &relative_path)
}

/// `FilterSet.isExcluded` 相当
#[uniffi::export]
pub fn diffloupe_filter_is_excluded(
    exclude_patterns: Vec<String>,
    name: String,
    relative_path: String,
) -> bool {
    let fs = filter::FilterSet::new(vec![], exclude_patterns);
    fs.is_excluded(&name, &relative_path)
}

/// `FilterPreset.all` 相当
#[uniffi::export]
pub fn diffloupe_filter_presets() -> Vec<FfiFilterPreset> {
    filter::FILTER_PRESETS.iter().map(Into::into).collect()
}

/// `FilterPreset.preset(id:)` 相当
#[uniffi::export]
pub fn diffloupe_filter_preset(id: String) -> Option<FfiFilterPreset> {
    filter::filter_preset(&id).map(Into::into)
}

// ==================== git ====================

/// `GitRepository.detect` 相当。検出時は toplevel パスを返す。
#[uniffi::export]
pub fn diffloupe_git_detect(path: String) -> Option<String> {
    git_snapshot::GitRepository::detect(Path::new(&path))
        .map(|r| r.toplevel.to_string_lossy().into_owned())
}

/// `GitRepository.listRefs` 相当
#[uniffi::export]
pub fn diffloupe_git_list_refs(toplevel: String) -> Vec<String> {
    let repo = git_snapshot::GitRepository {
        toplevel: PathBuf::from(toplevel),
    };
    repo.list_refs()
}

/// `GitRepository.materializeSnapshot` 相当。展開先の一時ディレクトリを返す。
#[uniffi::export]
pub fn diffloupe_git_materialize_snapshot(
    toplevel: String,
    git_ref: String,
) -> Result<String, FfiGitError> {
    let repo = git_snapshot::GitRepository {
        toplevel: PathBuf::from(toplevel),
    };
    repo.materialize_snapshot(&git_ref)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(Into::into)
}

/// `GitRepository.cleanup` 相当
#[uniffi::export]
pub fn diffloupe_git_cleanup(path: String) {
    git_snapshot::GitRepository::cleanup(Path::new(&path))
}

// ==================== ディレクトリ監視 ====================

/// `DirectorySnapshot.scan` 相当
#[uniffi::export]
pub fn diffloupe_snapshot_scan(
    path: String,
    excluded_names: Vec<String>,
) -> Result<FfiDirectorySnapshot, FfiWatchError> {
    let excluded: Vec<&str> = excluded_names.iter().map(|s| s.as_str()).collect();
    watcher::DirectorySnapshot::scan(Path::new(&path), &excluded)
        .map(Into::into)
        .map_err(Into::into)
}

/// 変更検知の通知先。Swift 側でクロージャを包むクラスが実装する。
#[uniffi::export(with_foreign)]
pub trait FfiWatchCallback: Send + Sync {
    fn on_change(&self);
}

/// `DirectoryWatcher` 相当の監視器。start/stop で制御し、破棄時は自動停止する。
#[derive(uniffi::Object)]
pub struct FfiDirectoryWatcher {
    inner: watcher::DirectoryWatcher,
}

#[uniffi::export]
impl FfiDirectoryWatcher {
    /// - interval_secs: 再スキャン間隔(秒)。下限0.01秒にクランプされる
    #[uniffi::constructor]
    pub fn new(
        roots: Vec<String>,
        interval_secs: f64,
        excluded_names: Vec<String>,
        on_change: Arc<dyn FfiWatchCallback>,
    ) -> Arc<Self> {
        let cb = on_change.clone();
        Arc::new(Self {
            inner: watcher::DirectoryWatcher::new(
                roots.into_iter().map(PathBuf::from).collect(),
                std::time::Duration::from_secs_f64(interval_secs),
                excluded_names,
                move || cb.on_change(),
            ),
        })
    }

    pub fn start(&self) {
        self.inner.start();
    }

    pub fn stop(&self) {
        self.inner.stop();
    }
}

// ==================== 内部ヘルパ ====================

/// `FolderComparator.sha256` 相当(64KiB チャンクの SHA-256)
#[uniffi::export]
pub fn diffloupe_sha256_file(path: String) -> Result<Vec<u8>, FfiIoError> {
    let digest =
        diffloupe_core::comparator::sha256_file(Path::new(&path)).map_err(FfiIoError::io)?;
    Ok(digest.to_vec())
}
