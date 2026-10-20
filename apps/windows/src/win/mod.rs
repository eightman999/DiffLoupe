//! Win32 依存の UI 層。`cfg(windows)` でのみコンパイルされる。

pub mod app;
pub mod capture;
pub mod dialogs;
pub mod dpi;
pub mod grid;
pub mod guard;
pub mod imagepane;
pub mod ops;
pub mod picker;
pub mod smoke;
pub mod theme;
pub mod wic;
pub mod wide;

/// エントリポイント(引数解析 → GUI 起動 or スモークテスト)
pub fn run(args: Vec<String>) -> i32 {
    // --smoke-test <left> <right> <out-dir>
    if args.len() >= 2 && args[1] == "--smoke-test" {
        if args.len() != 5 {
            eprintln!("使い方: diffloupe-win --smoke-test <左フォルダ> <右フォルダ> <出力先>");
            return 2;
        }
        return smoke::run(
            std::path::PathBuf::from(&args[2]),
            std::path::PathBuf::from(&args[3]),
            std::path::PathBuf::from(&args[4]),
        );
    }
    app::run()
}
