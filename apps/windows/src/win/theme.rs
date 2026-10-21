//! 配色テーマ(ライト/ダーク)。ダーク検出は個人設定レジストリ値
//! `HKCU\...\Personalize\AppsUseLightTheme` を使い、タイトルバーは
//! DWMWA_USE_IMMERSIVE_DARK_MODE を適用する(Windows 11/10 1809+、
//! 非対応 OS では失敗しても無害)。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, FillRect, SetBkColor, SetBkMode, SetTextColor, HBRUSH, HDC, OPAQUE,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetClientRect, GetWindowLongW, GWL_STYLE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
};

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

static DARK: AtomicBool = AtomicBool::new(false);
static BRUSHES: [AtomicIsize; 2] = [AtomicIsize::new(0), AtomicIsize::new(0)];

/// レジストリを読み直してダーク判定を更新する(起動時と設定変更通知時)
pub fn refresh() -> bool {
    let d = detect_dark();
    DARK.store(d, Ordering::Relaxed);
    d
}

/// スモーク用: ダーク/ライトを強制する
pub fn force(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

/// 直近の `refresh()` 結果に基づくテーマ(メッセージごとに呼んでも軽い)
pub fn cached() -> Theme {
    if DARK.load(Ordering::Relaxed) {
        dark()
    } else {
        light()
    }
}

/// テーマ背景色のブラシ(プロセス存続中キャッシュ。解放しない)
pub fn bg_brush(t: &Theme) -> HBRUSH {
    let i = t.dark as usize;
    let cur = BRUSHES[i].load(Ordering::Relaxed);
    if cur != 0 {
        return HBRUSH(cur as *mut _);
    }
    let b = unsafe { CreateSolidBrush(t.window_bg) };
    BRUSHES[i].store(b.0 as isize, Ordering::Relaxed);
    b
}

/// `WM_CTLCOLOR*` を処理する。静的テキスト/チェックボックス/グループ枠の背景を
/// テーマ背景に揃え、灰色の帯が出るのを防ぐ。対象外のメッセージは None。
pub fn ctl_color(msg: u32, wparam: WPARAM, t: &Theme) -> Option<LRESULT> {
    let hdc = HDC(wparam.0 as *mut _);
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => unsafe {
            SetTextColor(hdc, t.text);
            SetBkColor(hdc, t.window_bg);
            // 文字ランを不透明背景で塗る: 子の移動で保持された画像の残滓が
            // TRANSPARENT 描画では透けて見える(設定スクロールの残像)のを防ぐ
            SetBkMode(hdc, OPAQUE);
            Some(LRESULT(bg_brush(t).0 as isize))
        },
        // ライトでは既定(白)のままにし、ダークのみ編集系を塗る
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX if t.dark => unsafe {
            SetTextColor(hdc, t.text);
            SetBkColor(hdc, t.panel_bg);
            Some(LRESULT(bg_brush(t).0 as isize))
        },
        _ => None,
    }
}

/// `WM_ERASEBKGND` でクライアント全体をテーマ背景で塗る
pub fn erase_bg(hwnd: HWND, wparam: WPARAM, t: &Theme) -> LRESULT {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        FillRect(HDC(wparam.0 as *mut _), &rc, bg_brush(t));
    }
    LRESULT(1)
}

/// ネイティブ子コントロールへ視覚スタイルのダーク/ライトを適用する(失敗は無害)
pub fn set_control_theme(hwnd: HWND, dark: bool) {
    unsafe {
        if dark && is_labeled_button(hwnd) {
            // DarkMode_Explorer でもチェックボックス・ラジオ・グループ枠の文字は黒のまま
            // 描かれるので、視覚スタイルを外して WM_CTLCOLOR* の文字色で描かせる
            let empty = wstr("");
            let _ = SetWindowTheme(hwnd, PCWSTR(empty.as_ptr()), PCWSTR(empty.as_ptr()));
        } else if dark {
            let name = wstr("DarkMode_Explorer");
            let _ = SetWindowTheme(hwnd, PCWSTR(name.as_ptr()), PCWSTR::null());
        } else {
            // 既定の視覚スタイルへ戻す
            let _ = SetWindowTheme(hwnd, PCWSTR::null(), PCWSTR::null());
        }
    }
}

/// 文字を自前の視覚スタイルで描く BUTTON(チェックボックス・ラジオ・グループ枠)か
fn is_labeled_button(hwnd: HWND) -> bool {
    let mut buf = [0u16; 16];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    if !String::from_utf16_lossy(&buf[..n]).eq_ignore_ascii_case("Button") {
        return false;
    }
    let ty = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & 0x0F;
    // BS_CHECKBOX..BS_GROUPBOX(2..=7)、BS_AUTORADIOBUTTON(9)
    matches!(ty, 2..=7 | 9)
}
