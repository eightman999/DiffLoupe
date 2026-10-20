//! ヘッドレススモークテスト: `--smoke-test <left> <right> <out-dir>`。
//! メインウィンドウ生成 → 同期比較 → 差分テキスト/Hex/画像/設定の
//! 各画面を PrintWindow で BMP キャプチャし out-dir へ出力して終了 0。
//!
//! GUI サブシステムの exe は CI のログに何も出せないため、各段階の開始・成功・失敗を
//! `<out-dir>/smoke.log` に逐次追記して即座に書き出す(panic も含む)。
//! 終了コード: 0=成功 / 1=想定外の失敗(panic 等) / 2=引数・出力先エラーまたはタイムアウト /
//! 3=比較失敗 / 4=ウィンドウ生成失敗 / 5=キャプチャ失敗あり。

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::GetLastError;
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::app;

/// スモーク全体のタイムアウト(秒)
const TIMEOUT_SECS: u64 = 60;

struct Logger {
    file: Option<File>,
    console: Option<File>,
    start: Instant,
    stage: String,
}

fn logger() -> &'static Mutex<Logger> {
    static L: OnceLock<Mutex<Logger>> = OnceLock::new();
    L.get_or_init(|| {
        Mutex::new(Logger {
            file: None,
            console: None,
            start: Instant::now(),
            stage: "init".into(),
        })
    })
}

/// 1 行ログ(ファイルと、あれば親コンソールへ)。書き込みごとに flush する。
pub fn log(msg: &str) {
    let mut l = match logger().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let line = format!("[{:>8.3}s] {}\n", l.start.elapsed().as_secs_f64(), msg);
    if let Some(f) = l.file.as_mut() {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
        let _ = f.sync_data();
    }
    if let Some(c) = l.console.as_mut() {
        let _ = c.write_all(line.as_bytes());
        let _ = c.flush();
    }
}

fn set_stage(name: &str) {
    if let Ok(mut l) = logger().lock() {
        l.stage = name.to_string();
    }
    log(&format!("START {}", name));
}

fn stage_ok(name: &str) {
    log(&format!("OK    {}", name));
}

fn stage_fail(name: &str, reason: &str) {
    log(&format!("FAIL  {}: {}", name, reason));
}

fn current_stage() -> String {
    logger().lock().map(|l| l.stage.clone()).unwrap_or_default()
}

/// GetLastError 付きのエラー文字列
pub fn last_error(what: &str) -> String {
    let code = unsafe { GetLastError() }.0;
    format!("{} (GetLastError={} / 0x{:08X})", what, code, code)
}

fn init_logging(out_dir: &Path) {
    {
        let mut l = logger().lock().unwrap_or_else(|p| p.into_inner());
        l.start = Instant::now();
        l.file = File::create(out_dir.join("smoke.log")).ok();
        // 親コンソールがあれば CONOUT$ へも出す(Start-Process 経由では無い場合が多い)
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_ok() {
            l.console = std::fs::OpenOptions::new().write(true).open("CONOUT$").ok();
        }
    }
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        log(&format!(
            "PANIC stage={} : {}\nbacktrace:\n{}",
            current_stage(),
            info,
            bt
        ));
    }));
}

/// タイムアウト監視。完了フラグが立たないまま期限を超えたら理由を書いて終了コード 2 で終える。
fn spawn_watchdog(done: &'static AtomicBool) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(TIMEOUT_SECS);
        while Instant::now() < deadline {
            if done.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if !done.load(Ordering::SeqCst) {
            log(&format!(
                "TIMEOUT {}秒以内に完了せず。停止した段階: {}",
                TIMEOUT_SECS,
                current_stage()
            ));
            std::process::exit(2);
        }
    });
}

