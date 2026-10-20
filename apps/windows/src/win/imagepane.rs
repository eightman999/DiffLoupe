//! 画像比較ペイン("DiffLoupeImagePane")。表示モード(左右/重ね合わせ/
//! ブリンク/差分/スライス)・不透明度・閾値・境界・ズーム/パンを提供する。
//! デコード・差分生成はワーカースレッドで行い WM_APP_IMG_LOADED で受け取る。

use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{SS_CENTER, SS_CENTERIMAGE, SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::imagemodel::*;
use crate::layout::{compute_image_pane, ImageInput, Rect as LRect};
use crate::win::dpi::scale;
use crate::win::font;
use crate::win::theme::{self};
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
    r_dib: HBITMAP,
    d_dib: HBITMAP,
}

struct Pane {
    /// 親が WM_SETFONT で渡す UI フォント(所有しない)
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
    /// キャンバス/情報テーブルの直近の矩形(診断・パン上限用)
    canvas: LRect,
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
        canvas: LRect::default(),
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
    let tab = WS_TABSTOP;
    s.h_mode = mk(
        hinst,
        "COMBOBOX",
        "",
        WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_VSCROLL | tab,
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
    let track_style = WINDOW_STYLE(TBS_HORZ | TBS_NOTICKS) | tab;
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
    let push = WINDOW_STYLE(BS_PUSHBUTTON as u32) | tab;
    s.h_zout = mk(
        hinst,
        "BUTTON",
        "-",
        push,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZOUT,
    );
    s.h_zin = mk(hinst, "BUTTON", "+", push, WINDOW_EX_STYLE(0), hwnd, ID_ZIN);
    s.h_zfit = mk(
        hinst,
        "BUTTON",
        "フィット",
        push,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_ZFIT,
    );
    let right_lbl = WINDOW_STYLE(SS_CENTER.0 | SS_CENTERIMAGE.0 | SS_NOPREFIX.0);
    let left_lbl = WINDOW_STYLE(SS_LEFT.0 | SS_CENTERIMAGE.0 | SS_NOPREFIX.0);
    s.h_lbl1 = mk(
        hinst,
        "STATIC",
        "",
        left_lbl,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBL1,
    );
    s.h_lbl2 = mk(
        hinst,
        "STATIC",
        "",
        right_lbl,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBL2,
    );
    s.h_lblz = mk(
        hinst,
        "STATIC",
        "100%",
        right_lbl,
        WINDOW_EX_STYLE(0),
        hwnd,
        ID_LBLZ,
    );
    // ズームトラック: 25..400(%)
    unsafe {
        SendMessageW(
            s.h_ztrack,
            TBM_SETRANGE,
            Some(WPARAM(1)),
            Some(LPARAM((400 << 16) | 25)),
        );
        SendMessageW(s.h_ztrack, TBM_SETPOS, Some(WPARAM(1)), Some(LPARAM(100)));
    }
    apply_theme(hwnd);
    update_toolbar(hwnd);
    start_load(hwnd);
}

/// 子コントロールへ視覚スタイルのダーク/ライトを適用
fn apply_theme(hwnd: HWND) {
    let dark = theme::cached().dark;
    unsafe {
        let mut child = GetWindow(hwnd, GW_CHILD);
        while let Ok(c) = child {
            theme::set_control_theme(c, dark);
            child = GetWindow(c, GW_HWNDNEXT);
        }
    }
}

/// フォントを全子コントロールへ配り、コンボの閉じた高さを揃える
fn apply_font(hwnd: HWND) {
    let s = st(hwnd);
    if s.font.is_invalid() {
        return;
    }
    unsafe {
        let mut child = GetWindow(hwnd, GW_CHILD);
        while let Ok(c) = child {
            SendMessageW(
                c,
                WM_SETFONT,
                Some(WPARAM(s.font.0 as usize)),
                Some(LPARAM(1)),
            );
            child = GetWindow(c, GW_HWNDNEXT);
        }
        SendMessageW(
            s.h_mode,
            CB_SETITEMHEIGHT,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM((row_h(hwnd, s) - scale(hwnd, 6)) as isize)),
        );
    }
}

fn row_h(hwnd: HWND, s: &Pane) -> i32 {
    (font::font_height(s.font) + scale(hwnd, 10)).max(scale(hwnd, 24))
}

