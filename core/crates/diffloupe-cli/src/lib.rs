//! diffloupe-cli: CLI のロジックをライブラリとして公開する。
//!
//! `diffloupe` バイナリは `cli::run` を呼ぶだけの薄い殻。Pro など別バイナリが
//! 公開版 CLI を委譲先として再利用できる(`#[path]` での取り込みは不要)。

pub mod cli;
pub use cli::run;
