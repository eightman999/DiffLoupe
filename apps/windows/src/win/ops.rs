//! シェル操作(開く/エクスプローラーに表示/パスをコピー) — 読み取り専用導線

use std::path::Path;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::win::wide::wstr;

/// 規定アプリで開く
pub fn open_path(path: &Path) -> bool {
    let file = wstr(&path.to_string_lossy());
    let verb = wstr("open");
    let mut ei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: windows::core::PCWSTR(verb.as_ptr()),
        lpFile: windows::core::PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut ei) }.is_ok()
}

/// エクスプローラーで対象を選択表示
pub fn reveal_in_explorer(path: &Path) -> bool {
    let param = wstr(&format!("/select,{}", path.to_string_lossy()));
    let file = wstr("explorer.exe");
    let verb = wstr("open");
    let mut ei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: windows::core::PCWSTR(verb.as_ptr()),
        lpFile: windows::core::PCWSTR(file.as_ptr()),
        lpParameters: windows::core::PCWSTR(param.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut ei) }.is_ok()
}

/// プレーンテキストをクリップボードへ書き込む
pub fn copy_to_clipboard(hwnd: HWND, text: &str) -> bool {
    let wide = wstr(text);
    let bytes = wide.len() * 2;
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
            let _ = CloseClipboard();
            return false;
        };
        let dst = GlobalLock(hmem);
        if dst.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, dst as *mut u8, bytes);
        let _ = GlobalUnlock(hmem);
        // CF_UNICODETEXT = 13
        let ok = SetClipboardData(13, Some(windows::Win32::Foundation::HANDLE(hmem.0))).is_ok();
        let _ = CloseClipboard();
        ok
    }
}