/// モードに応じてスライダーの範囲(不透明度/境界 0-100、閾値 0-255)を設定する
fn set_track_range(s: &Pane) {
    let max = if s.mode == Mode::Diff { 255 } else { 100 };
    unsafe {
        // TBM_SETRANGE: LPARAM = MAKELONG(min, max)
        SendMessageW(
            s.h_track,
            TBM_SETRANGE,
            Some(WPARAM(1)),
            Some(LPARAM((max << 16) as isize)),
        );
    }
}

fn update_toolbar(hwnd: HWND) {
    let s = st(hwnd);
    set_track_range(s);
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
        let cur = match s.mode {
            Mode::Overlay => s.opacity,
            Mode::Diff => s.threshold,
            Mode::Slice => s.slice,
            _ => 0,
        };
        SendMessageW(
            s.h_track,
            TBM_SETPOS,
            Some(WPARAM(1)),
            Some(LPARAM(cur as isize)),
        );
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
                let l_dib = make_dib(&li);
                let r_dib = make_dib(&ri);
                let d_dib = make_dib(&cmp.diff_image);
                Ok(Loaded {
                    left: li,
                    right: ri,
                    diff: cmp.diff_image,
                    lmeta,
                    rmeta,
                    stats: cmp.stats,
                    l_dib,
                    r_dib,
                    d_dib,
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
unsafe fn make_dib(img: &RgbaImage) -> HBITMAP {
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
            // RGBA(premultiplied) → 白背景に合成した不透明 BGRA。
            // 透過画像も黒く潰れず、HALFTONE 縮小がそのまま使える。
            for (n, px) in img.pixels.as_chunks::<4>().0.iter().enumerate() {
                let dst = (bits as *mut u8).add(n * 4);
                let inv = 255 - px[3] as u16;
                *dst = (px[2] as u16 + inv).min(255) as u8;
                *dst.add(1) = (px[1] as u16 + inv).min(255) as u8;
                *dst.add(2) = (px[0] as u16 + inv).min(255) as u8;
                *dst.add(3) = 255;
            }
        }
        bmp
    }
}

/// 情報テーブルの列構成。(見出し, 左, 右, 差分) の 4 行 × 4 列 + 注意書き
fn meta_rows(s: &Pane) -> Option<(Vec<[String; 4]>, Option<String>)> {
    use crate::strings::*;
    let l = s.loaded.as_ref()?;
    let warn = if l.stats.used_downsampled_comparison {
        Some(IMG_DOWNSAMPLED.to_string())
    } else if l.lmeta.width != l.rmeta.width || l.lmeta.height != l.rmeta.height {
        Some(IMG_DIM_DIFFERS.to_string())
    } else {
        None
    };
    let rows = vec![
        [
            String::new(),
            IMG_DIM.to_string(),
            IMG_FILESIZE.to_string(),
            IMG_FORMAT.to_string(),
        ],
        [
            IMG_LEFT.to_string(),
            l.lmeta.dimensions_text(),
            l.lmeta.file_size_text(),
            l.lmeta.format.clone(),
        ],
        [
            IMG_RIGHT.to_string(),
            l.rmeta.dimensions_text(),
            l.rmeta.file_size_text(),
            l.rmeta.format.clone(),
        ],
        [
            IMG_DIFFSTAT.to_string(),
            l.stats.compared_dimensions_text(),
            format!("{} px", l.stats.diff_pixel_count),
            format!("{:.2}%", l.stats.diff_percentage),
        ],
    ];
    Some((rows, warn))
}

fn meta_height(hwnd: HWND, s: &Pane) -> i32 {
    let rh = font::font_height(s.font) + scale(hwnd, 6);
    match meta_rows(s) {
        Some((rows, warn)) => (rows.len() as i32 + warn.is_some() as i32) * rh + scale(hwnd, 4),
        None => 0,
    }
}

fn compute_layout(hwnd: HWND) -> crate::layout::ImageLayout {
    let s = st(hwnd);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let f = s.font;
    let w = |t: &str| font::measure(f, t).cx;
    let dyn_visible = matches!(s.mode, Mode::Overlay | Mode::Diff | Mode::Slice);
    let mode_w = Mode::ALL.iter().map(|m| w(m.label())).max().unwrap_or(0) + scale(hwnd, 44);
    let row = row_h(hwnd, s);
    compute_image_pane(&ImageInput {
        w: rc.right,
        h: rc.bottom,
        pad: scale(hwnd, 12),
        gap: scale(hwnd, 8),
        row,
        mode_w: mode_w.max(scale(hwnd, 120)),
        dyn_visible,
        lbl1_w: [
            crate::strings::IMG_OPACITY,
            crate::strings::IMG_THRESHOLD,
            crate::strings::IMG_BOUNDARY,
        ]
        .iter()
        .map(|t| w(t))
        .max()
        .unwrap_or(0)
            + scale(hwnd, 8),
        track_w: scale(hwnd, 140),
        lbl2_w: w("100%").max(w("255")) + scale(hwnd, 10),
        zbtn_w: row,
        ztrack_w: scale(hwnd, 130),
        lblz_w: w("400%") + scale(hwnd, 10),
        fit_w: w("フィット") + scale(hwnd, 28),
        meta_h: meta_height(hwnd, s),
    })
}

fn layout(hwnd: HWND) {
    if st(hwnd).font.is_invalid() {
        return;
    }
    let l = compute_layout(hwnd);
    let s = st(hwnd);
    s.canvas = l.canvas;
    let mv = |h: HWND, r: LRect| unsafe {
        let _ = MoveWindow(h, r.x, r.y, r.w, r.h, true);
    };
    unsafe {
        let _ = MoveWindow(
            s.h_mode,
            l.mode.x,
            l.mode.y,
            l.mode.w,
            l.mode.h + scale(hwnd, 200),
            true,
        );
    }
    if matches!(s.mode, Mode::Overlay | Mode::Diff | Mode::Slice) {
        mv(s.h_lbl1, l.lbl1);
        mv(s.h_track, l.track);
        mv(s.h_lbl2, l.lbl2);
    }
    mv(s.h_zout, l.zout);
    mv(s.h_ztrack, l.ztrack);
    mv(s.h_zin, l.zin);
    mv(s.h_lblz, l.lblz);
    mv(s.h_zfit, l.zfit);
}

fn fill(hdc: HDC, r: &RECT, c: COLORREF) {
    unsafe {
        let b = CreateSolidBrush(c);
        FillRect(hdc, r, b);
        let _ = DeleteObject(HGDIOBJ(b.0));
    }
}

fn draw_text(hdc: HDC, text: &str, r: &mut RECT, flags: DRAW_TEXT_FORMAT) {
    let mut w: Vec<u16> = text.encode_utf16().collect();
    if w.is_empty() {
        return;
    }
    unsafe {
        DrawTextW(
            hdc,
            &mut w,
            r,
            flags | DT_SINGLELINE | DT_NOPREFIX | DT_VCENTER,
        );
    }
}

/// 下端の情報テーブル(列を揃えて描く)
fn paint_meta(hwnd: HWND, hdc: HDC, s: &Pane, r: &LRect) {
    let Some((rows, warn)) = meta_rows(s) else {
        return;
    };
    let t = theme::cached();
    let rh = font::font_height(s.font) + scale(hwnd, 6);
    let colgap = scale(hwnd, 20);
    // 列幅 = その列の最長テキスト
    let mut cw = [0i32; 4];
    for row in &rows {
        for (i, c) in row.iter().enumerate() {
            cw[i] = cw[i].max(font::measure(s.font, c).cx);
        }
    }
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        for (ri, row) in rows.iter().enumerate() {
            let y = r.y + ri as i32 * rh;
            let mut x = r.x;
            for (ci, c) in row.iter().enumerate() {
                SetTextColor(hdc, if ri == 0 { t.dim_text } else { t.text });
                let mut tr = RECT {
                    left: x,
                    top: y,
                    right: x + cw[ci] + 1,
                    bottom: y + rh,
                };
                draw_text(hdc, c, &mut tr, DT_LEFT);
                x += cw[ci] + colgap;
            }
        }
        if let Some(wn) = warn {
            SetTextColor(hdc, t.warn);
            let y = r.y + rows.len() as i32 * rh;
            let mut tr = RECT {
                left: r.x,
                top: y,
                right: r.x + r.w,
                bottom: y + rh,
            };
            draw_text(hdc, &wn, &mut tr, DT_LEFT | DT_END_ELLIPSIS);
        }
    }
}

