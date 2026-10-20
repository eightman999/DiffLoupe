//! PrintWindow によるウィンドウの BMP スナップショット(スモークテスト用)

use std::path::Path;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, PW_RENDERFULLCONTENT};

/// ウィンドウ全体を 32bpp BMP として保存する。
pub fn capture(hwnd: HWND, path: &Path) -> Result<(), String> {
    unsafe {
        let mut rc = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rc).map_err(|e| e.to_string())?;
        let w = (rc.right - rc.left).max(1);
        let h = (rc.bottom - rc.top).max(1);

        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return Err("GetDC failed".into());
        }
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let bmp = CreateCompatibleBitmap(hdc_screen, w, h);
        let old = SelectObject(hdc_mem, HGDIOBJ(bmp.0));
        if !PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool() {
            return Err("PrintWindow failed".to_string());
        }

        let mut info = BITMAPINFO {
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
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            hdc_mem,
            bmp,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);
        if got == 0 {
            return Err("GetDIBits failed".into());
        }
        write_bmp(path, &pixels, w, h).map_err(|e| e.to_string())
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
