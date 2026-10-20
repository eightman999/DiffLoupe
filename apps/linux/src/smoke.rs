//! ヘッドレススモークテスト。`--smoke-test <left> <right> [outdir]` で
//! ウィンドウを開き、比較→テキスト差分選択→Hex→画像→設定の各画面を
//! PNG スナップショットして終了 0 で返す。

use std::path::PathBuf;
use std::rc::Rc;

use diffloupe_core::DiffStatus;
use gtk4::glib;
use gtk4::prelude::*;

use crate::model::Phase;
use crate::window::MainWindow;

/// Xvfb 画面全体を ImageMagick `import` で PNG キャプチャする。
/// GSK レンダラ経由より確実(ウィジェット描画完了を待って撮れる)。
fn snapshot(_widget: &impl IsA<gtk4::Widget>, path: &PathBuf) -> bool {
    let st = std::process::Command::new("import")
        .args(["-window", "root"])
        .arg(path)
        .status();
    match st {
        Ok(s) if s.success() => {
            eprintln!("[smoke] wrote {}", path.display());
            true
        }
        _ => {
            eprintln!("[smoke] capture failed: {}", path.display());
            false
        }
    }
}

/// テスト用の小さい PNG を2枚生成する(右だけ中央に赤い矩形)。
fn make_test_pngs() -> Option<(PathBuf, PathBuf)> {
    let dir = std::env::temp_dir().join("diffloupe-smoke-img");
    std::fs::create_dir_all(&dir).ok()?;
    let make = |name: &str, with_red: bool| -> Option<PathBuf> {
        let p = dir.join(name);
        let surf = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, 160, 120).ok()?;
        {
            let cr = gtk4::cairo::Context::new(&surf).ok()?;
            cr.set_source_rgb(0.2, 0.4, 0.8);
            cr.paint().ok()?;
            if with_red {
                cr.set_source_rgb(0.9, 0.1, 0.1);
                cr.rectangle(60.0, 40.0, 40.0, 40.0);
                cr.fill().ok()?;
            }
        }
        // write_to_png は cairo png フィーチャで有効
        let mut f = std::fs::File::create(&p).ok()?;
        surf.write_to_png(&mut f).ok()?;
        Some(p)
    };
    Some((
        make("smoke_left.png", false)?,
        make("smoke_right.png", true)?,
    ))
}

pub fn run(win: Rc<MainWindow>, left: PathBuf, right: PathBuf, outdir: PathBuf) {
    let _ = std::fs::create_dir_all(&outdir);
    win.apply_folder(true, left);
    win.apply_folder(false, right);
    win.start_compare(true);

    type Step = Box<dyn Fn(&Rc<MainWindow>, &PathBuf) -> bool + 'static>;
    let steps: Rc<Vec<Step>> = {
        let mut v: Vec<Step> = Vec::new();
        // step0: 比較完了待ち → メイン画面スナップショット
        v.push(Box::new(|w, out| {
            if w.model.phase.get() != Phase::Done {
                return false; // 待機継続
            }
            snapshot(&w.window, &out.join("01_main.png"));
            true
        }));
        // step1: 差分のあるテキストを選択 → テキスト差分
        v.push(Box::new(|w, _out| {
            let target = w
                .model
                .result
                .borrow()
                .as_ref()
                .and_then(|r| {
                    r.entries.iter().find(|e| {
                        !e.is_directory
                            && e.status == DiffStatus::Different
                            && e.relative_path.ends_with(".txt")
                    })
                })
                .map(|e| e.relative_path.clone());
            if let Some(path) = target {
                w.on_tree_select(&path);
            }
            true
        }));
        v.push(Box::new(|w, out| {
            snapshot(&w.window, &out.join("02_text_diff.png"));
            true
        }));
        // step2: バイナリ差分を選択 → Hex
        v.push(Box::new(|w, _out| {
            let target = w
                .model
                .result
                .borrow()
                .as_ref()
                .and_then(|r| {
                    r.entries.iter().find(|e| {
                        !e.is_directory
                            && e.status == DiffStatus::Different
                            && e.relative_path.ends_with(".bin")
                    })
                })
                .map(|e| e.relative_path.clone());
            if let Some(path) = target {
                w.on_tree_select(&path);
            }
            true
        }));
        v.push(Box::new(|w, out| {
            snapshot(&w.window, &out.join("03_hex.png"));
            true
        }));
        // step3: 画像比較(フィクスチャに画像が無いため生成した PNG で描画)
        v.push(Box::new(|w, _out| {
            if let Some((l, r)) = make_test_pngs() {
                w.detail.show_image_paths(&l, &r);
            }
            true
        }));
        v.push(Box::new(|w, out| {
            snapshot(&w.window, &out.join("04_image.png"));
            true
        }));
        // step4: 設定ウィンドウ
        v.push(Box::new(|w, out| {
            crate::settingsui::open(&w.window, &w.model, &w.ext);
            // 表示中のトップレベル(設定ウィンドウ)を撮る
            let _ = out;
            true
        }));
        v.push(Box::new(|w, out| {
            // 設定ウィンドウを探して撮る
            let mut shot = false;
            for w2 in w
                .window
                .application()
                .map(|a| a.windows())
                .unwrap_or_default()
            {
                if w2.title().map(|t| t == crate::strings::SETTINGS_TITLE) == Some(true) {
                    shot |= snapshot(&w2, &out.join("05_settings.png"));
                }
            }
            if !shot {
                snapshot(&w.window, &out.join("05_settings.png"));
            }
            true
        }));
        Rc::new(v)
    };

    let idx = std::rc::Rc::new(std::cell::Cell::new(0usize));
    glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        let i = idx.get();
        if i >= steps.len() {
            eprintln!("[smoke] done");
            std::process::exit(0);
        }
        if steps[i](&win, &outdir) {
            idx.set(i + 1);
        }
        glib::ControlFlow::Continue
    });
}
