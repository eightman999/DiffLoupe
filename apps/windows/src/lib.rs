//! diffloupe-win: DiffLoupe Windows ネイティブ GUI(Win32, Free 版)
//!
//! UI 非依存のロジック(設定・履歴・フィルタ・差分ビューモデル・画像差分)は
//! ライブラリ側に置き、macOS/Linux ホストでも `cargo test` で検証できるようにする。
//! Win32 依存コードはすべて `win` モジュール(`cfg(windows)`のみ)に隔離する。

pub mod appmodel;
pub mod detail;
pub mod extmodel;
pub mod grid_model;
pub mod imagemodel;
pub mod layout;
pub mod recent;
pub mod settings;
pub mod strings;
pub mod viewmodel;

#[cfg(windows)]
pub mod win;
