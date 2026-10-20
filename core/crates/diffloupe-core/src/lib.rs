//! diffloupe-core: DiffLoupe の Rust 比較エンジン(Swift DiffLoupeCore の移植)
//!
//! フォルダ比較・フィルタ・テキスト/構造化/Hex/3-way 差分・コピー計画・
//! ディレクトリ監視・レポート出力を含む。UI・OS 依存を持たない。

pub mod cancel;
pub mod comparator;
pub mod diff_display;
pub mod filter;
pub mod git_snapshot;
pub mod glob;
pub mod hex_diff;
pub mod inline_diff;
pub mod line_diff;
pub mod models;
mod myers;
pub mod options;
pub mod paths;
pub mod report;
pub mod structured;
pub mod sync_plan;
pub mod text_loader;
pub mod three_way;
mod unicode;
mod unicode_tables;
pub mod watcher;

pub use cancel::{CancellationToken, Cancelled, EntryLimitExceeded};
pub use comparator::{CompareError, CompareOptions, FolderComparator, ProgressHandler};
pub use filter::{filter_preset, FilterKind, FilterPreset, FilterRule, FilterSet, FILTER_PRESETS};
pub use models::*;
pub use options::ContentComparison;
pub use structured::{StructuredComparisonMode, StructuredFormat};
