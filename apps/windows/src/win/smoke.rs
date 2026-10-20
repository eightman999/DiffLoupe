//! ヘッドレススモークテスト: `--smoke-test <left> <right> <out-dir>`。
//! メインウィンドウ生成 → 同期比較 → 差分テキスト/Hex/画像/設定の
//! 各画面を PrintWindow で BMP キャプチャし out-dir へ出力して終了 0。

use std::path::{Path, PathBuf};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::app;

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
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < ms as u128 {
        msg_pump_once();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn write_tiny_pngs(dir: &Path) -> (PathBuf, PathBuf) {
    // ごく小さい 32x32 BMP(無圧縮)を2枚生成(WIC がデコード可能)
    fn bmp(path: &Path, c: [u8; 3]) {
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
        for px in data[54..].chunks_exact_mut(3) {
            px.copy_from_slice(&c);
        }
        std::fs::write(path, data).unwrap();
    }
    let a = dir.join("smoke_left.bmp");
    let b = dir.join("smoke_right.bmp");
    bmp(&a, [0, 0, 200]);
    bmp(&b, [0, 200, 0]);
    (a, b)
}

pub fn run(left: PathBuf, right: PathBuf, out_dir: PathBuf) -> i32 {
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("出力先を作成できません: {}", e);
        return 2;
    }
    let dir = app::appdata_dir();
    let hwnd = app::create_window(dir.join("settings.json"), true);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    pump_ms(200);
    let app = app::app_ref(hwnd);
    let snap = |name: &str| {
        pump_ms(150);
        let p = out_dir.join(name);
        if let Err(e) = super::capture::capture(hwnd, &p) {
            eprintln!("スナップショット失敗 {}: {}", name, e);
        }
    };
    // 1. メイン画面(比較前)
    snap("01_main.png.bmp");
    // 2. 比較実行 → 結果ツリー
    if !app.run_compare_sync(&left, &right) {
        eprintln!("比較に失敗しました");
        return 3;
    }
    snap("02_result.bmp");
    // 3. テキスト差分
    if app.select_first_different().is_some() {
        pump_ms(300);
        snap("03_textdiff.bmp");
    }
    // 4. Hex
    if app.select_first_binary().is_some() {
        pump_ms(200);
        snap("04_hex.bmp");
    }
    // 5. 画像比較(フィクスチャに無いため合成画像でビューを検証)
    let (li, ri) = write_tiny_pngs(&out_dir);
    app.show_image_compare(li, ri);
    pump_ms(600);
    snap("05_image.bmp");
    // 6. 設定ウィンドウ
    let sh = super::dialogs::open_settings(hwnd, &mut app.model, app.font);
    pump_ms(300);
    let p = out_dir.join("06_settings.bmp");
    if let Err(e) = super::capture::capture(sh, &p) {
        eprintln!("設定画面のスナップショット失敗: {}", e);
    }
    unsafe {
        let _ = DestroyWindow(sh);
        let _ = DestroyWindow(hwnd);
    }
    println!("smoke-test 完了: {}", out_dir.display());
    0
}
