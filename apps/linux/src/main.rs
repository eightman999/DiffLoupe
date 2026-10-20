//! diffloupe-gtk: DiffLoupe Linux 版(GTK4)。
//!
//! 使い方:
//!   diffloupe-gtk                          GUI を起動
//!   diffloupe-gtk --smoke-test L R [DIR]   ヘッドレス検証(ウィンドウを開き、
//!                                        比較→選択→PNGスナップショット→終了0)
//!   diffloupe-gtk --compare-time L R       GUI 経路の比較時間だけ計測して終了

mod css;
mod detail;
mod edition_ext;
mod hexview;
mod imageops;
mod imageview;
mod model;
mod recent;
mod settings;
mod settingsui;
mod smoke;
mod strings;
mod syncplan;
mod textdiff;
mod threeway;
mod tree;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;

use gtk4::prelude::*;

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

    let smoke = args.iter().any(|a| a == "--smoke-test");
    if smoke && pos.len() < 2 {
        eprintln!("usage: diffloupe-gtk --smoke-test <left> <right> [screenshots_dir]");
        return ExitCode::from(2);
    }

    let app = gtk4::Application::new(
        Some("com.furin-lab.diffloupe"),
        gtk4::gio::ApplicationFlags::NON_UNIQUE,
    );
    let smoke_args = if smoke {
        Some((
            PathBuf::from(&pos[0]),
            PathBuf::from(&pos[1]),
            pos.get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("apps/linux/screenshots")),
        ))
    } else {
        None
    };
    app.connect_activate(move |app| {
        install_css();
        let w = window::MainWindow::new(app);
        w.window.present();
        if let Some((l, r, dir)) = &smoke_args {
            smoke::run(w.clone(), l.clone(), r.clone(), dir.clone());
        }
    });
    let code = app.run_with_args::<&str>(&[]);
    ExitCode::from(code.value() as u8)
}

fn install_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css::APP_CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
