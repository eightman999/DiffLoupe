//! DPI スケーリング(Per-Monitor v2 前提)

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::GetDpiForWindow;

/// 96dpi 基準の論理ピクセル → 物理ピクセル
pub fn scale(hwnd: HWND, logical: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) } as i64;
    (logical as i64 * dpi / 96) as i32
}
