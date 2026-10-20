// diffloupe-win: DiffLoupe Windows GUI(Free 版)のエントリポイント。
// 引数:
//   (なし)                        GUI を起動
//   --smoke-test <左> <右> <出力先>  ヘッドレス検証: ウィンドウ生成・比較実行・
//                                  各画面のスクリーンショット(BMP)を出力して終了 0

#[cfg(windows)]
fn main() {
    // Free 版: 既定構成(FreeEdition・拡張ビューなし・%APPDATA%\DiffLoupe)
    let config = diffloupe_win::win::ext::AppConfig::free();
    std::process::exit(diffloupe_win::win::run(std::env::args().collect(), config));
}

#[cfg(not(windows))]
fn main() {
    eprintln!("diffloupe-win は Windows 専用です。ホスト上ではライブラリのテストのみが対象です。");
    std::process::exit(2);
}