/// 画像を `area` 内にフィットさせ、ズーム/パンを反映した描画先矩形を返す
fn dest_rect(s: &Pane, img: &RgbaImage, area: RECT) -> (i32, i32, i32, i32) {
    let (aw, ah) = (
        (area.right - area.left) as f64,
        (area.bottom - area.top) as f64,
    );
    let (fw, fh) = ImageViewport::fitted_size(img.width as f64, img.height as f64, aw, ah);
    let dw = fw * s.vp.scale;
    let dh = fh * s.vp.scale;
    let x = area.left as f64 + aw / 2.0 + s.vp.offset_x - dw / 2.0;
    let y = area.top as f64 + ah / 2.0 + s.vp.offset_y - dh / 2.0;
    (x as i32, y as i32, (dw as i32).max(1), (dh as i32).max(1))
}

/// DIB を (x,y,w,h) へ縮拡して描く。alpha<255 なら AlphaBlend で重ねる。
fn blit(hdc: HDC, dib: HBITMAP, sw: i32, sh: i32, dst: (i32, i32, i32, i32), alpha: u8) {
    let (x, y, w, h) = dst;
    unsafe {
        let mem = CreateCompatibleDC(Some(hdc));
        let old = SelectObject(mem, HGDIOBJ(dib.0));
        if alpha == 255 {
            SetStretchBltMode(hdc, HALFTONE);
            let _ = SetBrushOrgEx(hdc, 0, 0, None);
            let _ = StretchBlt(hdc, x, y, w, h, Some(mem), 0, 0, sw, sh, SRCCOPY);
        } else {
            // いったん縮拡した画像を作り、定数アルファで重ねる
            let tmp = CreateCompatibleDC(Some(hdc));
            let bmp = CreateCompatibleBitmap(hdc, w, h);
            let told = SelectObject(tmp, HGDIOBJ(bmp.0));
            SetStretchBltMode(tmp, HALFTONE);
            let _ = StretchBlt(tmp, 0, 0, w, h, Some(mem), 0, 0, sw, sh, SRCCOPY);
            let bf = BLENDFUNCTION {
                BlendOp: 0, // AC_SRC_OVER
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            let _ = AlphaBlend(hdc, x, y, w, h, tmp, 0, 0, w, h, bf);
            SelectObject(tmp, told);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            let _ = DeleteDC(tmp);
        }
        SelectObject(mem, old);
        let _ = DeleteDC(mem);
    }
}

fn frame(hdc: HDC, d: (i32, i32, i32, i32), c: COLORREF) {
    let (x, y, w, h) = d;
    for r in [
        RECT {
            left: x - 1,
            top: y - 1,
            right: x + w + 1,
            bottom: y,
        },
        RECT {
            left: x - 1,
            top: y + h,
            right: x + w + 1,
            bottom: y + h + 1,
        },
        RECT {
            left: x - 1,
            top: y,
            right: x,
            bottom: y + h,
        },
        RECT {
            left: x + w,
            top: y,
            right: x + w + 1,
            bottom: y + h,
        },
    ] {
        fill(hdc, &r, c);
    }
}

/// クライアント全体を `hdc`(クライアント座標)へ描く
fn paint_into(hwnd: HWND, hdc: HDC) {
    let s = st(hwnd);
    let t = theme::cached();
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    fill(hdc, &rc, t.window_bg);
    let l = compute_layout(hwnd);
    let cvr = RECT {
        left: l.canvas.x,
        top: l.canvas.y,
        right: l.canvas.right(),
        bottom: l.canvas.bottom(),
    };
    // キャンバス枠(コンテンツ領域の見切りを明確にする)
    fill(hdc, &cvr, t.panel_bg);
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        let old_font = SelectObject(
            hdc,
            HGDIOBJ(if s.font.is_invalid() {
                GetStockObject(DEFAULT_GUI_FONT).0
            } else {
                s.font.0
            }),
        );
        let Some(ld) = &s.loaded else {
            SetTextColor(hdc, t.dim_text);
            let mut tr = cvr;
            draw_text(hdc, &s.message, &mut tr, DT_CENTER);
            SelectObject(hdc, old_font);
            return;
        };
        paint_meta(hwnd, hdc, s, &l.meta);
        // 以降はキャンバス内にクリップ(はみ出しを防ぐ)
        let saved = SaveDC(hdc);
        IntersectClipRect(hdc, cvr.left, cvr.top, cvr.right, cvr.bottom);
        let m = scale(hwnd, 8);
        let area = RECT {
            left: cvr.left + m,
            top: cvr.top + m,
            right: (cvr.right - m).max(cvr.left + m + 1),
            bottom: (cvr.bottom - m).max(cvr.top + m + 1),
        };
        let one = |img: &RgbaImage, dib: HBITMAP, a: RECT, alpha: u8, border: bool| {
            let d = dest_rect(s, img, a);
            blit(hdc, dib, img.width as i32, img.height as i32, d, alpha);
            if border {
                frame(hdc, d, t.border);
            }
        };
        match s.mode {
            Mode::SideBySide => {
                let gap = scale(hwnd, 8);
                let half = (area.right - area.left - gap) / 2;
                let a = RECT {
                    right: area.left + half,
                    ..area
                };
                let b = RECT {
                    left: area.right - half,
                    ..area
                };
                one(&ld.left, ld.l_dib, a, 255, true);
                one(&ld.right, ld.r_dib, b, 255, true);
            }
            Mode::Overlay => {
                one(&ld.left, ld.l_dib, area, 255, true);
                one(
                    &ld.right,
                    ld.r_dib,
                    area,
                    (s.opacity * 255 / 100) as u8,
                    false,
                );
            }
            Mode::Blink => {
                if s.blink_right {
                    one(&ld.right, ld.r_dib, area, 255, true);
                } else {
                    one(&ld.left, ld.l_dib, area, 255, true);
                }
            }
            Mode::Diff => one(&ld.diff, ld.d_dib, area, 255, true),
            Mode::Slice => {
                // 同じ矩形に両画像を載せ、境界の左は左画像・右は右画像を見せる
                let big = RgbaImage {
                    width: ld.left.width.max(ld.right.width),
                    height: ld.left.height.max(ld.right.height),
                    pixels: Vec::new(),
                };
                let d = dest_rect(s, &big, area);
                let divx = d.0 + (d.2 as f64 * s.slice as f64 / 100.0) as i32;
                let part = |img: &RgbaImage, dib: HBITMAP, clip_l: i32, clip_r: i32| {
                    if clip_r > clip_l {
                        let sv = SaveDC(hdc);
                        IntersectClipRect(hdc, clip_l, cvr.top, clip_r, cvr.bottom);
                        blit(hdc, dib, img.width as i32, img.height as i32, d, 255);
                        let _ = RestoreDC(hdc, sv);
                    }
                };
                part(&ld.left, ld.l_dib, d.0, divx);
                part(&ld.right, ld.r_dib, divx, d.0 + d.2);
                frame(hdc, d, t.border);
                fill(
                    hdc,
                    &RECT {
                        left: divx - 1,
                        top: d.1,
                        right: divx + 1,
                        bottom: d.1 + d.3,
                    },
                    t.accent,
                );
            }
        }
        let _ = RestoreDC(hdc, saved);
        SelectObject(hdc, old_font);
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    unsafe {
        if !hdc.is_invalid() {
            // 子コントロールの背面はダブルバッファで描く(ちらつき防止)
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let mem = CreateCompatibleDC(Some(hdc));
            let bmp = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
            let old = SelectObject(mem, HGDIOBJ(bmp.0));
            paint_into(hwnd, mem);
            let r = ps.rcPaint;
            let _ = BitBlt(
                hdc,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                Some(mem),
                r.left,
                r.top,
                SRCCOPY,
            );
            SelectObject(mem, old);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            let _ = DeleteDC(mem);
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
        let d_dib = make_dib(&cmp.diff_image);
        let s2 = st(hwnd);
        if let Some(l2) = s2.loaded.as_mut() {
            let _ = DeleteObject(HGDIOBJ(l2.d_dib.0));
            l2.diff = cmp.diff_image;
            l2.stats = cmp.stats;
            l2.d_dib = d_dib;
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
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
    super::guard::run("imagepane", hwnd, msg, wparam, lparam, true, || unsafe {
        wndproc_inner(hwnd, msg, wparam, lparam)
    })
}

unsafe fn wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
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
                        let _ = SetTimer(Some(hwnd), TIMER_BLINK, 650, None);
                    }
                }
                Err(e) => {
                    s.message = e;
                }
            }
            // 情報テーブルの高さが決まるので配置し直す
            layout(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
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
                        let _ = InvalidateRect(Some(hwnd), None, false);
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
                let _ = InvalidateRect(Some(hwnd), None, false);
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
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_PRINTCLIENT => {
            paint_into(hwnd, HDC(wparam.0 as *mut _));
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SETFONT => {
            if wparam.0 != 0 {
                st(hwnd).font = HFONT(wparam.0 as *mut _);
            }
            apply_font(hwnd);
            layout(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            match theme::ctl_color(msg, wparam, &theme::cached()) {
                Some(r) => r,
                None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }
        WM_MOUSEWHEEL => {
            // ホイールでズーム(フィット=100% が基準)
            let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
            let s = st(hwnd);
            let f = if delta > 0 { 1.1 } else { 1.0 / 1.1 };
            s.vp.scale = ImageViewport::clamped_scale(s.vp.scale * f);
            sync_zoom_ui(s);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
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
