//! diffloupe-gtk: DiffLoupe Linux 版(GTK4)。
//!
//! 使い方:
//!   diffloupe-gtk                          GUI を起動
//!   diffloupe-gtk --smoke-test L R [DIR]   ヘッドレス検証(ウィンドウを開き、
//!                                        比較→選択→PNGスナップショット→終了0)
//!   diffloupe-gtk --compare-time L R       GUI 経路の比較時間だけ計測して終了
//!
//! 実装は lib(`diffloupe_gtk::run`)にある。Pro 版も同じライブラリを使う。

use std::path::PathBuf;
use std::process::ExitCode;

use diffloupe_gtk::{run, smoke, AppConfig};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let pos: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();

    // ヘッドレス計測モード: GTK 初期化なしで core を直接計測
    // (GUI の比較呼び出しと同じオプション経路)
    if args.iter().any(|a| a == "--compare-time") {
        if pos.len() < 2 {
            eprintln!("usage: diffloupe-gtk --compare-time <left> <right>");
            return ExitCode::from(2);
        }
        // 計測モードはエンジンの素の速度を測るため Free 上限はかけない
        // (GUI 通常経路では FreeEdition の 2000 件上限が効く)
        let options = diffloupe_core::CompareOptions::default();
        let t0 = std::time::Instant::now();
        let res = diffloupe_core::FolderComparator::compare(
            PathBuf::from(&pos[0]).as_path(),
            PathBuf::from(&pos[1]).as_path(),
            &options,
            None,
            None,
        );
        let elapsed = t0.elapsed();
        match res {
            Ok(r) => {
                println!("compare_time_ms={}", elapsed.as_millis());
                println!("files={}", r.summary.total_files());
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("compare failed: {e:?}");
                return ExitCode::from(3);
            }
        }
    }

    let smoke_mode = args.iter().any(|a| a == "--smoke-test");
    if smoke_mode && pos.len() < 2 {
        eprintln!("usage: diffloupe-gtk --smoke-test <left> <right> [screenshots_dir]");
        return ExitCode::from(2);
    }

    let mut config = AppConfig::free();
    if smoke_mode {
        let (l, r) = (PathBuf::from(&pos[0]), PathBuf::from(&pos[1]));
        let dir = pos
            .get(2)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("apps/linux/screenshots"));
        config.on_ready = Some(Box::new(move |w| {
            smoke::run(w.clone(), l.clone(), r.clone(), dir.clone());
        }));
    }
    run(config)
}
