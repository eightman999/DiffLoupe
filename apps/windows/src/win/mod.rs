//! Win32 依存の UI 層。`cfg(windows)` でのみコンパイルされる。

pub mod app;
pub mod capture;
pub mod dialogs;
pub mod dpi;
pub mod ext;
pub mod font;
pub mod grid;
pub mod guard;
pub mod imagepane;
pub mod ops;
pub mod picker;
pub mod smoke;
pub mod theme;
pub mod wic;
pub mod wide;

/// エントリポイント(引数解析 → GUI 起動 or スモークテスト)。
/// `config` はエディション・拡張ビュー・表示名・設定保存先を決める(Free は `AppConfig::free`)。
pub fn run(args: Vec<String>, config: ext::AppConfig) -> i32 {
    // --smoke-test <left> <right> <out-dir>
    if args.len() >= 2 && args[1] == "--smoke-test" {
        if args.len() != 5 {
            eprintln!("使い方: diffloupe-win --smoke-test <左フォルダ> <右フォルダ> <出力先>");
            return 2;
        }
        let left = std::path::PathBuf::from(&args[2]);
        let right = std::path::PathBuf::from(&args[3]);
        let out = std::path::PathBuf::from(&args[4]);
        if let Some(f) = config.smoke {
            return f(&config, left, right, out);
        }
        return smoke::run(left, right, out, &config);
    }
    app::run(&config)
}