fn msg_pump_once() {
    unsafe {
        let mut m = MSG::default();
        while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
}

fn pump_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed().as_millis() < ms as u128 {
        msg_pump_once();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn write_tiny_bmps(dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    // ごく小さい 32x32 BMP(無圧縮)を2枚生成(WIC がデコード可能)
    fn bmp(path: &Path, c: [u8; 3]) -> std::io::Result<()> {
        let mut data = vec![0u8; 32 * 32 * 3 + 54];
        data[0] = b'B';
        data[1] = b'M';
        let sz = (data.len() as u32).to_le_bytes();
        data[2..6].copy_from_slice(&sz);
        data[10..14].copy_from_slice(&54u32.to_le_bytes());
        data[14..18].copy_from_slice(&40u32.to_le_bytes());
        data[18..22].copy_from_slice(&32i32.to_le_bytes());
        data[22..26].copy_from_slice(&32i32.to_le_bytes());
        data[26..28].copy_from_slice(&1u16.to_le_bytes());
        data[28..30].copy_from_slice(&24u16.to_le_bytes());
        for px in data[54..].as_chunks_mut::<3>().0 {
            px.copy_from_slice(&c);
        }
        std::fs::write(path, data)
    }
    let a = dir.join("smoke_left.bmp");
    let b = dir.join("smoke_right.bmp");
    bmp(&a, [0, 0, 200])?;
    bmp(&b, [0, 200, 0])?;
    Ok((a, b))
}

/// 相対パスは cwd 基準で絶対化する(CI はリポジトリルートから相対指定で呼ぶ)。
fn absolutize(p: PathBuf) -> PathBuf {
    std::path::absolute(&p).unwrap_or(p)
}

pub fn run(left: PathBuf, right: PathBuf, out_dir: PathBuf) -> i32 {
    let out_dir = absolutize(out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("出力先を作成できません: {}", e);
        return 2;
    }
    init_logging(&out_dir);
    static DONE: AtomicBool = AtomicBool::new(false);
    spawn_watchdog(&DONE);
    let left = absolutize(left);
    let right = absolutize(right);
    log(&format!(
        "smoke-test 開始 left={} right={} out={} cwd={:?}",
        left.display(),
        right.display(),
        out_dir.display(),
        std::env::current_dir().ok()
    ));
    let code = run_inner(&left, &right, &out_dir);
    DONE.store(true, Ordering::SeqCst);
    log(&format!("終了 exit code={}", code));
    code
}

fn run_inner(left: &Path, right: &Path, out_dir: &Path) -> i32 {
    for (label, p) in [("left", left), ("right", right)] {
        if !p.is_dir() {
            stage_fail(
                "入力確認",
                &format!("{} がディレクトリではありません: {}", label, p.display()),
            );
            return 2;
        }
    }

    set_stage("ウィンドウ生成");
    // 実ユーザーの設定を汚さないよう、設定は出力先に置く
    let hwnd = app::create_window(out_dir.join("settings.json"), true);
    if hwnd.is_invalid() {
        stage_fail(
            "ウィンドウ生成",
            &last_error("CreateWindowExW が null を返した"),
        );
        return 4;
    }
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    pump_ms(200);
    stage_ok("ウィンドウ生成");

    let app = app::app_ref(hwnd);
    let capture_failures = std::cell::Cell::new(0u32);
    let snap = |name: &str, target: windows::Win32::Foundation::HWND| {
        let stage = format!("キャプチャ {}", name);
        set_stage(&stage);
        pump_ms(150);
        match super::capture::capture(target, &out_dir.join(name)) {
            Ok(()) => stage_ok(&stage),
            Err(e) => {
                stage_fail(&stage, &e);
                capture_failures.set(capture_failures.get() + 1);
            }
        }
    };

    snap("01_main.bmp", hwnd);

    set_stage("比較");
    log("比較開始");
    if !app.run_compare_sync(left, right) {
        stage_fail(
            "比較",
            "run_compare_sync が false(フォルダ不正または結果なし)",
        );
        return 3;
    }
    log("比較完了");
    stage_ok("比較");
    snap("02_result.bmp", hwnd);

    set_stage("エントリ選択(テキスト差分)");
    match app.select_first_different() {
        Some(p) => {
            log(&format!("選択: {}", p));
            stage_ok("エントリ選択(テキスト差分)");
            pump_ms(300);
            snap("03_textdiff.bmp", hwnd);
        }
        None => log("SKIP  テキスト差分の対象なし"),
    }

    set_stage("エントリ選択(Hex)");
    match app.select_first_binary() {
        Some(p) => {
            log(&format!("選択: {}", p));
            stage_ok("エントリ選択(Hex)");
            pump_ms(200);
            snap("04_hex.bmp", hwnd);
        }
        None => log("SKIP  バイナリ差分の対象なし"),
    }

    set_stage("画像比較の準備");
    match write_tiny_bmps(out_dir) {
        Ok((li, ri)) => {
            stage_ok("画像比較の準備");
            app.show_image_compare(li, ri);
            pump_ms(600);
            snap("05_image.bmp", hwnd);
        }
        Err(e) => {
            stage_fail("画像比較の準備", &e.to_string());
            capture_failures.set(capture_failures.get() + 1);
        }
    }

    set_stage("設定ウィンドウ");
    let sh = super::dialogs::open_settings(hwnd, &mut app.model, app.font);
    if sh.is_invalid() {
        stage_fail(
            "設定ウィンドウ",
            &last_error("open_settings が null を返した"),
        );
        capture_failures.set(capture_failures.get() + 1);
    } else {
        pump_ms(300);
        snap("06_settings.bmp", sh);
        unsafe {
            let _ = DestroyWindow(sh);
        }
    }

    set_stage("終了処理");
    unsafe {
        if let Err(e) = DestroyWindow(hwnd) {
            stage_fail(
                "終了処理",
                &format!("DestroyWindow: {} / {}", e, last_error("")),
            );
        }
    }
    pump_ms(50);
    if capture_failures.get() > 0 {
        log(&format!(
            "キャプチャ/準備の失敗: {} 件",
            capture_failures.get()
        ));
        return 5;
    }
    stage_ok("終了処理");
    println!("smoke-test 完了: {}", out_dir.display());
    0
}
