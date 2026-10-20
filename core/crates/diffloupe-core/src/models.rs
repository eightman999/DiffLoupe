//! 比較結果のモデル群(Swift `DiffModels.swift` 相当)

use std::path::PathBuf;

/// ファイル/ディレクトリの分類ステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffStatus {
    Identical,
    Different,
    LeftOnly,
    RightOnly,
    Error,
}

impl DiffStatus {
    /// JSON レポートで使う rawValue
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Different => "different",
            Self::LeftOnly => "leftOnly",
            Self::RightOnly => "rightOnly",
            Self::Error => "error",
        }
    }
}

/// POSIX基本メタデータ(パーミッション・UID・GID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PosixBasicMetadata {
    pub permissions: Option<u32>,
    pub owner_id: Option<u32>,
    pub group_id: Option<u32>,
}

/// リネーム/移動候補
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenameCandidate {
    pub counterpart_path: String,
    pub is_unique_match: bool,
    pub total_matches: usize,
}

/// 走査で得た1エントリのメタデータ(内部用)
#[derive(Debug, Clone)]
pub(crate) struct FileEntryMeta {
    pub path: PathBuf,
    pub is_directory: bool,
    pub size: i64,
    /// シンボリックリンクだったか(将来の拡張用に保持)
    #[allow(dead_code)]
    pub is_symlink: bool,
    /// UNIX epoch 秒(Swift `Date` と同じ f64)。秒未満は表示時に
    /// ISO8601DateFormatter の規則(最近接ミリ秒へ丸め→秒へ切り捨て)で整形する
    pub mtime: Option<f64>,
    pub posix: Option<PosixBasicMetadata>,
}

/// 相対パスで突き合わせた比較結果1件
#[derive(Debug, Clone)]
pub struct ComparedEntry {
    pub relative_path: String,
    pub name: String,
    pub is_directory: bool,
    pub status: DiffStatus,
    pub left_path: Option<PathBuf>,
    pub right_path: Option<PathBuf>,
    pub left_size: i64,
    pub right_size: i64,
    pub error_message: Option<String>,
    /// mtime(UNIX epoch 秒の f64、Swift `Date` 相当)。取得不能時は None
    pub left_modification_date: Option<f64>,
    pub right_modification_date: Option<f64>,
    pub left_posix: Option<PosixBasicMetadata>,
    pub right_posix: Option<PosixBasicMetadata>,
    pub rename_candidate: Option<RenameCandidate>,
}

/// ツリー表示用ノード
#[derive(Debug, Clone, PartialEq)]
pub struct DiffNode {
    pub id: String,
    pub name: String,
    pub is_directory: bool,
    pub status: DiffStatus,
    pub children: Option<Vec<DiffNode>>,
}

/// 比較サマリ
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComparisonSummary {
    pub identical_files: usize,
    pub different_files: usize,
    pub left_only_files: usize,
    pub right_only_files: usize,
    pub error_files: usize,
}

impl ComparisonSummary {
    pub fn total_files(&self) -> usize {
        self.identical_files
            + self.different_files
            + self.left_only_files
            + self.right_only_files
            + self.error_files
    }
}

/// 比較全体の結果
#[derive(Debug)]
pub struct ComparisonResult {
    /// 正規化済み(canonicalize)ルートパス
    pub left_root: String,
    pub right_root: String,
    /// 相対パス昇順
    pub entries: Vec<ComparedEntry>,
    pub root_node: DiffNode,
    pub summary: ComparisonSummary,
}

impl ComparisonResult {
    /// 公開コンストラクタ。絞り込み済みの結果などを外部から組み立てるために使う。
    /// 値の整合(summary と entries の対応など)は呼び出し側の責任。
    pub fn new(
        left_root: String,
        right_root: String,
        entries: Vec<ComparedEntry>,
        root_node: DiffNode,
        summary: ComparisonSummary,
    ) -> Self {
        Self {
            left_root,
            right_root,
            entries,
            root_node,
            summary,
        }
    }

    pub fn entry_by_path(&self, path: &str) -> Option<&ComparedEntry> {
        self.entries.iter().find(|e| e.relative_path == path)
    }
}

/// 比較処理の進捗
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonProgress {
    Scanning { found: usize },
    Comparing { done: usize, total: usize },
    Renames { done: usize, total: usize },
}
