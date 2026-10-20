//! 画像比較ペイン("DiffLoupeImagePane")。表示モード(左右/重ね合わせ/
//! ブリンク/差分/スライス)・不透明度・閾値・境界・ズーム/パンを提供する。
//! デコード・差分生成はワーカースレッドで行い WM_APP_IMG_LOADED で受け取る。

use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::SS_LEFT;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::imagemodel::*;
use crate::win::dpi::scale;
use crate::win::theme::{self, Theme};
use crate::win::wide::wstr;

pub const CLASS_NAME: &str = "DiffLoupeImagePane";
const WM_APP_IMG_LOADED: u32 = WM_APP + 50;
const TIMER_BLINK: usize = 1;
const TIMER_DEBOUNCE: usize = 2;
// windows クレート 0.62 には TBM_GETPOS が無いため WM_USER+0(=0x0400)を定義する
const TBM_GETPOS: u32 = 0x0400;

const ID_MODE: i32 = 10;
const ID_TRACK: i32 = 11;
const ID_ZTRACK: i32 = 12;
const ID_ZOUT: i32 = 13;
const ID_ZIN: i32 = 14;
const ID_ZFIT: i32 = 15;
const ID_LBL1: i32 = 16;
const ID_LBL2: i32 = 17;
const ID_LBLZ: i32 = 18;
const ID_META: i32 = 19;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    SideBySide,
    Overlay,
    Blink,
    Diff,
    Slice,
}

impl Mode {
    const ALL: [Mode; 5] = [
        Self::SideBySide,
        Self::Overlay,
        Self::Blink,
        Self::Diff,
        Self::Slice,
    ];
    fn label(&self) -> &'static str {
        use crate::strings::*;
        match self {
            Self::SideBySide => IMG_SIDE_BY_SIDE,
            Self::Overlay => IMG_OVERLAY,
            Self::Blink => IMG_BLINK,
            Self::Diff => IMG_DIFF,
            Self::Slice => IMG_SLICE,
        }
    }
}

struct Loaded {
    left: RgbaImage,
    right: RgbaImage,
    diff: RgbaImage,
    lmeta: ImageFileMetadata,
    rmeta: ImageFileMetadata,
    stats: ImageDiffStats,
    // 描画用 DIB ビット(キャンバスで StretchDIBits に使う)
    l_dib: HBITMAP,
    l_bits: *mut core::ffi::c_void,
    r_dib: HBITMAP,
    r_bits: *mut core::ffi::c_void,
    d_dib: HBITMAP,
    d_bits: *mut core::ffi::c_void,
}

struct Pane {
    theme: Theme,
    font: HFONT,
    left_path: PathBuf,
    right_path: PathBuf,
    mode: Mode,
    opacity: i32,
    threshold: i32,
    slice: i32,
    vp: ImageViewport,
    loaded: Option<Box<Loaded>>,
    message: String,
    blink_right: bool,
    dragging: bool,
    drag_x: i32,
    drag_y: i32,
    // 子コントロール
    h_mode: HWND,
    h_track: HWND,
    h_ztrack: HWND,
    h_zout: HWND,
    h_zin: HWND,
    h_zfit: HWND,
    h_lbl1: HWND,
    h_lbl2: HWND,
    h_lblz: HWND,
    h_meta: HWND,
}

pub fn register_class(hinst: HINSTANCE) {
    let name = wstr(CLASS_NAME);
    let wc = WNDCLASSW {
        hInstance: hinst,
        lpszClassName: windows::core::PCWSTR(name.as_ptr()),
        lpfnWndProc: Some(wndproc),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }
}

/// (hwnd, left, right)
pub fn create(parent: HWND, id: i32, left: PathBuf, right: PathBuf) -> HWND {
    let class = wstr(CLASS_NAME);
    let st = Box::new(Pane {
        theme: theme::current(),
        font: HFONT::default(),
        left_path: left,
        right_path: right,
        mode: Mode::SideBySide,
        opacity: 50,
        threshold: DEFAULT_THRESHOLD,
        slice: 50,
        vp: ImageViewport::default(),
        loaded: None,
        message: crate::strings::IMG_LOADING.into(),
        blink_right: false,
        dragging: false,
        drag_x: 0,
        drag_y: 0,
        h_mode: HWND::default(),
        h_track: HWND::default(),
        h_ztrack: HWND::default(),
        h_zout: HWND::default(),
        h_zin: HWND::default(),
        h_zfit: HWND::default(),
        h_lbl1: HWND::default(),
        h_lbl2: HWND::default(),
        h_lblz: HWND::default(),
        h_meta: HWND::default(),
    });
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            GetModuleHandleW(None).ok().map(|m| m.into()),
            Some(Box::into_raw(st) as *const _),
        )
        .unwrap_or_default()
    }
}

