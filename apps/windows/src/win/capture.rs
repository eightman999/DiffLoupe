//! PrintWindow によるウィンドウの BMP スナップショット(スモークテスト用)

use std::path::Path;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SendMessageW, PW_RENDERFULLCONTENT, WM_PRINT,
};

/// ウィンドウ全体を 32bpp BMP として保存する。
///
/// 画面 DC(GetDC(NULL))や互換ビットマップに依存しない。サービスセッション等で
/// ディスプレイが無くても動くよう、メモリ DC + DIB セクションに描画する。
/// PrintWindow(FULLCONTENT) が失敗したら PrintWindow(0)、WM_PRINT の順にフォールバックする。
pub fn capture(hwnd: HWND, path: &Path) -> Result<(), String> {
    unsafe {
        let mut rc = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rc)
            .map_err(|e| super::smoke::last_error(&format!("GetWindowRect: {e}")))?;
        let w = (rc.right - rc.left).max(1);
        let h = (rc.bottom - rc.top).max(1);

        let hdc_mem = CreateCompatibleDC(None);
        if hdc_mem.is_invalid() {
            return Err(super::smoke::last_error("CreateCompatibleDC"));
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(hdc_mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) if !bits.is_null() => b,
            Ok(_) => {
                let _ = DeleteDC(hdc_mem);
                return Err(super::smoke::last_error("CreateDIBSection: bits が null"));
            }
            Err(e) => {
                let _ = DeleteDC(hdc_mem);
                return Err(super::smoke::last_error(&format!("CreateDIBSection: {e}")));
            }
        };
        let old = SelectObject(hdc_mem, HGDIOBJ(bmp.0));

        let mut how = "PrintWindow(FULLCONTENT)";
        let mut ok = PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        if !ok {
            super::smoke::log(&super::smoke::last_error(
                "PrintWindow(FULLCONTENT) 失敗。PrintWindow(0) へ",
            ));
            how = "PrintWindow(0)";
            ok = PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(0)).as_bool();
        }
        let mut result = Ok(());
        if !ok {
            super::smoke::log(&super::smoke::last_error(
                "PrintWindow(0) 失敗。WM_PRINT へ",
            ));
            how = "WM_PRINT";
            // PRF_CHECKVISIBLE を付けない(非表示でも描かせる)。NONCLIENT|CLIENT|CHILDREN|ERASEBKGND
            let flags = 0x2 | 0x4 | 0x10 | 0x8;
            SendMessageW(
                hwnd,
                WM_PRINT,
                Some(WPARAM(hdc_mem.0 as usize)),
                Some(LPARAM(flags)),
            );
        }
        if result.is_ok() {
            let pixels =
                std::slice::from_raw_parts(bits as *const u8, (w as usize) * (h as usize) * 4);
            super::smoke::log(&format!("キャプチャ方式: {} ({}x{})", how, w, h));
            result = write_bmp(path, pixels, w, h)
                .map_err(|e| format!("BMP 書き込み失敗 {}: {}", path.display(), e));
        }
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(hdc_mem);
        result
    }
}

/// 32bpp BGRA(top-down) → BMP ファイル
fn write_bmp(path: &Path, pixels: &[u8], w: i32, h: i32) -> std::io::Result<()> {
    use std::io::Write;
    let img_size = pixels.len() as u32;
    let mut f = std::fs::File::create(path)?;
    // BITMAPFILEHEADER
    f.write_all(b"BM")?;
    f.write_all(&(14 + 40 + img_size).to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?;
    f.write_all(&54u32.to_le_bytes())?;
    // BITMAPINFOHEADER(top-down でそのまま書く)
    f.write_all(&40u32.to_le_bytes())?;
    f.write_all(&w.to_le_bytes())?;
    f.write_all(&(-h).to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&32u16.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?; // BI_RGB
    f.write_all(&img_size.to_le_bytes())?;
    f.write_all(&2835i32.to_le_bytes())?;
    f.write_all(&2835i32.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?;
    f.write_all(pixels)?;
    Ok(())
}
