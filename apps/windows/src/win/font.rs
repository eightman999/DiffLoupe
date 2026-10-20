//! フォント生成とテキスト実測。
//!
//! UI フォントは `SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS)` の
//! `lfMessageFont`(日本語環境では Yu Gothic UI / Meiryo UI 相当)を使い、
//! 差分本文は等幅フォント(Cascadia Mono → Consolas → MS Gothic)を使う。
//! いずれも DPI を引数に取り、`WM_DPICHANGED` で作り直せる。

use std::sync::OnceLock;
use windows::Win32::Foundation::{LPARAM, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS};

use crate::win::wide::wstr;

fn set_face(lf: &mut LOGFONTW, face: &str) {
    let mut fw = wstr(face);
    fw.truncate(LF_FACESIZE as usize - 1);
    lf.lfFaceName = [0; LF_FACESIZE as usize];
    lf.lfFaceName[..fw.len()].copy_from_slice(&fw);
}

/// システムのメッセージフォント(取得失敗時は Yu Gothic UI 9pt)
pub fn ui_logfont(dpi: u32) -> LOGFONTW {
    let mut ncm = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut core::ffi::c_void),
            0,
            dpi,
        )
    }
    .is_ok();
    if ok && ncm.lfMessageFont.lfHeight != 0 {
        let mut lf = ncm.lfMessageFont;
        lf.lfWeight = 400;
        return lf;
    }
    let mut lf = LOGFONTW {
        lfHeight: -((dpi as i32) * 9 / 72),
        lfWeight: 400,
        lfCharSet: DEFAULT_CHARSET,
        lfQuality: CLEARTYPE_QUALITY,
        ..Default::default()
    };
    set_face(&mut lf, "Yu Gothic UI");
    lf
}

pub fn make_ui_font(dpi: u32) -> HFONT {
    unsafe { CreateFontIndirectW(&ui_logfont(dpi)) }
}

pub fn make_bold_font(dpi: u32) -> HFONT {
    let mut lf = ui_logfont(dpi);
    lf.lfWeight = 700;
    unsafe { CreateFontIndirectW(&lf) }
}

unsafe extern "system" fn enum_proc(
    _lf: *const LOGFONTW,
    _tm: *const TEXTMETRICW,
    _ty: u32,
    lparam: LPARAM,
) -> i32 {
    unsafe {
        *(lparam.0 as *mut bool) = true;
    }
    0 // 1 件見つかれば十分
}

fn font_exists(face: &str) -> bool {
    let mut lf = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    set_face(&mut lf, face);
    let mut found = false;
    unsafe {
        let hdc = CreateCompatibleDC(None);
        EnumFontFamiliesExW(
            hdc,
            &lf,
            Some(enum_proc),
            LPARAM(&mut found as *mut bool as isize),
            0,
        );
        let _ = DeleteDC(hdc);
    }
    found
}

/// 等幅フォント名(Cascadia Mono → Consolas → MS Gothic)
pub fn mono_face() -> &'static str {
    static FACE: OnceLock<&'static str> = OnceLock::new();
    FACE.get_or_init(|| {
        ["Cascadia Mono", "Consolas"]
            .into_iter()
            .find(|f| font_exists(f))
            .unwrap_or("MS Gothic")
    })
}

pub fn make_mono_font(dpi: u32) -> HFONT {
    let mut lf = LOGFONTW {
        lfHeight: -((dpi as i32) * 10 / 72),
        lfWeight: 400,
        lfCharSet: DEFAULT_CHARSET,
        lfQuality: CLEARTYPE_QUALITY,
        lfPitchAndFamily: 49, // FIXED_PITCH | FF_MODERN
        ..Default::default()
    };
    set_face(&mut lf, mono_face());
    unsafe { CreateFontIndirectW(&lf) }
}

/// フォントの文字高(tmHeight)
pub fn font_height(font: HFONT) -> i32 {
    unsafe {
        let hdc = CreateCompatibleDC(None);
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        let mut tm = TEXTMETRICW::default();
        let _ = GetTextMetricsW(hdc, &mut tm);
        SelectObject(hdc, old);
        let _ = DeleteDC(hdc);
        tm.tmHeight
    }
}

/// 1 行テキストの実測サイズ
pub fn measure(font: HFONT, text: &str) -> SIZE {
    let w: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    unsafe {
        let hdc = CreateCompatibleDC(None);
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        let _ = GetTextExtentPoint32W(hdc, &w, &mut size);
        SelectObject(hdc, old);
        let _ = DeleteDC(hdc);
    }
    size
}

/// 指定幅で折り返したときのテキスト高さ(`DT_CALCRECT`)
pub fn calc_text_height(font: HFONT, text: &str, width: i32) -> i32 {
    let mut w: Vec<u16> = text.encode_utf16().collect();
    if w.is_empty() {
        return font_height(font);
    }
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: width.max(1),
        bottom: 0,
    };
    unsafe {
        let hdc = CreateCompatibleDC(None);
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        DrawTextW(
            hdc,
            &mut w,
            &mut rc,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EXPANDTABS,
        );
        SelectObject(hdc, old);
        let _ = DeleteDC(hdc);
    }
    rc.bottom - rc.top
}

/// UI フォントの書体名(診断ログ用)
pub fn ui_face_name(dpi: u32) -> String {
    let lf = ui_logfont(dpi);
    let end = lf
        .lfFaceName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(lf.lfFaceName.len());
    String::from_utf16_lossy(&lf.lfFaceName[..end])
}
