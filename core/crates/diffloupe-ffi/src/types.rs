//! Swift `DiffLoupeCore` の公開型に対応する FFI レコード/enum と変換。
//! フィールド名は Swift API に揃える(UniFFI が Swift 側へ lowerCamel で出す)。

use diffloupe_core::{
    comparator::CompareOptions,
    diff_display,
    filter::{self, FilterKind, FilterRule},
    hex_diff, line_diff, models,
    options::ContentComparison,
    structured::{StructuredComparisonMode, StructuredFormat},
    sync_plan, three_way, watcher,
};
use std::path::PathBuf;

// ---------- 基本モデル ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiDiffStatus {
    Identical,
    Different,
    LeftOnly,
    RightOnly,
    Error,
}

impl From<models::DiffStatus> for FfiDiffStatus {
    fn from(s: models::DiffStatus) -> Self {
        match s {
            models::DiffStatus::Identical => Self::Identical,
            models::DiffStatus::Different => Self::Different,
            models::DiffStatus::LeftOnly => Self::LeftOnly,
            models::DiffStatus::RightOnly => Self::RightOnly,
            models::DiffStatus::Error => Self::Error,
        }
    }
}
impl From<FfiDiffStatus> for models::DiffStatus {
    fn from(s: FfiDiffStatus) -> Self {
        match s {
            FfiDiffStatus::Identical => Self::Identical,
            FfiDiffStatus::Different => Self::Different,
            FfiDiffStatus::LeftOnly => Self::LeftOnly,
            FfiDiffStatus::RightOnly => Self::RightOnly,
            FfiDiffStatus::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiPosixBasicMetadata {
    pub permissions: Option<u32>,
    pub owner_id: Option<u32>,
    pub group_id: Option<u32>,
}

impl From<models::PosixBasicMetadata> for FfiPosixBasicMetadata {
    fn from(m: models::PosixBasicMetadata) -> Self {
        Self {
            permissions: m.permissions,
            owner_id: m.owner_id,
            group_id: m.group_id,
        }
    }
}
impl From<FfiPosixBasicMetadata> for models::PosixBasicMetadata {
    fn from(m: FfiPosixBasicMetadata) -> Self {
        Self {
            permissions: m.permissions,
            owner_id: m.owner_id,
            group_id: m.group_id,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiRenameCandidate {
    pub counterpart_path: String,
    pub is_unique_match: bool,
    pub total_matches: u64,
}

impl From<models::RenameCandidate> for FfiRenameCandidate {
    fn from(c: models::RenameCandidate) -> Self {
        Self {
            counterpart_path: c.counterpart_path,
            is_unique_match: c.is_unique_match,
            total_matches: c.total_matches as u64,
        }
    }
}
impl From<FfiRenameCandidate> for models::RenameCandidate {
    fn from(c: FfiRenameCandidate) -> Self {
        Self {
            counterpart_path: c.counterpart_path,
            is_unique_match: c.is_unique_match,
            total_matches: c.total_matches as usize,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiComparedEntry {
    pub relative_path: String,
    pub name: String,
    pub is_directory: bool,
    pub status: FfiDiffStatus,
    pub left_path: Option<String>,
    pub right_path: Option<String>,
    pub left_size: i64,
    pub right_size: i64,
    pub error_message: Option<String>,
    /// UNIX epoch 秒(秒未満を含む)。Swift `Date` 相当
    pub left_modification_date: Option<f64>,
    pub right_modification_date: Option<f64>,
    pub left_posix: Option<FfiPosixBasicMetadata>,
    pub right_posix: Option<FfiPosixBasicMetadata>,
    pub rename_candidate: Option<FfiRenameCandidate>,
}

impl From<models::ComparedEntry> for FfiComparedEntry {
    fn from(e: models::ComparedEntry) -> Self {
        Self {
            relative_path: e.relative_path,
            name: e.name,
            is_directory: e.is_directory,
            status: e.status.into(),
            left_path: e.left_path.map(|p| p.to_string_lossy().into_owned()),
            right_path: e.right_path.map(|p| p.to_string_lossy().into_owned()),
            left_size: e.left_size,
            right_size: e.right_size,
            error_message: e.error_message,
            left_modification_date: e.left_modification_date,
            right_modification_date: e.right_modification_date,
            left_posix: e.left_posix.map(Into::into),
            right_posix: e.right_posix.map(Into::into),
            rename_candidate: e.rename_candidate.map(Into::into),
        }
    }
}
impl From<FfiComparedEntry> for models::ComparedEntry {
    fn from(e: FfiComparedEntry) -> Self {
        Self {
            relative_path: e.relative_path,
            name: e.name,
            is_directory: e.is_directory,
            status: e.status.into(),
            left_path: e.left_path.map(PathBuf::from),
            right_path: e.right_path.map(PathBuf::from),
            left_size: e.left_size,
            right_size: e.right_size,
            error_message: e.error_message,
            left_modification_date: e.left_modification_date,
            right_modification_date: e.right_modification_date,
            left_posix: e.left_posix.map(Into::into),
            right_posix: e.right_posix.map(Into::into),
            rename_candidate: e.rename_candidate.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiDiffNode {
    pub id: String,
    pub name: String,
    pub is_directory: bool,
    pub status: FfiDiffStatus,
    pub children: Option<Vec<FfiDiffNode>>,
}

impl From<models::DiffNode> for FfiDiffNode {
    fn from(n: models::DiffNode) -> Self {
        Self {
            id: n.id,
            name: n.name,
            is_directory: n.is_directory,
            status: n.status.into(),
            children: n
                .children
                .map(|cs| cs.into_iter().map(Into::into).collect()),
        }
    }
}
impl From<FfiDiffNode> for models::DiffNode {
    fn from(n: FfiDiffNode) -> Self {
        Self {
            id: n.id,
            name: n.name,
            is_directory: n.is_directory,
            status: n.status.into(),
            children: n
                .children
                .map(|cs| cs.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiComparisonSummary {
    pub identical_files: u64,
    pub different_files: u64,
    pub left_only_files: u64,
    pub right_only_files: u64,
    pub error_files: u64,
}

impl From<models::ComparisonSummary> for FfiComparisonSummary {
    fn from(s: models::ComparisonSummary) -> Self {
        Self {
            identical_files: s.identical_files as u64,
            different_files: s.different_files as u64,
            left_only_files: s.left_only_files as u64,
            right_only_files: s.right_only_files as u64,
            error_files: s.error_files as u64,
        }
    }
}
impl From<FfiComparisonSummary> for models::ComparisonSummary {
    fn from(s: FfiComparisonSummary) -> Self {
        Self {
            identical_files: s.identical_files as usize,
            different_files: s.different_files as usize,
            left_only_files: s.left_only_files as usize,
            right_only_files: s.right_only_files as usize,
            error_files: s.error_files as usize,
        }
    }
}

/// 比較結果のツリーノード(圧縮版)。
/// `entry_index` が有効なら id/name は対応する ComparedEntry から復元できるので転送しない
/// (1万ファイルで同じ相対パス文字列を2重に転送・生成するコストを避ける)。
/// 対応するエントリが無いノードのみ `entry_index = u32::MAX` で id/name を持つ。
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTreeNode {
    pub entry_index: u32,
    pub id: String,
    pub name: String,
    pub is_directory: bool,
    pub status: FfiDiffStatus,
    pub children: Option<Vec<FfiTreeNode>>,
}

fn tree_node_from(
    n: models::DiffNode,
    index: &std::collections::HashMap<&str, u32>,
    entries: &[models::ComparedEntry],
) -> FfiTreeNode {
    let (entry_index, id, name) = match index.get(n.id.as_str()) {
        Some(&i) => {
            let name = if entries[i as usize].name == n.name {
                String::new()
            } else {
                n.name
            };
            (i, String::new(), name)
        }
        None => (u32::MAX, n.id, n.name),
    };
    FfiTreeNode {
        entry_index,
        id,
        name,
        is_directory: n.is_directory,
        status: n.status.into(),
        children: n.children.map(|cs| {
            cs.into_iter()
                .map(|c| tree_node_from(c, index, entries))
                .collect()
        }),
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiComparisonResult {
    pub left_root: String,
    pub right_root: String,
    pub entries: Vec<FfiComparedEntry>,
    /// Rust → Swift 方向でのみ有効。Swift → Rust(レポート等)では無視され、entries から再構築する。
    pub root_node: FfiTreeNode,
    pub summary: FfiComparisonSummary,
}

impl From<models::ComparisonResult> for FfiComparisonResult {
    fn from(r: models::ComparisonResult) -> Self {
        let root_node = {
            let index: std::collections::HashMap<&str, u32> = r
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (e.relative_path.as_str(), i as u32))
                .collect();
            tree_node_from(r.root_node, &index, &r.entries)
        };
        Self {
            left_root: r.left_root,
            right_root: r.right_root,
            entries: r.entries.into_iter().map(Into::into).collect(),
            root_node,
            summary: r.summary.into(),
        }
    }
}
impl From<FfiComparisonResult> for models::ComparisonResult {
    fn from(r: FfiComparisonResult) -> Self {
        let entries: Vec<models::ComparedEntry> = r.entries.into_iter().map(Into::into).collect();
        let root_node = diffloupe_core::comparator::FolderComparator::build_tree(&entries);
        Self {
            left_root: r.left_root,
            right_root: r.right_root,
            entries,
            root_node,
            summary: r.summary.into(),
        }
    }
}

// ---------- 進捗 / オプション ----------

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiComparisonProgress {
    Scanning { found: u64 },
    Comparing { done: u64, total: u64 },
    Renames { done: u64, total: u64 },
}

impl From<models::ComparisonProgress> for FfiComparisonProgress {
    fn from(p: models::ComparisonProgress) -> Self {
        match p {
            models::ComparisonProgress::Scanning { found } => Self::Scanning {
                found: found as u64,
            },
            models::ComparisonProgress::Comparing { done, total } => Self::Comparing {
                done: done as u64,
                total: total as u64,
            },
            models::ComparisonProgress::Renames { done, total } => Self::Renames {
                done: done as u64,
                total: total as u64,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiStructuredComparisonMode {
    Disabled,
    Automatic,
    Json,
    Xml,
    Csv,
}

impl From<FfiStructuredComparisonMode> for StructuredComparisonMode {
    fn from(m: FfiStructuredComparisonMode) -> Self {
        match m {
            FfiStructuredComparisonMode::Disabled => Self::Disabled,
            FfiStructuredComparisonMode::Automatic => Self::Automatic,
            FfiStructuredComparisonMode::Json => Self::Json,
            FfiStructuredComparisonMode::Xml => Self::Xml,
            FfiStructuredComparisonMode::Csv => Self::Csv,
        }
    }
}
impl From<StructuredComparisonMode> for FfiStructuredComparisonMode {
    fn from(m: StructuredComparisonMode) -> Self {
        match m {
            StructuredComparisonMode::Disabled => Self::Disabled,
            StructuredComparisonMode::Automatic => Self::Automatic,
            StructuredComparisonMode::Json => Self::Json,
            StructuredComparisonMode::Xml => Self::Xml,
            StructuredComparisonMode::Csv => Self::Csv,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiStructuredFormat {
    Json,
    Xml,
    Csv,
}

impl From<StructuredFormat> for FfiStructuredFormat {
    fn from(f: StructuredFormat) -> Self {
        match f {
            StructuredFormat::Json => Self::Json,
            StructuredFormat::Xml => Self::Xml,
            StructuredFormat::Csv => Self::Csv,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiContentComparison {
    pub ignore_line_endings: bool,
    pub ignore_trailing_whitespace: bool,
    pub normalize_whitespace: bool,
    pub ignore_case: bool,
    pub structured_mode: FfiStructuredComparisonMode,
}

impl From<FfiContentComparison> for ContentComparison {
    fn from(c: FfiContentComparison) -> Self {
        Self {
            ignore_line_endings: c.ignore_line_endings,
            ignore_trailing_whitespace: c.ignore_trailing_whitespace,
            normalize_whitespace: c.normalize_whitespace,
            ignore_case: c.ignore_case,
            structured_mode: c.structured_mode.into(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiCompareOptions {
    pub recursive: bool,
    pub use_default_excludes: bool,
    /// None で無制限
    pub max_entries: Option<u64>,
    pub user_exclude_patterns: Vec<String>,
    pub user_include_patterns: Vec<String>,
    pub content_comparison: FfiContentComparison,
    pub detect_renames: bool,
    pub max_normalization_compare_bytes: i64,
}

impl From<FfiCompareOptions> for CompareOptions {
    fn from(o: FfiCompareOptions) -> Self {
        Self {
            recursive: o.recursive,
            use_default_excludes: o.use_default_excludes,
            max_entries: o.max_entries.map(|v| v as usize),
            user_exclude_patterns: o.user_exclude_patterns,
            user_include_patterns: o.user_include_patterns,
            content_comparison: o.content_comparison.into(),
            detect_renames: o.detect_renames,
            max_normalization_compare_bytes: o.max_normalization_compare_bytes,
        }
    }
}

// ---------- テキスト ----------

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiTextLoadResult {
    Text { text: String, encoding_name: String },
    Binary,
    TooLarge,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiDecodedText {
    pub text: String,
    pub encoding_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiDiffRowKind {
    Context,
    Added,
    Removed,
    Modified,
}

impl From<line_diff::DiffRowKind> for FfiDiffRowKind {
    fn from(k: line_diff::DiffRowKind) -> Self {
        match k {
            line_diff::DiffRowKind::Context => Self::Context,
            line_diff::DiffRowKind::Added => Self::Added,
            line_diff::DiffRowKind::Removed => Self::Removed,
            line_diff::DiffRowKind::Modified => Self::Modified,
        }
    }
}
impl From<FfiDiffRowKind> for line_diff::DiffRowKind {
    fn from(k: FfiDiffRowKind) -> Self {
        match k {
            FfiDiffRowKind::Context => Self::Context,
            FfiDiffRowKind::Added => Self::Added,
            FfiDiffRowKind::Removed => Self::Removed,
            FfiDiffRowKind::Modified => Self::Modified,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiDiffRow {
    pub id: u64,
    pub kind: FfiDiffRowKind,
    pub left_number: Option<u64>,
    pub left_text: Option<String>,
    pub right_number: Option<u64>,
    pub right_text: Option<String>,
}

impl From<line_diff::DiffRow> for FfiDiffRow {
    fn from(r: line_diff::DiffRow) -> Self {
        Self {
            id: r.id as u64,
            kind: r.kind.into(),
            left_number: r.left_number.map(|v| v as u64),
            left_text: r.left_text,
            right_number: r.right_number.map(|v| v as u64),
            right_text: r.right_text,
        }
    }
}
impl From<FfiDiffRow> for line_diff::DiffRow {
    fn from(r: FfiDiffRow) -> Self {
        Self {
            id: r.id as usize,
            kind: r.kind.into(),
            left_number: r.left_number.map(|v| v as usize),
            left_text: r.left_text,
            right_number: r.right_number.map(|v| v as usize),
            right_text: r.right_text,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiIndexRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiInlineHighlight {
    pub left_ranges: Vec<FfiIndexRange>,
    pub right_ranges: Vec<FfiIndexRange>,
}

/// 行IDとハイライトのペア(UniFFI は整数キーの辞書を扱わないため列で返す)
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiRowHighlight {
    pub row_id: u64,
    pub highlight: FfiInlineHighlight,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiFoldGroup {
    pub id: u64,
    pub row_ids: Vec<u64>,
    pub count: u64,
}

impl From<diff_display::FoldGroup> for FfiFoldGroup {
    fn from(g: diff_display::FoldGroup) -> Self {
        Self {
            id: g.id as u64,
            row_ids: g.row_ids.iter().map(|&v| v as u64).collect(),
            count: g.count as u64,
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiDisplayItem {
    Row(FfiDiffRow),
    Collapsed(FfiFoldGroup),
}

impl From<diff_display::DisplayItem> for FfiDisplayItem {
    fn from(i: diff_display::DisplayItem) -> Self {
        match i {
            diff_display::DisplayItem::Row(r) => Self::Row(r.into()),
            diff_display::DisplayItem::Collapsed(g) => Self::Collapsed(g.into()),
        }
    }
}

// ---------- 構造化 ----------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiStructuredNormalizationResult {
    pub did_normalize: bool,
    pub format: Option<FfiStructuredFormat>,
    pub normalized_text: Option<String>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTextPair {
    pub left_text: String,
    pub right_text: String,
}

// ---------- Hex ----------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiHexComparison {
    pub left_data: Vec<u8>,
    pub right_data: Vec<u8>,
    pub left_total_size: i64,
    pub right_total_size: i64,
    pub truncated: bool,
}

impl From<hex_diff::HexComparison> for FfiHexComparison {
    fn from(c: hex_diff::HexComparison) -> Self {
        Self {
            left_data: c.left_data,
            right_data: c.right_data,
            left_total_size: c.left_total_size,
            right_total_size: c.right_total_size,
            truncated: c.truncated,
        }
    }
}

// ---------- コピー計画 / 3-way / レポート ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncDirection {
    LeftToRight,
    RightToLeft,
}

impl From<FfiSyncDirection> for sync_plan::SyncDirection {
    fn from(d: FfiSyncDirection) -> Self {
        match d {
            FfiSyncDirection::LeftToRight => Self::LeftToRight,
            FfiSyncDirection::RightToLeft => Self::RightToLeft,
        }
    }
}
impl From<sync_plan::SyncDirection> for FfiSyncDirection {
    fn from(d: sync_plan::SyncDirection) -> Self {
        match d {
            sync_plan::SyncDirection::LeftToRight => Self::LeftToRight,
            sync_plan::SyncDirection::RightToLeft => Self::RightToLeft,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncAction {
    CopyFile,
    CreateDirectory,
    Conflict,
    PreservedDestination,
    Blocked,
}

impl From<sync_plan::SyncAction> for FfiSyncAction {
    fn from(a: sync_plan::SyncAction) -> Self {
        match a {
            sync_plan::SyncAction::CopyFile => Self::CopyFile,
            sync_plan::SyncAction::CreateDirectory => Self::CreateDirectory,
            sync_plan::SyncAction::Conflict => Self::Conflict,
            sync_plan::SyncAction::PreservedDestination => Self::PreservedDestination,
            sync_plan::SyncAction::Blocked => Self::Blocked,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSyncItem {
    pub relative_path: String,
    pub action: FfiSyncAction,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub bytes: i64,
    pub reason: String,
    /// Swift SyncItem の `direction`(id 生成用)
    pub direction_key: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSyncSummary {
    pub copy_files: u64,
    pub create_directories: u64,
    pub conflicts: u64,
    pub preserved_destinations: u64,
    pub blocked: u64,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSyncPlan {
    pub direction: FfiSyncDirection,
    pub source_root: String,
    pub destination_root: String,
    pub items: Vec<FfiSyncItem>,
    pub summary: FfiSyncSummary,
}

impl From<sync_plan::SyncPlan> for FfiSyncPlan {
    fn from(p: sync_plan::SyncPlan) -> Self {
        Self {
            direction: p.direction.into(),
            source_root: p.source_root,
            destination_root: p.destination_root,
            items: p
                .items
                .into_iter()
                .map(|i| FfiSyncItem {
                    relative_path: i.relative_path,
                    action: i.action.into(),
                    source: i.source,
                    destination: i.destination,
                    bytes: i.bytes,
                    reason: i.reason,
                    direction_key: i.direction_key.to_string(),
                })
                .collect(),
            summary: FfiSyncSummary {
                copy_files: p.summary.copy_files as u64,
                create_directories: p.summary.create_directories as u64,
                conflicts: p.summary.conflicts as u64,
                preserved_destinations: p.summary.preserved_destinations as u64,
                blocked: p.summary.blocked as u64,
                total_bytes: p.summary.total_bytes,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiThreeWayDiffStatus {
    Unchanged,
    LeftChanged,
    RightChanged,
    BothChangedSame,
    Conflict,
}

impl From<three_way::ThreeWayDiffStatus> for FfiThreeWayDiffStatus {
    fn from(s: three_way::ThreeWayDiffStatus) -> Self {
        match s {
            three_way::ThreeWayDiffStatus::Unchanged => Self::Unchanged,
            three_way::ThreeWayDiffStatus::LeftChanged => Self::LeftChanged,
            three_way::ThreeWayDiffStatus::RightChanged => Self::RightChanged,
            three_way::ThreeWayDiffStatus::BothChangedSame => Self::BothChangedSame,
            three_way::ThreeWayDiffStatus::Conflict => Self::Conflict,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiThreeWayLine {
    pub id: u64,
    pub status: FfiThreeWayDiffStatus,
    pub base: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
    pub base_number: Option<u64>,
    pub left_number: Option<u64>,
    pub right_number: Option<u64>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiThreeWayDiff {
    pub lines: Vec<FfiThreeWayLine>,
    pub truncated: bool,
    pub unchanged_count: u64,
    pub left_changed_count: u64,
    pub right_changed_count: u64,
    pub both_changed_same_count: u64,
    pub conflict_count: u64,
}

impl From<three_way::ThreeWayDiff> for FfiThreeWayDiff {
    fn from(d: three_way::ThreeWayDiff) -> Self {
        Self {
            lines: d
                .lines
                .into_iter()
                .map(|l| FfiThreeWayLine {
                    id: l.id as u64,
                    status: l.status.into(),
                    base: l.base,
                    left: l.left,
                    right: l.right,
                    base_number: l.base_number.map(|v| v as u64),
                    left_number: l.left_number.map(|v| v as u64),
                    right_number: l.right_number.map(|v| v as u64),
                })
                .collect(),
            truncated: d.truncated,
            unchanged_count: d.unchanged_count as u64,
            left_changed_count: d.left_changed_count as u64,
            right_changed_count: d.right_changed_count as u64,
            both_changed_same_count: d.both_changed_same_count as u64,
            conflict_count: d.conflict_count as u64,
        }
    }
}

// ---------- フィルタ ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiFilterKind {
    Include,
    Exclude,
}

impl From<FilterKind> for FfiFilterKind {
    fn from(k: FilterKind) -> Self {
        match k {
            FilterKind::Include => Self::Include,
            FilterKind::Exclude => Self::Exclude,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiFilterRule {
    pub kind: FfiFilterKind,
    pub pattern: String,
}

impl From<FilterRule> for FfiFilterRule {
    fn from(r: FilterRule) -> Self {
        Self {
            kind: r.kind.into(),
            pattern: r.pattern,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiFilterPreset {
    pub id: String,
    pub display_name: String,
    pub exclude_patterns: Vec<String>,
}

impl From<&'static filter::FilterPreset> for FfiFilterPreset {
    fn from(p: &'static filter::FilterPreset) -> Self {
        Self {
            id: p.id.to_string(),
            display_name: p.display_name.to_string(),
            exclude_patterns: p.exclude_patterns.iter().map(|s| s.to_string()).collect(),
        }
    }
}

// ---------- レポート ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiReportFormat {
    Html,
    Csv,
    Json,
    Markdown,
}

// ---------- ウォッチャー ----------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSnapshotEntry {
    pub relative_path: String,
    pub is_directory: bool,
    pub file_size: i64,
    /// UNIX epoch 秒(高精度)。ディレクトリは None
    pub modification_date: Option<f64>,
    pub is_symbolic_link: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiDirectorySnapshot {
    pub entries: Vec<FfiSnapshotEntry>,
}

impl From<watcher::DirectorySnapshot> for FfiDirectorySnapshot {
    fn from(s: watcher::DirectorySnapshot) -> Self {
        Self {
            entries: s
                .entries
                .into_iter()
                .map(|e| FfiSnapshotEntry {
                    relative_path: e.relative_path,
                    is_directory: e.is_directory,
                    file_size: e.file_size,
                    modification_date: e.modification_date.and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs_f64())
                    }),
                    is_symbolic_link: e.is_symbolic_link,
                })
                .collect(),
        }
    }
}
