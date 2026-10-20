//! 配色テーマ(ライト/ダーク)。ダーク検出は個人設定レジストリ値
//! `HKCU\...\Personalize\AppsUseLightTheme` を使い、タイトルバーは
//! DWMWA_USE_IMMERSIVE_DARK_MODE を適用する(Windows 11/10 1809+、
//! 非対応 OS では失敗しても無害)。

use windows::core::BOOL;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

use crate::win::wide::wstr;

pub fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub window_bg: COLORREF,
    pub panel_bg: COLORREF,
    pub text: COLORREF,
    pub dim_text: COLORREF,
    pub gutter_bg: COLORREF,
    pub border: COLORREF,
    pub added_bg: COLORREF,
    pub added_hl: COLORREF,
    pub removed_bg: COLORREF,
    pub removed_hl: COLORREF,
    pub modified_bg: COLORREF,
    pub modified_hl: COLORREF,
    pub info_bg: COLORREF,
    pub accent: COLORREF,
    pub warn: COLORREF,
    pub search_hit: COLORREF,
    pub current_hit: COLORREF,
}

pub fn light() -> Theme {
    Theme {
        dark: false,
        window_bg: rgb(255, 255, 255),
        panel_bg: rgb(245, 245, 245),
        text: rgb(30, 30, 30),
        dim_text: rgb(120, 120, 120),
        gutter_bg: rgb(246, 246, 246),
        border: rgb(210, 210, 210),
        added_bg: rgb(219, 242, 226),
        added_hl: rgb(158, 219, 179),
        removed_bg: rgb(252, 228, 226),
        removed_hl: rgb(242, 174, 171),
        modified_bg: rgb(255, 247, 209),
        modified_hl: rgb(255, 223, 117),
        info_bg: rgb(235, 235, 235),
        accent: rgb(0, 95, 184),
        warn: rgb(196, 107, 0),
        search_hit: rgb(0, 150, 136),
        current_hit: rgb(0, 95, 184),
    }
}

pub fn dark() -> Theme {
    Theme {
        dark: true,
        window_bg: rgb(32, 32, 32),
        panel_bg: rgb(40, 40, 40),
        text: rgb(230, 230, 230),
        dim_text: rgb(160, 160, 160),
        gutter_bg: rgb(38, 38, 38),
        border: rgb(80, 80, 80),
        added_bg: rgb(23, 61, 33),
        added_hl: rgb(46, 105, 64),
        removed_bg: rgb(68, 30, 28),
        removed_hl: rgb(120, 54, 48),
        modified_bg: rgb(75, 63, 18),
        modified_hl: rgb(128, 103, 32),
        info_bg: rgb(48, 48, 48),
        accent: rgb(96, 160, 255),
        warn: rgb(255, 170, 60),
        search_hit: rgb(80, 200, 190),
        current_hit: rgb(96, 160, 255),
    }
}

/// ダークモード有効か(レジストリ AppsUseLightTheme == 0)
pub fn detect_dark() -> bool {
    let subkey = wstr("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = wstr("AppsUseLightTheme");
    let mut data: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            pcwstr_helper(&subkey),
            pcwstr_helper(&value),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok() && data == 0
}

fn pcwstr_helper(v: &[u16]) -> windows::core::PCWSTR {
    windows::core::PCWSTR(v.as_ptr())
}

pub fn current() -> Theme {
    if detect_dark() {
        dark()
    } else {
        light()
    }
}

/// タイトルバーへダークモード適用(失敗しても無害)
pub fn apply_title_bar(hwnd: HWND, dark: bool) {
    let val: BOOL = BOOL(dark as i32);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20), // DWMWA_USE_IMMERSIVE_DARK_MODE
            &val as *const BOOL as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}