fn st(hwnd: HWND) -> &'static mut Pane {
    unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Pane) }
}

fn mk_w(
    hinst: HINSTANCE,
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
    parent: HWND,
    id: i32,
) -> HWND {
    let t = wstr(text);
    unsafe {
        CreateWindowExW(
            ex,
            class,
            windows::core::PCWSTR(t.as_ptr()),
            style | WS_CHILD | WS_VISIBLE,
            0,
            0,
            10,
            10,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(hinst),
            None,
        )
        .unwrap_or_default()
    }
}

fn mk(
    hinst: HINSTANCE,
    class: &str,
    text: &str,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
    parent: HWND,
    id: i32,
) -> HWND {
    let c = wstr(class);
    mk_w(hinst, PCWSTR(c.as_ptr()), text, style, ex, parent, id)
}

fn on_create(hwnd: HWND) {
    let s = st(hwnd);
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None) }.unwrap_or_default().into();
    s.font = super::app::make_font(hwnd, false);
    s.h_mode = mk(
        hinst,
        "COMBOBOX",
        "",
        WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_MODE,
    );
    for m in Mode::ALL {
        let t = wstr(m.label());
        unsafe {
            SendMessageW(
                s.h_mode,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(t.as_ptr() as isize)),
            );
        }
    }
    unsafe {
        SendMessageW(s.h_mode, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    let track_style = WINDOW_STYLE(TBS_HORZ | TBS_BOTH);
    s.h_track = mk_w(
        hinst,
        TRACKBAR_CLASS,
        "",
        track_style,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_TRACK,
    );
    s.h_ztrack = mk_w(
        hinst,
        TRACKBAR_CLASS,
        "",
        track_style,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZTRACK,
    );
    s.h_zout = mk(
        hinst,
        "BUTTON",
        "-",
        WINDOW_STYLE(BS_PUSHBUTTON as u32),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZOUT,
    );
    s.h_zin = mk(
        hinst,
        "BUTTON",
        "+",
        WINDOW_STYLE(BS_PUSHBUTTON as u32),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZIN,
    );
    s.h_zfit = mk(
        hinst,
        "BUTTON",
        "フィット",
        WINDOW_STYLE(BS_PUSHBUTTON as u32),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZFIT,
    );
    s.h_lbl1 = mk(
        hinst,
        "STATIC",
        "",
        WINDOW_STYLE(SS_LEFT.0),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBL1,
    );
    s.h_lbl2 = mk(
        hinst,
        "STATIC",
        "",
        WINDOW_STYLE(SS_LEFT.0),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBL2,
    );
    s.h_lblz = mk(
        hinst,
        "STATIC",
        "100%",
        WINDOW_STYLE(SS_LEFT.0),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBLZ,
    );
    s.h_meta = mk(
        hinst,
        "STATIC",
        "",
        WINDOW_STYLE(SS_LEFT.0),
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_META,
    );
    for c in [
        s.h_mode, s.h_track, s.h_ztrack, s.h_zout, s.h_zin, s.h_zfit, s.h_lbl1, s.h_lbl2, s.h_lblz,
        s.h_meta,
    ] {
        unsafe {
            SendMessageW(
                c,
                WM_SETFONT,
                Some(WPARAM(s.font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }
    // ズームトラック: 25..400(%)
    unsafe {
        SendMessageW(
            s.h_ztrack,
            TBM_SETRANGE,
            Some(WPARAM(1)),
            Some(LPARAM((25 << 16) | 400)),
        );
        SendMessageW(s.h_ztrack, TBM_SETPOS, Some(WPARAM(1)), Some(LPARAM(100)));
        SendMessageW(
            s.h_track,
            TBM_SETRANGE,
            Some(WPARAM(1)),
            Some(LPARAM(50 << 16)),
        );
        SendMessageW(s.h_track, TBM_SETPOS, Some(WPARAM(1)), Some(LPARAM(50)));
    }
    update_toolbar(hwnd);
    start_load(hwnd);
}

fn update_toolbar(hwnd: HWND) {
    let s = st(hwnd);
    let (lbl, _min, _max, val, show) = match s.mode {
        Mode::Overlay => (crate::strings::IMG_OPACITY, 0, 100, s.opacity, true),
        Mode::Diff => (crate::strings::IMG_THRESHOLD, 0, 255, s.threshold, true),
        Mode::Slice => (crate::strings::IMG_BOUNDARY, 0, 100, s.slice, true),
        _ => ("", 0, 100, 0, false),
    };
    unsafe {
        let lw = wstr(lbl);
        let l2 = wstr(&format!(
            "{}{}",
            val,
            if s.mode == Mode::Diff { "" } else { "%" }
        ));
        let _ = SetWindowTextW(s.h_lbl1, PCWSTR(lw.as_ptr()));
        let _ = SetWindowTextW(s.h_lbl2, PCWSTR(l2.as_ptr()));
        let _ = ShowWindow(s.h_track, if show { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(s.h_lbl1, if show { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(s.h_lbl2, if show { SW_SHOW } else { SW_HIDE });
    }
}

/// 画像読み込みをワーカーへ投げる
fn start_load(hwnd: HWND) {
    let s = st(hwnd);
    let (l, r, th) = (s.left_path.clone(), s.right_path.clone(), s.threshold);
    // HWND は Send でないため生ポインタを usize として持ち回る
    let hwnd = hwnd.0 as usize;
    std::thread::spawn(move || {
        let result = (|| -> Result<Loaded, String> {
            let li = super::wic::decode(&l)?;
            let ri = super::wic::decode(&r)?;
            let lmeta = super::wic::metadata(&l, li.width, li.height);
            let rmeta = super::wic::metadata(&r, ri.width, ri.height);
            let cmp = compare(li.clone(), ri.clone(), lmeta.clone(), rmeta.clone(), th);
            unsafe {
                let (l_dib, l_bits) = make_dib(&li);
                let (r_dib, r_bits) = make_dib(&ri);
                let (d_dib, d_bits) = make_dib(&cmp.diff_image);
                Ok(Loaded {
                    left: li,
                    right: ri,
                    diff: cmp.diff_image,
                    lmeta,
                    rmeta,
                    stats: cmp.stats,
                    l_dib,
                    l_bits,
                    r_dib,
                    r_bits,
                    d_dib,
                    d_bits,
                })
            }
        })();
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        unsafe {
            let _ = PostMessageW(
                Some(hwnd),
                WM_APP_IMG_LOADED,
                WPARAM(0),
                LPARAM(Box::into_raw(Box::new(result)) as isize),
            );
        }
    });
}

/// RGBA8 → 32bpp DIBSection(BGRA)を作る
unsafe fn make_dib(img: &RgbaImage) -> (HBITMAP, *mut core::ffi::c_void) {
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: img.width as i32,
                biHeight: -(img.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bmp =
            CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        if !bits.is_null() {
            // RGBA → BGRA
            for px in img.pixels.as_chunks::<4>().0 {
                let i = px.as_ptr().offset_from(img.pixels.as_ptr()) as usize;
                let dst = (bits as *mut u8).add(i);
                *dst = px[2];
                *dst.add(1) = px[1];
                *dst.add(2) = px[0];
                *dst.add(3) = px[3];
            }
        }
        (bmp, bits)
    }
}

fn layout(hwnd: HWND) {
    let s = st(hwnd);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let w = rc.right;
    let pad = scale(hwnd, 8);
    let h26 = scale(hwnd, 24);
    let y = pad;
    let mut x = pad;
    let mv = |h: HWND, x: i32, y: i32, w: i32, hh: i32| unsafe {
        let _ = MoveWindow(h, x, y, w, hh, true);
    };
    mv(s.h_mode, x, y, scale(hwnd, 150), scale(hwnd, 200));
    x += scale(hwnd, 150) + pad;
    if matches!(s.mode, Mode::Overlay | Mode::Diff | Mode::Slice) {
        mv(s.h_lbl1, x, y + 2, scale(hwnd, 48), h26);
        x += scale(hwnd, 48);
        mv(s.h_track, x, y, scale(hwnd, 130), h26);
        x += scale(hwnd, 130) + 2;
        mv(s.h_lbl2, x, y + 2, scale(hwnd, 40), h26);
        x += scale(hwnd, 40) + pad;
    }
    // zoom controls
    mv(s.h_zout, x, y, h26, h26);
    x += h26 + 2;
    mv(s.h_ztrack, x, y, scale(hwnd, 130), h26);
    x += scale(hwnd, 130) + 2;
    mv(s.h_zin, x, y, h26, h26);
    x += h26 + 2;
    mv(s.h_lblz, x, y + 2, scale(hwnd, 44), h26);
    x += scale(hwnd, 44) + 2;
    mv(s.h_zfit, x, y, scale(hwnd, 56), h26);
    // メタ行
    let meta_y = y + h26 + pad;
    let meta_h = scale(hwnd, 56);
    mv(s.h_meta, pad, meta_y, w - pad * 2, meta_h);
}

fn meta_text(s: &Pane) -> String {
    let Some(l) = &s.loaded else {
        return s.message.clone();
    };
    use crate::strings::*;
    let mut warn = String::new();
    if l.stats.used_downsampled_comparison {
        warn = format!("  {}", IMG_DOWNSAMPLED);
    } else if l.lmeta.width != l.rmeta.width || l.lmeta.height != l.rmeta.height {
        warn = format!("  {}", IMG_DIM_DIFFERS);
    }
    format!(
        "       {}  {}  {}\r\n  {}: {}    {}    {}\r\n  {}: {}    {}    {}\r\n{}: {}    {} px    {:.2}%{}",
        IMG_DIM, IMG_FILESIZE, IMG_FORMAT,
        IMG_LEFT, l.lmeta.dimensions_text(), l.lmeta.file_size_text(), l.lmeta.format,
        IMG_RIGHT, l.rmeta.dimensions_text(), l.rmeta.file_size_text(), l.rmeta.format,
        IMG_DIFFSTAT, l.stats.compared_dimensions_text(), l.stats.diff_pixel_count, l.stats.diff_percentage, warn
    )
}

fn paint(hwnd: HWND) {
    let s = st(hwnd);
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let bar_h = scale(hwnd, 24) + scale(hwnd, 8) * 2 + scale(hwnd, 56);
        // 背景
        let bg = CreateSolidBrush(s.theme.window_bg);
        let canvas_rect = RECT {
            top: bar_h + 4,
            ..rc
        };
        FillRect(hdc, &canvas_rect, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));
        let Some(l) = &s.loaded else {
            let brush = CreateSolidBrush(s.theme.window_bg);
            FillRect(hdc, &rc, brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));
            SetTextColor(hdc, s.theme.dim_text);
            SetBkMode(hdc, TRANSPARENT);
            let mut tr = rc;
            let msg = wstr(&s.message);
            let mut mv = msg[..msg.len() - 1].to_vec();
            let _ = DrawTextW(
                hdc,
                &mut mv,
                &mut tr,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            let _ = EndPaint(hwnd, &ps);
            return;
        };
        let cv = canvas_rect;
        let (_cw, _ch) = ((cv.right - cv.left) as f64, (cv.bottom - cv.top) as f64);
        let draw_img = |img: &RgbaImage,
                        dib: HBITMAP,
                        bits: *mut core::ffi::c_void,
                        area: RECT,
                        opacity_255: u8| {
            let (fw, fh) = ImageViewport::fitted_size(
                img.width as f64,
                img.height as f64,
                (area.right - area.left) as f64,
                (area.bottom - area.top) as f64,
            );
            let dw = fw * s.vp.scale;
            let dh = fh * s.vp.scale;
            let x = area.left
                + ((area.right - area.left) as f64 / 2.0 + s.vp.offset_x - dw / 2.0) as i32;
            let y = area.top
                + ((area.bottom - area.top) as f64 / 2.0 + s.vp.offset_y - dh / 2.0) as i32;
            let mem = CreateCompatibleDC(Some(hdc));
            SelectObject(mem, HGDIOBJ(dib.0));
            SetStretchBltMode(hdc, HALFTONE);
            let _ = StretchBlt(
                hdc,
                x,
                y,
                dw as i32,
                dh as i32,
                Some(mem),
                0,
                0,
                img.width as i32,
                img.height as i32,
                SRCCOPY,
            );
            let _ = DeleteDC(mem);
            let _ = bits;
            let _ = opacity_255;
        };
        match s.mode {
            Mode::SideBySide => {
                let half = cv.right / 2;
                let a = RECT { right: half, ..cv };
                let b = RECT { left: half, ..cv };
                draw_img(&l.left, l.l_dib, l.l_bits, a, 255);
                draw_img(&l.right, l.r_dib, l.r_bits, b, 255);
            }
            Mode::Overlay => {
                draw_img(&l.left, l.l_dib, l.l_bits, cv, 255);
                draw_img(&l.right, l.r_dib, l.r_bits, cv, s.opacity as u8);
            }
            Mode::Blink => {
                if s.blink_right {
                    draw_img(&l.right, l.r_dib, l.r_bits, cv, 255);
                } else {
                    draw_img(&l.left, l.l_dib, l.l_bits, cv, 255);
                }
            }
            Mode::Diff => {
                draw_img(&l.diff, l.d_dib, l.d_bits, cv, 255);
            }
            Mode::Slice => {
                // 左画像を境界まで、右画像を境界から描く(横分割)
                let frac = s.slice as f64 / 100.0;
                let canvas_w = (cv.right - cv.left) as f64;
                let (fw, fh) = ImageViewport::fitted_size(
                    l.left.width.max(l.right.width) as f64,
                    l.left.height.max(l.right.height) as f64,
                    canvas_w,
                    (cv.bottom - cv.top) as f64,
                );
                let dw = fw * s.vp.scale;
                let dh = fh * s.vp.scale;
                let ox = cv.left as f64 + canvas_w / 2.0 + s.vp.offset_x - dw / 2.0;
                let oy =
                    cv.top as f64 + (cv.bottom - cv.top) as f64 / 2.0 + s.vp.offset_y - dh / 2.0;
                let divx = ox + dw * frac;
                // 左: クリップ [cv.left, divx)
                let draw_clip = |img: &RgbaImage, dib: HBITMAP, left_clip: i32, right_clip: i32| {
                    let mem = CreateCompatibleDC(Some(hdc));
                    SelectObject(mem, HGDIOBJ(dib.0));
                    SetStretchBltMode(hdc, HALFTONE);
                    let src_w = img.width as f64;
                    // クリップ範囲をソース座標へ
                    let sx0 = ((left_clip as f64 - ox) / dw * src_w).clamp(0.0, src_w) as i32;
                    let sx1 = ((right_clip as f64 - ox) / dw * src_w).clamp(0.0, src_w) as i32;
                    if sx1 > sx0 {
                        let _ = StretchBlt(
                            hdc,
                            left_clip.max(cv.left),
                            oy as i32,
                            right_clip.min(cv.right) - left_clip.max(cv.left),
                            dh as i32,
                            Some(mem),
                            sx0,
                            0,
                            sx1 - sx0,
                            img.height as i32,
                            SRCCOPY,
                        );
                    }
                    let _ = DeleteDC(mem);
                };
                draw_clip(&l.left, l.l_dib, cv.left, divx as i32);
                draw_clip(&l.right, l.r_dib, divx as i32, cv.right);
                // 境界線
                let pen = CreatePen(PS_SOLID, 1, s.theme.accent);
                let old = SelectObject(hdc, HGDIOBJ(pen.0));
                let _ = MoveToEx(hdc, divx as i32, cv.top, None);
                let _ = LineTo(hdc, divx as i32, cv.bottom);
                SelectObject(hdc, old);
                let _ = DeleteObject(HGDIOBJ(pen.0));
            }
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

fn recompute_diff(hwnd: HWND) {
    // 閾値変更: 既存のデコード済み画像から差分画像を再生成(同期・高速)
    let s = st(hwnd);
    if s.loaded.is_none() {
        return;
    }
    let l = s.loaded.as_ref().unwrap();
    let cmp = compare(
        l.left.clone(),
        l.right.clone(),
        l.lmeta.clone(),
        l.rmeta.clone(),
        s.threshold,
    );
    unsafe {
        let (d_dib, d_bits) = make_dib(&cmp.diff_image);
        let s2 = st(hwnd);
        if let Some(l2) = s2.loaded.as_mut() {
            let _ = DeleteObject(HGDIOBJ(l2.d_dib.0));
            l2.diff = cmp.diff_image;
            l2.stats = cmp.stats;
            l2.d_dib = d_dib;
            l2.d_bits = d_bits;
        }
        let _ = SetWindowTextW(s2.h_meta, PCWSTR(wstr(&meta_text(s2)).as_ptr()));
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

fn sync_zoom_ui(s: &Pane) {
    unsafe {
        let v = (s.vp.scale * 100.0) as i64;
        SendMessageW(
            s.h_ztrack,
            TBM_SETPOS,
            Some(WPARAM(1)),
            Some(LPARAM(v as isize)),
        );
        let _ = SetWindowTextW(s.h_lblz, PCWSTR(wstr(&format!("{}%", v)).as_ptr()));
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            on_create(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_BLINK);
                    let _ = KillTimer(Some(hwnd), TIMER_DEBOUNCE);
                    let _ = Box::from_raw(ptr as *mut Pane);
                }
            }
            LRESULT(0)
        }
        WM_SIZE => {
            layout(hwnd);
            LRESULT(0)
        }
        WM_APP_IMG_LOADED => {
            let boxed = unsafe { Box::from_raw(lparam.0 as *mut Result<Loaded, String>) };
            let s = st(hwnd);
            match *boxed {
                Ok(l) => {
                    s.loaded = Some(Box::new(l));
                    s.message.clear();
                    unsafe {
                        let _ = SetWindowTextW(s.h_meta, PCWSTR(wstr(&meta_text(s)).as_ptr()));
                        let _ = SetTimer(Some(hwnd), TIMER_BLINK, 650, None);
                    }
                }
                Err(e) => {
                    s.message = e;
                    unsafe {
                        let _ = SetWindowTextW(s.h_meta, PCWSTR(wstr(&s.message).as_ptr()));
                    }
                }
            }
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_TIMER => match wparam.0 {
            TIMER_BLINK => {
                let s = st(hwnd);
                if s.mode == Mode::Blink {
                    s.blink_right = !s.blink_right;
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            TIMER_DEBOUNCE => {
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_DEBOUNCE);
                }
                recompute_diff(hwnd);
                LRESULT(0)
            }
            _ => LRESULT(0),
        },
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            if id == ID_MODE && code == CBN_SELCHANGE as u16 {
                let s = st(hwnd);
                let sel = unsafe {
                    SendMessageW(s.h_mode, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)))
                }
                .0;
                if let Some(m) = Mode::ALL.get(sel as usize) {
                    s.mode = *m;
                    s.vp.offset_x = 0.0;
                    s.vp.offset_y = 0.0;
                    update_toolbar(hwnd);
                    layout(hwnd);
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
            }
            let s = st(hwnd);
            match id {
                ID_ZOUT => {
                    s.vp.scale = ImageViewport::clamped_scale(s.vp.scale / 1.25);
                    sync_zoom_ui(s);
                }
                ID_ZIN => {
                    s.vp.scale = ImageViewport::clamped_scale(s.vp.scale * 1.25);
                    sync_zoom_ui(s);
                }
                ID_ZFIT => {
                    s.vp = ImageViewport::default();
                    sync_zoom_ui(s);
                }
                _ => {}
            }
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_HSCROLL => {
            // トラックバー通知
            let s = st(hwnd);
            let th =
                unsafe { SendMessageW(s.h_track, TBM_GETPOS, Some(WPARAM(0)), Some(LPARAM(0))) }.0
                    as i32;
            let zv =
                unsafe { SendMessageW(s.h_ztrack, TBM_GETPOS, Some(WPARAM(0)), Some(LPARAM(0))) }.0
                    as i32;
            if HWND(lparam.0 as *mut _) == s.h_track {
                match s.mode {
                    Mode::Overlay => s.opacity = th,
                    Mode::Diff => {
                        s.threshold = th;
                        unsafe {
                            let _ = KillTimer(Some(hwnd), TIMER_DEBOUNCE);
                            let _ = SetTimer(Some(hwnd), TIMER_DEBOUNCE, 400, None);
                        }
                    }
                    Mode::Slice => s.slice = th,
                    _ => {}
                }
                update_toolbar(hwnd);
            } else if HWND(lparam.0 as *mut _) == s.h_ztrack {
                s.vp.scale = ImageViewport::clamped_scale(zv as f64 / 100.0);
                sync_zoom_ui(s);
            }
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            let s = st(hwnd);
            s.dragging = true;
            s.drag_x = lparam.0 as i32 as i16 as i32;
            s.drag_y = (lparam.0 >> 16) as i32 as i16 as i32;
            unsafe {
                let _ = SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            st(hwnd).dragging = false;
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let s = st(hwnd);
            if s.dragging {
                let x = lparam.0 as i32 as i16 as i32;
                let y = (lparam.0 >> 16) as i32 as i16 as i32;
                s.vp.offset_x += (x - s.drag_x) as f64;
                s.vp.offset_y += (y - s.drag_y) as f64;
                s.drag_x = x;
                s.drag_y = y;
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
