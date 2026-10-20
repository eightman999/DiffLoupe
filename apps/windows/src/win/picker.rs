//! 共通ダイアログ(IFileOpenDialog / IFileSaveDialog)

use std::path::PathBuf;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, FOS_PICKFOLDERS,
    SIGDN_FILESYSPATH,
};

use crate::win::wide::wstr;

struct ComInit;
impl ComInit {
    fn new() -> Option<Self> {
        // CoInitializeEx は HRESULT を返す(0.62 では Result ではない)
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .ok()?;
        Some(Self)
    }
}
impl Drop for ComInit {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

unsafe fn item_path(item: &IShellItem) -> Option<PathBuf> {
    unsafe {
        let p: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        if p.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *p.0.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len));
        CoTaskMemFree(Some(p.0 as *const _));
        Some(PathBuf::from(s))
    }
}

/// フォルダ選択(FOS_PICKFOLDERS)
pub fn pick_folder(parent: HWND, title: &str) -> Option<PathBuf> {
    let _com = ComInit::new()?;
    unsafe {
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
        let title_w = wstr(title);
        let _ = dlg.SetTitle(PCWSTR(title_w.as_ptr()));
        let opts = dlg.GetOptions().unwrap_or_default() | FOS_PICKFOLDERS;
        dlg.SetOptions(opts).ok()?;
        dlg.Show(Some(parent)).ok()?;
        item_path(&dlg.GetResult().ok()?)
    }
}

/// ファイル選択
pub fn pick_file(parent: HWND, title: &str) -> Option<PathBuf> {
    let _com = ComInit::new()?;
    unsafe {
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
        let title_w = wstr(title);
        let _ = dlg.SetTitle(PCWSTR(title_w.as_ptr()));
        dlg.Show(Some(parent)).ok()?;
        item_path(&dlg.GetResult().ok()?)
    }
}

/// 保存先選択(拡張子フィルタ付き)
pub fn save_file(
    parent: HWND,
    title: &str,
    default_name: &str,
    filter_label: &str,
    ext: &str,
) -> Option<PathBuf> {
    let _com = ComInit::new()?;
    unsafe {
        let dlg: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL).ok()?;
        let title_w = wstr(title);
        let _ = dlg.SetTitle(PCWSTR(title_w.as_ptr()));
        let name_w = wstr(default_name);
        let _ = dlg.SetFileName(PCWSTR(name_w.as_ptr()));
        let label_w = wstr(filter_label);
        let pat_w = wstr(&format!("*.{}", ext));
        let spec = COMDLG_FILTERSPEC {
            pszName: PCWSTR(label_w.as_ptr()),
            pszSpec: PCWSTR(pat_w.as_ptr()),
        };
        let _ = dlg.SetFileTypes(&[spec]);
        let ext_w = wstr(ext);
        let _ = dlg.SetDefaultExtension(PCWSTR(ext_w.as_ptr()));
        dlg.Show(Some(parent)).ok()?;
        item_path(&dlg.GetResult().ok()?)
    }
}
