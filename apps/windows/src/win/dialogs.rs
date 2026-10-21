//! 設定・コピー計画・3-way・ref入力・バージョン情報の各ウィンドウ。
//! すべてモードレス(親ウィンドウのハンドルと AppModel へのポインタを受け取る)。

use std::path::PathBuf;
use windows::core::PWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{SS_CENTERIMAGE, SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::appmodel::AppModel;
use crate::win::ext::{ExtContext, Win32EditionExtension};
use crate::win::font;
use crate::win::wide::wstr;
use std::sync::Arc;

// ---------- 共通ヘルパ ----------

pub fn hinst() -> HINSTANCE {
    unsafe { GetModuleHandleW(None) }.unwrap_or_default().into()
}

#[allow(clippy::too_many_arguments)]
fn mk_pcw(
    parent: HWND,
    class: windows::core::PCWSTR,
    text: &str,
    style: u32,
    ex: WINDOW_EX_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
) -> HWND {
    unsafe {
        CreateWindowExW(
            ex,
            class,
            windows::core::PCWSTR(wstr(text).as_ptr()),
            WINDOW_STYLE(style) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(hinst()),
            None,
        )
        .unwrap_or_default()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn mk(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    ex: WINDOW_EX_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
) -> HWND {
    let c = wstr(class);
    mk_pcw(
        parent,
        windows::core::PCWSTR(c.as_ptr()),
        text,
        style,
        ex,
        x,
        y,
        w,
        h,
        id,
    )
}

pub fn set_font(hwnd: HWND, font: HFONT) {
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

/// モードレスダイアログ共通: 背景・コントロール色をテーマに合わせる
fn dlg_common(hwnd: HWND, msg: u32, wparam: WPARAM) -> Option<LRESULT> {
    let t = crate::win::theme::cached();
    match msg {
        WM_ERASEBKGND => Some(crate::win::theme::erase_bg(hwnd, wparam, &t)),
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            crate::win::theme::ctl_color(msg, wparam, &t)
        }
        _ => None,
    }
}

/// 全子コントロールにフォントとテーマを適用(`skip` は適用しない子)
pub fn style_children(hwnd: HWND, font: HFONT, skip: HWND) {
    let dark = crate::win::theme::cached().dark;
    unsafe {
        let mut child = GetWindow(hwnd, GW_CHILD);
        while let Ok(c) = child {
            if c != skip {
                set_font(c, font);
                crate::win::theme::set_control_theme(c, dark);
            }
            child = GetWindow(c, GW_HWNDNEXT);
        }
    }
}

fn dpi_of(hwnd: HWND) -> u32 {
    unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96)
}

pub fn get_edit_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

pub fn checkbox(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(if checked { BST_CHECKED.0 as usize } else { 0 })),
            Some(LPARAM(0)),
        );
    }
}

pub fn is_checked(hwnd: HWND) -> bool {
    unsafe {
        SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))).0 as u32 == BST_CHECKED.0
    }
}

// ---------- 設定ウィンドウ ----------

struct SettingsWin {
    main: HWND,
    /// このウィンドウが所有するフォント(自身の DPI で生成し WM_DESTROY で解放)
    font: HFONT,
    model: *mut AppModel,
    /// スクロール対象の全コンテンツ高(物理 px)
    content_h: i32,
    scroll_y: i32,
    /// エディション拡張(追加設定セクション)
    ext: Option<Arc<dyn Win32EditionExtension>>,
    /// 拡張セクションのウィンドウ(`pane` の子)
    ext_hwnd: HWND,
    /// 全コンテンツを載せる 1 枚のコンテナ子ウィンドウ。スクロールはこれを動かす
    /// (テーマ付きボタン類が親の描画面を共有して残像を残すのを避ける)
    pane: HWND,
}

const ID_CHK_RECURSIVE: i32 = 201;
const ID_CHK_DEFEXCL: i32 = 202;
const ID_CHK_RENAMES: i32 = 203;
const ID_CHK_LE: i32 = 204;
const ID_CHK_TWS: i32 = 205;
const ID_CHK_NWS: i32 = 206;
const ID_CHK_CASE: i32 = 207;
const ID_CMB_STRUCT: i32 = 208;
const ID_ED_EXCL: i32 = 209;
const ID_ED_INCL: i32 = 210;
const ID_CHK_AUTOREF: i32 = 213;
const ID_PRESET_BASE: i32 = 220; // +i
const ID_BTN_CLOSE: i32 = 230;
/// 説明文(補足テキスト)の ID 起点。淡色で描くための目印
const ID_NOTE_BASE: i32 = 500;

const SET_CLASS: &str = "DiffLoupeSettings";
const SET_PANE_CLASS: &str = "DiffLoupeSettingsPane";

pub fn open_settings(
    main: HWND,
    model: *mut AppModel,
    ext: Option<Arc<dyn Win32EditionExtension>>,
) -> HWND {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let dpi = unsafe { GetDpiForWindow(main) } as i32;
    let sc = |v: i32| v * dpi / 96;
    let class = wstr(SET_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(settings_wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let pane_class = wstr(SET_PANE_CLASS);
        let pwc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(pane_class.as_ptr()),
            lpfnWndProc: Some(settings_pane_wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&pwc);
    }
    let st = Box::new(SettingsWin {
        main,
        font: HFONT::default(),
        model,
        content_h: 0,
        scroll_y: 0,
        ext,
        ext_hwnd: HWND::default(),
        pane: HWND::default(),
    });
    unsafe {
        // 実寸は WM_CREATE でコンテンツ高を測ってから確定する
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::SETTINGS_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VSCROLL | WS_CLIPCHILDREN | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            sc(580),
            sc(700),
            Some(main),
            None,
            Some(hinst()),
            Some(Box::into_raw(st) as *const _),
        )
        .unwrap_or_default()
    }
}

fn ctext(h: HWND, text: &str) {
    unsafe {
        let _ = SetWindowTextW(h, windows::core::PCWSTR(wstr(text).as_ptr()));
    }
}

/// 設定ウィンドウのコンテンツを縦に積む配置カーソル。
/// 説明文の高さは `DT_CALCRECT` で実測し、重なり・切れを避ける。
struct Flow {
    hwnd: HWND,
    font: HFONT,
    margin: i32,
    ip: i32,
    gw: i32,
    iw: i32,
    y: i32,
    row: i32,
    next_note: i32,
}

impl Flow {
    fn sc(&self, v: i32) -> i32 {
        crate::win::dpi::scale(self.hwnd, v)
    }

    fn begin_group(&mut self, title: &str) -> (HWND, i32) {
        let top = self.y;
        let g = mk(
            self.hwnd,
            "BUTTON",
            title,
            BS_GROUPBOX as u32,
            // 枠の内側は塗らないので、コンテナ(WS_CLIPCHILDREN)の背景消去から
            // 外されないようにする(外れると枠内に古い描画が残り残像になる)
            WS_EX_TRANSPARENT,
            self.margin,
            top,
            self.gw,
            self.sc(40),
            0,
        );
        self.y += font::font_height(self.font) + self.sc(8);
        (g, top)
    }

    fn end_group(&mut self, g: HWND, top: i32) {
        let h = self.y - top + self.sc(4);
        unsafe {
            let _ = MoveWindow(g, self.margin, top, self.gw, h, true);
        }
        self.y = top + h + self.sc(10);
    }

    fn check(&mut self, text: &str, id: i32, on: bool) -> HWND {
        let c = mk(
            self.hwnd,
            "BUTTON",
            text,
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0,
            WINDOW_EX_STYLE(0),
            self.margin + self.ip,
            self.y,
            self.iw,
            self.row,
            id,
        );
        checkbox(c, on);
        self.y += self.row;
        c
    }

    /// 折り返し説明文(淡色)。`indent` はチェックボックス本文に揃える字下げ
    fn note(&mut self, text: &str, indent: i32) {
        let w = self.iw - indent;
        let h = font::calc_text_height(self.font, text, w) + self.sc(2);
        mk(
            self.hwnd,
            "STATIC",
            text,
            SS_LEFT.0 | SS_NOPREFIX.0,
            WINDOW_EX_STYLE(0),
            self.margin + self.ip + indent,
            self.y,
            w,
            h,
            self.next_note,
        );
        self.next_note += 1;
        self.y += h + self.sc(6);
    }

    fn gap(&mut self, v: i32) {
        self.y += v;
    }

    fn pattern_box(&mut self, id: i32, text: &str) {
        let h = self.sc(84);
        let ed = mk(
            self.hwnd,
            "EDIT",
            "",
            (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            WS_EX_CLIENTEDGE,
            self.margin + self.ip,
            self.y,
            self.iw,
            h,
            id,
        );
        ctext(ed, text);
        self.y += h + self.sc(6);
    }
}

fn settings_create(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    let m = unsafe { &mut *s.model };
    let sc = |v: i32| crate::win::dpi::scale(hwnd, v);
    let th = font::font_height(s.font);
    let margin = sc(12);
    let ip = sc(14);
    let client_w = sc(560);
    let gw = client_w - 2 * margin;
    // コンテンツ全体を載せるコンテナ(位置と高さは settings_layout_pane で確定)
    let pane = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(wstr(SET_PANE_CLASS).as_ptr()),
            windows::core::PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            client_w,
            1,
            Some(hwnd),
            None,
            Some(hinst()),
            None,
        )
        .unwrap_or_default()
    };
    s.pane = pane;
    let mut f = Flow {
        hwnd: pane,
        font: s.font,
        margin,
        ip,
        gw,
        iw: gw - 2 * ip,
        y: margin,
        row: (th + sc(8)).max(sc(24)),
        next_note: ID_NOTE_BASE,
    };

    // エディション: 「現在のエディション: Free」+ 制限の説明(版名の重複表記はしない)
    let (g, top) = f.begin_group(crate::strings::SEC_EDITION);
    let name = m.edition.name();
    let head = format!("{}: {}", crate::strings::CURRENT_EDITION, name);
    let lbl = mk(
        pane,
        "STATIC",
        &head,
        SS_LEFT.0 | SS_NOPREFIX.0 | SS_CENTERIMAGE.0,
        WINDOW_EX_STYLE(0),
        margin + ip,
        f.y,
        f.iw,
        f.row,
        0,
    );
    let _ = lbl;
    f.y += f.row;
    if m.edition.max_folder_entries().is_some() {
        f.note(
            &format!("{}{}", name, crate::strings::EDITION_LIMIT_NOTE),
            0,
        );
    }
    f.end_group(g, top);

    // 比較
    let (g, top) = f.begin_group(crate::strings::SEC_COMPARE);
    f.check(
        crate::strings::OPT_RECURSIVE,
        ID_CHK_RECURSIVE,
        m.settings.recursive_comparison,
    );
    f.note(crate::strings::OPT_RECURSIVE_NOTE, sc(22));
    f.check(
        crate::strings::OPT_DEFAULT_EXCLUDES,
        ID_CHK_DEFEXCL,
        m.settings.use_default_excludes,
    );
    f.gap(sc(6));
    f.check(
        crate::strings::OPT_RENAMES,
        ID_CHK_RENAMES,
        m.settings.detect_renames,
    );
    f.note(crate::strings::OPT_RENAMES_NOTE, sc(22));
    f.end_group(g, top);

    // 内容比較の正規化
    let (g, top) = f.begin_group(crate::strings::SEC_NORMALIZE);
    for (id, text, on) in [
        (
            ID_CHK_LE,
            crate::strings::OPT_IGNORE_LINE_ENDINGS,
            m.settings.content_ignore_line_endings,
        ),
        (
            ID_CHK_TWS,
            crate::strings::OPT_IGNORE_TRAILING_WS,
            m.settings.content_ignore_trailing_whitespace,
        ),
        (
            ID_CHK_NWS,
            crate::strings::OPT_NORMALIZE_WS,
            m.settings.content_normalize_whitespace,
        ),
        (
            ID_CHK_CASE,
            crate::strings::OPT_IGNORE_CASE,
            m.settings.content_ignore_case,
        ),
    ] {
        f.check(text, id, on);
    }
    f.note(crate::strings::NORMALIZE_NOTE, 0);
    // ラベル + コンボ(ラベルは行の縦中央に揃える)
    let label_w = font::measure(s.font, crate::strings::OPT_STRUCTURED).cx + sc(12);
    mk(
        pane,
        "STATIC",
        crate::strings::OPT_STRUCTURED,
        SS_LEFT.0 | SS_NOPREFIX.0 | SS_CENTERIMAGE.0,
        WINDOW_EX_STYLE(0),
        margin + ip,
        f.y,
        label_w,
        f.row,
        0,
    );
    let cmb = mk(
        pane,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
        WINDOW_EX_STYLE(0),
        margin + ip + label_w,
        f.y,
        sc(240),
        f.row + sc(200),
        ID_CMB_STRUCT,
    );
    unsafe {
        SendMessageW(
            cmb,
            CB_SETITEMHEIGHT,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM((f.row - sc(6)) as isize)),
        );
    }
    f.y += f.row + sc(4);
    let cur = m.settings.content_structured_mode.clone();
    for (_m, label) in crate::settings::AppSettings::structured_modes().iter() {
        let t = wstr(label);
        unsafe {
            SendMessageW(
                cmb,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(t.as_ptr() as isize)),
            );
        }
    }
    let sel = crate::settings::AppSettings::structured_modes()
        .iter()
        .position(|(mm, _)| mm.raw_value() == cur)
        .unwrap_or(0);
    unsafe {
        SendMessageW(cmb, CB_SETCURSEL, Some(WPARAM(sel)), Some(LPARAM(0)));
    }
    f.note(crate::strings::STRUCTURED_NOTE, 0);
    f.end_group(g, top);

    // 除外パターン
    let (g, top) = f.begin_group(crate::strings::SEC_EXCLUDE);
    f.pattern_box(ID_ED_EXCL, &m.settings.user_exclude_patterns);
    f.note(crate::strings::PATTERN_HINT, 0);
    f.end_group(g, top);

    // インクルード
    let (g, top) = f.begin_group(crate::strings::SEC_INCLUDE);
    f.pattern_box(ID_ED_INCL, &m.settings.user_include_patterns);
    f.note(crate::strings::INCLUDE_NOTE, 0);
    f.end_group(g, top);

    // 自動再比較
    let (g, top) = f.begin_group(crate::strings::SEC_AUTO_REFRESH);
    f.check(
        crate::strings::OPT_AUTO_REFRESH,
        ID_CHK_AUTOREF,
        m.settings.auto_refresh_enabled,
    );
    f.note(crate::strings::AUTO_REFRESH_NOTE, sc(22));
    f.end_group(g, top);

    // プリセット
    let (g, top) = f.begin_group(crate::strings::SEC_PRESETS);
    let sel_ids: Vec<String> = m.settings.selected_preset_ids();
    for (i, p) in diffloupe_core::FILTER_PRESETS.iter().enumerate() {
        f.check(
            p.display_name,
            ID_PRESET_BASE + i as i32,
            sel_ids.iter().any(|id| id == p.id),
        );
    }
    f.note(crate::strings::PRESETS_NOTE, 0);
    f.end_group(g, top);

    // エディション拡張セクション(Pro のレポートテンプレートなど)
    if m.edition
        .provides(diffloupe_edition::ExtensionPoint::SettingsSection)
    {
        if let Some(ext) = s.ext.clone() {
            let ctx = ExtContext {
                parent: pane,
                main: s.main,
                model: m,
                font: s.font,
                dpi: dpi_of(hwnd),
            };
            if let Some(v) = ext.settings_section(&ctx) {
                let h = f.sc(v.height.max(1));
                s.ext_hwnd = v.hwnd;
                unsafe {
                    let _ = MoveWindow(v.hwnd, f.margin, f.y, f.gw, h, true);
                }
                f.y += h + sc(10);
            }
        }
    }

    // 閉じる(右寄せ)
    let bw = (font::measure(s.font, crate::strings::CLOSE).cx + sc(40)).max(sc(96));
    mk(
        pane,
        "BUTTON",
        crate::strings::CLOSE,
        (BS_PUSHBUTTON | BS_DEFPUSHBUTTON) as u32 | WS_TABSTOP.0,
        WINDOW_EX_STYLE(0),
        margin + gw - bw,
        f.y,
        bw,
        f.row,
        ID_BTN_CLOSE,
    );
    f.y += f.row + margin;
    s.content_h = f.y;
}

/// 全子コントロールへフォントとテーマを適用する
/// 子をすべて作り直す(DPI・テーマの変更時)。フォントは `dpi` で作り直す
fn settings_rebuild(hwnd: HWND, dpi: u32) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    unsafe {
        while let Ok(c) = GetWindow(hwnd, GW_CHILD) {
            let _ = DestroyWindow(c);
        }
        let old = s.font;
        s.font = font::make_ui_font(dpi);
        let _ = DeleteObject(HGDIOBJ(old.0));
    }
    s.scroll_y = 0;
    s.ext_hwnd = HWND::default();
    s.pane = HWND::default();
    settings_create(hwnd);
    settings_style_children(hwnd, s.font);
    settings_fit_window(hwnd);
}

fn settings_style_children(hwnd: HWND, font: HFONT) {
    let dark = crate::win::theme::cached().dark;
    unsafe {
        let s = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsWin);
        let mut child = GetWindow(s.pane, GW_CHILD);
        while let Ok(c) = child {
            set_font(c, font);
            crate::win::theme::set_control_theme(c, dark);
            child = GetWindow(c, GW_HWNDNEXT);
        }
    }
}

/// コンテンツ高に合わせてクライアント高を決める(作業領域を超える分はスクロール)
fn settings_fit_window(hwnd: HWND) {
    use windows::Win32::UI::HiDpi::{
        AdjustWindowRectExForDpi, GetDpiForWindow, GetSystemMetricsForDpi,
    };
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let client_w = crate::win::dpi::scale(hwnd, 560);
    unsafe {
        let mon = MonitorFromWindow(s.main, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let work = if GetMonitorInfoW(mon, &mut mi).as_bool() {
            mi.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            }
        };
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VSCROLL;
        // 縦スクロールバー分の幅も見込んで外寸を求める
        let mut r = RECT {
            left: 0,
            top: 0,
            right: client_w,
            bottom: s.content_h,
        };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE(0), dpi);
        let frame_h = (r.bottom - r.top) - s.content_h;
        let max_outer_h = ((work.bottom - work.top) as f64 * 0.92) as i32;
        let outer_h = (s.content_h + frame_h).min(max_outer_h);
        let scrollbar = GetSystemMetricsForDpi(SM_CXVSCROLL, dpi);
        let outer_w = (r.right - r.left) + scrollbar;
        // 親ウィンドウの中央へ(作業領域内に収める)
        let mut pr = RECT::default();
        let _ = GetWindowRect(s.main, &mut pr);
        let x = ((pr.left + pr.right - outer_w) / 2)
            .clamp(work.left, (work.right - outer_w).max(work.left));
        let y = ((pr.top + pr.bottom - outer_h) / 2)
            .clamp(work.top, (work.bottom - outer_h).max(work.top));
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            outer_w,
            outer_h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    settings_update_scroll(hwnd);
}

fn settings_update_scroll(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let page = rc.bottom.max(1);
    let max = (s.content_h - page).max(0);
    s.scroll_y = s.scroll_y.clamp(0, max);
    let si = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS | SIF_DISABLENOSCROLL,
        nMin: 0,
        nMax: (s.content_h - 1).max(0),
        nPage: page as u32,
        nPos: s.scroll_y,
        ..Default::default()
    };
    unsafe {
        let _ = SetScrollInfo(hwnd, SB_VERT, &si, true);
    }
    settings_layout_pane(hwnd, false);
}

/// コンテナを現在のスクロール位置に置く。`full_repaint` なら内容を複写せず全面再描画させる
fn settings_layout_pane(hwnd: HWND, full_repaint: bool) {
    let s = unsafe { &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsWin) };
    if s.pane.is_invalid() {
        return;
    }
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
        let mut flags = SWP_NOZORDER | SWP_NOACTIVATE;
        if full_repaint {
            flags |= SWP_NOCOPYBITS;
        }
        let _ = SetWindowPos(
            s.pane,
            None,
            0,
            -s.scroll_y,
            rc.right.max(1),
            s.content_h.max(1),
            flags,
        );
        if full_repaint {
            let _ = RedrawWindow(
                Some(s.pane),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }
}

/// 縦スクロール位置を `target` へ移し、子コントロールごと表示をずらす
fn settings_scroll_to(hwnd: HWND, target: i32) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let max = (s.content_h - rc.bottom).max(0);
    let target = target.clamp(0, max);
    if s.scroll_y != target {
        s.scroll_y = target;
        // コンテナごと動かして全面再描画する(子はコンテナにクリップされ残像が出ない)
        settings_layout_pane(hwnd, true);
    }
    settings_update_scroll(hwnd);
}

/// 設定ウィンドウのコンテナ。子コントロールの通知・色要求は設定ウィンドウへ転送する
unsafe extern "system" fn settings_pane_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => {
                crate::win::theme::erase_bg(hwnd, wparam, &crate::win::theme::cached())
            }
            WM_COMMAND | WM_NOTIFY | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT
            | WM_CTLCOLORLISTBOX => match GetParent(hwnd) {
                Ok(p) => SendMessageW(p, msg, Some(wparam), Some(lparam)),
                Err(_) => DefWindowProcW(hwnd, msg, wparam, lparam),
            },
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    super::guard::run("settings", hwnd, msg, wparam, lparam, true, || unsafe {
        settings_wndproc_inner(hwnd, msg, wparam, lparam)
    })
}

unsafe fn settings_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
            let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
            s.font = font::make_ui_font(dpi);
            settings_create(hwnd);
            settings_style_children(hwnd, s.font);
            crate::win::theme::apply_title_bar(hwnd, crate::win::theme::cached().dark);
            settings_fit_window(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let st = Box::from_raw(ptr as *mut SettingsWin);
                    let _ = DeleteObject(HGDIOBJ(st.font.0));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // 新 DPI: 位置を提案矩形へ移し、フォントを作り直してコンテンツを組み直す
            let new_dpi = ((wparam.0 >> 16) & 0xFFFF).max(96) as u32;
            unsafe {
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            settings_rebuild(hwnd, new_dpi);
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            // ライト/ダーク切替(ImmersiveColorSet)。視覚スタイルの付け外しは作成時に決まる
            // コントロールがあるので、拡張セクションも含めて組み直す
            let is_color = lparam.0 != 0
                && unsafe {
                    windows::core::PCWSTR(lparam.0 as *const u16)
                        .to_string()
                        .map(|v| v == "ImmersiveColorSet")
                        .unwrap_or(false)
                };
            if is_color {
                crate::win::theme::refresh();
                crate::win::theme::apply_title_bar(hwnd, crate::win::theme::cached().dark);
                let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
                settings_rebuild(hwnd, dpi);
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_ERASEBKGND => crate::win::theme::erase_bg(hwnd, wparam, &crate::win::theme::cached()),
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let t = crate::win::theme::cached();
            let r = crate::win::theme::ctl_color(msg, wparam, &t);
            // 補足説明は淡色で描く
            if msg == WM_CTLCOLORSTATIC {
                let cid = unsafe { GetDlgCtrlID(HWND(lparam.0 as *mut _)) };
                if cid >= ID_NOTE_BASE {
                    unsafe {
                        SetTextColor(HDC(wparam.0 as *mut _), t.dim_text);
                    }
                }
            }
            match r {
                Some(r) => r,
                None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }
        WM_SIZE => {
            settings_update_scroll(hwnd);
            LRESULT(0)
        }
        WM_VSCROLL => {
            let s = unsafe { &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsWin) };
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let step = crate::win::dpi::scale(hwnd, 32);
            let code = (wparam.0 & 0xFFFF) as u16;
            let target = match code {
                x if x == SB_LINEUP.0 as u16 => s.scroll_y - step,
                x if x == SB_LINEDOWN.0 as u16 => s.scroll_y + step,
                x if x == SB_PAGEUP.0 as u16 => s.scroll_y - rc.bottom,
                x if x == SB_PAGEDOWN.0 as u16 => s.scroll_y + rc.bottom,
                x if x == SB_TOP.0 as u16 => 0,
                x if x == SB_BOTTOM.0 as u16 => s.content_h,
                x if x == SB_THUMBTRACK.0 as u16 || x == SB_THUMBPOSITION.0 as u16 => {
                    let mut si = SCROLLINFO {
                        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                        fMask: SIF_TRACKPOS,
                        ..Default::default()
                    };
                    unsafe {
                        let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);
                    }
                    si.nTrackPos
                }
                _ => s.scroll_y,
            };
            settings_scroll_to(hwnd, target);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let s = unsafe { &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsWin) };
            let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
            let step = crate::win::dpi::scale(hwnd, 48);
            settings_scroll_to(hwnd, s.scroll_y - delta / WHEEL_DELTA as i32 * step);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
            let m = unsafe { &mut *s.model };
            let pane = s.pane;
            let get = |id: i32| unsafe { GetDlgItem(Some(pane), id).unwrap_or_default() };
            match id {
                ID_CHK_RECURSIVE => m.settings.recursive_comparison = is_checked(get(id)),
                ID_CHK_DEFEXCL => m.settings.use_default_excludes = is_checked(get(id)),
                ID_CHK_RENAMES => m.settings.detect_renames = is_checked(get(id)),
                ID_CHK_LE => m.settings.content_ignore_line_endings = is_checked(get(id)),
                ID_CHK_TWS => m.settings.content_ignore_trailing_whitespace = is_checked(get(id)),
                ID_CHK_NWS => m.settings.content_normalize_whitespace = is_checked(get(id)),
                ID_CHK_CASE => m.settings.content_ignore_case = is_checked(get(id)),
                ID_CHK_AUTOREF => {
                    let on = is_checked(get(id));
                    m.settings.auto_refresh_enabled = on;
                    if on {
                        m.auto_refresh_run_count = 0;
                        m.auto_refresh_paused_reason = None;
                    }
                    // メインウィンドウへ watcher 再構成を通知
                    unsafe {
                        let _ = PostMessageW(
                            Some(s.main),
                            super::app::WM_APP_SETTINGS_CHANGED,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
                ID_CMB_STRUCT => {
                    if code == CBN_SELCHANGE as u16 {
                        let sel = unsafe {
                            SendMessageW(get(id), CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)))
                        }
                        .0 as usize;
                        if let Some((mode, _)) =
                            crate::settings::AppSettings::structured_modes().get(sel)
                        {
                            m.settings.content_structured_mode = mode.raw_value().into();
                        }
                    }
                }
                ID_ED_EXCL | ID_ED_INCL => {
                    if code == EN_KILLFOCUS as u16 || code == EN_CHANGE as u16 {
                        let txt = get_edit_text(get(id));
                        if id == ID_ED_EXCL {
                            m.settings.user_exclude_patterns = txt;
                        } else {
                            m.settings.user_include_patterns = txt;
                        }
                    }
                }
                i if (ID_PRESET_BASE..ID_PRESET_BASE + 32).contains(&i) => {
                    let idx = (i - ID_PRESET_BASE) as usize;
                    if let Some(p) = diffloupe_core::FILTER_PRESETS.get(idx) {
                        let on = is_checked(get(i));
                        let mut ids: Vec<String> = m
                            .settings
                            .filter_presets
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if on && !ids.iter().any(|x| x == p.id) {
                            ids.push(p.id.to_string());
                        } else if !on {
                            ids.retain(|x| x != p.id);
                        }
                        m.settings.filter_presets = ids.join(",");
                    }
                }
                ID_BTN_CLOSE => {
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return LRESULT(0);
                }
                _ => {}
            }
            m.save_settings();
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ---------- コピー計画ウィンドウ ----------

const SYNC_CLASS: &str = "DiffLoupeSyncPlan";
const ID_CMB_DIR: i32 = 301;
const ID_LIST: i32 = 302;
const ID_SUMMARY: i32 = 303;

struct SyncWin {
    _main: HWND,
    font: HFONT,
    model: *mut AppModel,
    h_lbl: HWND,
    h_dir: HWND,
    h_list: HWND,
    h_sum: HWND,
    /// 拡張の操作領域(同期実行など)。Free は不正値で帯を取らない
    ext_area: HWND,
    /// 操作領域の高さ(物理 px)
    ext_area_h: i32,
    ext: Option<Arc<dyn Win32EditionExtension>>,
}

/// `model` は呼び出し側が所有する AppModel へのポインタで、ウィンドウ存続中は
/// 有効である必要がある。
///
/// # Safety
/// `model` は `AppModel` への有効なポインタであり、生成したウィンドウが
/// 破棄されるまで参照先が生存していること。
pub unsafe fn open_sync_plan(
    main: HWND,
    model: *mut AppModel,
    direction: diffloupe_core::sync_plan::SyncDirection,
    ext: Option<Arc<dyn Win32EditionExtension>>,
) -> HWND {
    let class = wstr(SYNC_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(sync_wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let m = unsafe { &mut *model };
    m.prepare_sync_plan(direction);
    let st = Box::new(SyncWin {
        _main: main,
        font: HFONT::default(),
        model,
        h_lbl: HWND::default(),
        h_dir: HWND::default(),
        h_list: HWND::default(),
        h_sum: HWND::default(),
        ext_area: HWND::default(),
        ext_area_h: 0,
        ext,
    });
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::SYNC_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            crate::win::dpi::scale(main, 860),
            crate::win::dpi::scale(main, 560),
            Some(main),
            None,
            Some(hinst()),
            Some(Box::into_raw(st) as *const _),
        )
        .unwrap_or_default()
    }
}

fn sync_fill(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
    let m = unsafe { &*s.model };
    let Some(plan) = &m.sync_plan else { return };
    unsafe {
        SendMessageW(
            s.h_list,
            LVM_DELETEALLITEMS,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
        for (i, item) in plan.items.iter().enumerate() {
            let mut li = LVITEMW {
                mask: LVIF_TEXT,
                iItem: i as i32,
                iSubItem: 0,
                ..Default::default()
            };
            let action = match item.action {
                diffloupe_core::sync_plan::SyncAction::CopyFile => crate::strings::SYNC_COPY,
                diffloupe_core::sync_plan::SyncAction::CreateDirectory => {
                    crate::strings::SYNC_MKDIR
                }
                diffloupe_core::sync_plan::SyncAction::Conflict => crate::strings::SYNC_CONFLICT,
                diffloupe_core::sync_plan::SyncAction::PreservedDestination => {
                    crate::strings::SYNC_PRESERVED
                }
                diffloupe_core::sync_plan::SyncAction::Blocked => crate::strings::SYNC_BLOCKED,
            };
            let cols = [
                action.to_string(),
                item.relative_path.clone(),
                item.source.clone().unwrap_or_else(|| "—".into()),
                if item.bytes > 0 {
                    crate::viewmodel::byte_count_text(item.bytes)
                } else {
                    "—".into()
                },
                item.reason.clone(),
            ];
            let t0 = wstr(&cols[0]);
            li.pszText = PWSTR(t0.as_ptr() as *mut u16);
            SendMessageW(
                s.h_list,
                LVM_INSERTITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&li as *const _ as isize)),
            );
            for (sub, text) in cols.iter().enumerate().skip(1) {
                let t = wstr(text);
                let si = LVITEMW {
                    mask: LVIF_TEXT,
                    iItem: i as i32,
                    iSubItem: sub as i32,
                    pszText: PWSTR(t.as_ptr() as *mut u16),
                    ..Default::default()
                };
                SendMessageW(
                    s.h_list,
                    LVM_SETITEMTEXTW,
                    Some(WPARAM(i)),
                    Some(LPARAM(&si as *const _ as isize)),
                );
            }
        }
        let sum = plan.summary.clone();
        let text = format!(
            "{}: {}  {}: {}  {}: {}  {}: {}  {}: {}    {}",
            crate::strings::SYNC_SUMMARY_COPY,
            sum.copy_files,
            crate::strings::SYNC_SUMMARY_DIR,
            sum.create_directories,
            crate::strings::SYNC_SUMMARY_CONFLICT,
            sum.conflicts,
            crate::strings::SYNC_SUMMARY_KEEP,
            sum.preserved_destinations,
            crate::strings::SYNC_SUMMARY_ERROR,
            sum.blocked,
            crate::viewmodel::byte_count_text(sum.total_bytes),
        );
        let _ = SetWindowTextW(s.h_sum, windows::core::PCWSTR(wstr(&text).as_ptr()));
    }
}

unsafe extern "system" fn sync_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    super::guard::run("sync", hwnd, msg, wparam, lparam, true, || unsafe {
        sync_wndproc_inner(hwnd, msg, wparam, lparam)
    })
}

/// 拡張の操作領域を生成/再生成する(計画の方向変更時は作り直し)。戻り値は高さ(物理 px)。
fn sync_make_ext_area(hwnd: HWND) -> i32 {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
    if !s.ext_area.is_invalid() {
        unsafe {
            let _ = DestroyWindow(s.ext_area);
        }
        s.ext_area = HWND::default();
    }
    let m = unsafe { &*s.model };
    if !m
        .edition
        .provides(diffloupe_edition::ExtensionPoint::SyncPlanActions)
    {
        return 0;
    }
    let Some(ext) = s.ext.clone() else { return 0 };
    let ctx = ExtContext {
        parent: hwnd,
        main: s._main,
        model: m,
        font: s.font,
        dpi: dpi_of(hwnd),
    };
    let Some(v) = ext.sync_plan_actions(&ctx) else {
        return 0;
    };
    s.ext_area = v.hwnd;
    s.ext_area_h = crate::win::dpi::scale(hwnd, v.height.max(150));
    s.ext_area_h
}

fn sync_layout(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
    if s.font.is_invalid() {
        return;
    }
    let sc = |v: i32| crate::win::dpi::scale(hwnd, v);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let (pad, gap) = (sc(12), sc(8));
    let row = (font::font_height(s.font) + sc(10)).max(sc(24));
    // 拡張の操作領域はシート下端の帯として確保する
    let ext_h = if s.ext_area.is_invalid() {
        0
    } else {
        s.ext_area_h
    };
    let lbl_w = font::measure(s.font, crate::strings::SYNC_DIRECTION).cx + sc(8);
    unsafe {
        let _ = MoveWindow(s.h_lbl, pad, pad, lbl_w, row, true);
        let _ = MoveWindow(
            s.h_dir,
            pad + lbl_w + gap,
            pad,
            sc(180),
            row + sc(120),
            true,
        );
        let y = pad + row + gap;
        let _ = MoveWindow(s.h_sum, pad, y, (rc.right - pad * 2).max(0), row, true);
        let ly = y + row + gap;
        let list_h = (rc.bottom - ext_h - ly - pad).max(sc(60));
        let _ = MoveWindow(s.h_list, pad, ly, (rc.right - pad * 2).max(0), list_h, true);
        if !s.ext_area.is_invalid() {
            let _ = MoveWindow(s.ext_area, 0, rc.bottom - ext_h, rc.right, ext_h, true);
        }
    }
}

unsafe fn sync_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(r) = dlg_common(hwnd, msg, wparam) {
        return r;
    }
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
            let m = unsafe { &*s.model };
            let sc = |v: i32| crate::win::dpi::scale(hwnd, v);
            s.font = font::make_ui_font(dpi_of(hwnd));
            s.h_lbl = mk(
                hwnd,
                "STATIC",
                crate::strings::SYNC_DIRECTION,
                SS_LEFT.0 | SS_CENTERIMAGE.0 | SS_NOPREFIX.0,
                WINDOW_EX_STYLE(0),
                0,
                0,
                10,
                10,
                0,
            );
            s.h_dir = mk(
                hwnd,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32 | WS_TABSTOP.0,
                WINDOW_EX_STYLE(0),
                0,
                0,
                10,
                10,
                ID_CMB_DIR,
            );
            for t in [
                crate::strings::SYNC_LEFT_TO_RIGHT,
                crate::strings::SYNC_RIGHT_TO_LEFT,
            ] {
                let w = wstr(t);
                unsafe {
                    SendMessageW(
                        s.h_dir,
                        CB_ADDSTRING,
                        Some(WPARAM(0)),
                        Some(LPARAM(w.as_ptr() as isize)),
                    );
                }
            }
            let cur = match m.sync_plan.as_ref().map(|p| p.direction) {
                Some(diffloupe_core::sync_plan::SyncDirection::RightToLeft) => 1,
                _ => 0,
            };
            unsafe {
                SendMessageW(s.h_dir, CB_SETCURSEL, Some(WPARAM(cur)), Some(LPARAM(0)));
            }
            s.h_sum = mk(
                hwnd,
                "STATIC",
                "",
                SS_LEFT.0 | SS_CENTERIMAGE.0 | SS_NOPREFIX.0,
                WINDOW_EX_STYLE(0),
                0,
                0,
                10,
                10,
                ID_SUMMARY,
            );
            s.h_list = mk_pcw(
                hwnd,
                WC_LISTVIEW,
                "",
                LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | WS_TABSTOP.0,
                WS_EX_CLIENTEDGE,
                0,
                0,
                10,
                10,
                ID_LIST,
            );
            unsafe {
                SendMessageW(
                    s.h_list,
                    LVM_SETEXTENDEDLISTVIEWSTYLE,
                    Some(WPARAM(0)),
                    Some(LPARAM(
                        (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
                    )),
                );
            }
            let headers = [
                crate::strings::SYNC_COL_ACTION,
                crate::strings::SYNC_COL_PATH,
                crate::strings::SYNC_COL_SOURCE,
                crate::strings::SYNC_COL_SIZE,
                crate::strings::SYNC_COL_REASON,
            ];
            let widths = [150, 260, 220, 90, 200];
            for (i, (h, w)) in headers.iter().zip(widths).enumerate() {
                let t = wstr(h);
                let col = LVCOLUMNW {
                    mask: LVCF_TEXT | LVCF_WIDTH,
                    cx: sc(w),
                    pszText: PWSTR(t.as_ptr() as *mut u16),
                    ..Default::default()
                };
                unsafe {
                    SendMessageW(
                        s.h_list,
                        LVM_INSERTCOLUMNW,
                        Some(WPARAM(i)),
                        Some(LPARAM(&col as *const _ as isize)),
                    );
                }
            }
            style_children(hwnd, s.font, HWND::default());
            unsafe {
                SendMessageW(
                    s.h_dir,
                    CB_SETITEMHEIGHT,
                    Some(WPARAM(usize::MAX)),
                    Some(LPARAM(
                        ((font::font_height(s.font) + sc(10)).max(sc(24)) - sc(6)) as isize,
                    )),
                );
            }
            crate::win::theme::apply_title_bar(hwnd, crate::win::theme::cached().dark);
            let _ = sync_make_ext_area(hwnd);
            sync_layout(hwnd);
            sync_fill(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let st = Box::from_raw(ptr as *mut SyncWin);
                    let _ = DeleteObject(HGDIOBJ(st.font.0));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_SIZE => {
            sync_layout(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            if id == ID_CMB_DIR && code == CBN_SELCHANGE as u16 {
                let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
                let sel = unsafe {
                    SendMessageW(s.h_dir, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)))
                }
                .0 as usize;
                let m = unsafe { &mut *s.model };
                m.prepare_sync_plan(if sel == 0 {
                    diffloupe_core::sync_plan::SyncDirection::LeftToRight
                } else {
                    diffloupe_core::sync_plan::SyncDirection::RightToLeft
                });
                // 計画が変わるので操作領域も作り直す
                let _ = sync_make_ext_area(hwnd);
                sync_layout(hwnd);
                sync_fill(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => unsafe {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        },
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ---------- 3-way ウィンドウ ----------

const TW_CLASS: &str = "DiffLoupeThreeWay";
const ID_BTN_BASE: i32 = 401;
const ID_BTN_LEFT: i32 = 402;
const ID_BTN_RIGHT: i32 = 403;
const ID_ED_BASE: i32 = 404;
const ID_ED_LEFT: i32 = 405;
const ID_ED_RIGHT: i32 = 406;
const ID_BTN_RUN: i32 = 407;
const ID_TW_GRID: i32 = 408;
const ID_TW_SUM: i32 = 409;

struct ThreeWin {
    _main: HWND,
    font: HFONT,
    mono: HFONT,
    h_run: HWND,
    model: *mut AppModel,
    h_base: HWND,
    h_left: HWND,
    h_right: HWND,
    h_eb: HWND,
    h_el: HWND,
    h_er: HWND,
    h_grid: HWND,
    h_sum: HWND,
    /// エディション拡張の 3-way ビュー(Pro のマージ resolver)。使うときは従来子を作らない
    ext_hwnd: HWND,
    ext: Option<Arc<dyn Win32EditionExtension>>,
}

pub fn open_threeway(
    main: HWND,
    model: *mut AppModel,
    ext: Option<Arc<dyn Win32EditionExtension>>,
) -> HWND {
    let class = wstr(TW_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(tw_wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let st = Box::new(ThreeWin {
        _main: main,
        font: HFONT::default(),
        mono: HFONT::default(),
        h_run: HWND::default(),
        model,
        h_base: HWND::default(),
        h_left: HWND::default(),
        h_right: HWND::default(),
        h_eb: HWND::default(),
        h_el: HWND::default(),
        h_er: HWND::default(),
        h_grid: HWND::default(),
        h_sum: HWND::default(),
        ext_hwnd: HWND::default(),
        ext,
    });
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::THREE_WAY_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            crate::win::dpi::scale(main, 1040),
            crate::win::dpi::scale(main, 680),
            Some(main),
            None,
            Some(hinst()),
            Some(Box::into_raw(st) as *const _),
        )
        .unwrap_or_default()
    }
}

fn tw_layout(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ThreeWin) };
    if s.font.is_invalid() {
        return;
    }
    let sc = |v: i32| crate::win::dpi::scale(hwnd, v);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let (pad, gap) = (sc(12), sc(8));
    let row = (font::font_height(s.font) + sc(10)).max(sc(24));
    if !s.ext_hwnd.is_invalid() {
        unsafe {
            let _ = MoveWindow(s.ext_hwnd, 0, 0, rc.right, rc.bottom, true);
        }
        return;
    }
    let btn_w = [
        crate::strings::BASE_FILE,
        crate::strings::LEFT_FILE,
        crate::strings::RIGHT_FILE,
    ]
    .iter()
    .map(|t| font::measure(s.font, t).cx)
    .max()
    .unwrap_or(0)
        + sc(28);
    let run_w = (font::measure(s.font, crate::strings::COMPARE_THREE).cx + sc(40)).max(sc(96));
    let path_x = pad + btn_w + gap;
    let full_w = (rc.right - path_x - pad).max(0);
    unsafe {
        let mut y = pad;
        for (b, e) in [(s.h_base, s.h_eb), (s.h_left, s.h_el), (s.h_right, s.h_er)] {
            let w = if e == s.h_er {
                (full_w - run_w - gap).max(0)
            } else {
                full_w
            };
            let _ = MoveWindow(b, pad, y, btn_w, row, true);
            let _ = MoveWindow(e, path_x, y, w, row, true);
            y += row + gap;
        }
        // 比較ボタンは 3 行目の右端
        let ry = pad + (row + gap) * 2;
        let _ = MoveWindow(s.h_run, rc.right - pad - run_w, ry, run_w, row, true);
        let _ = MoveWindow(s.h_sum, pad, y, (rc.right - pad * 2).max(0), row, true);
        y += row + gap;
        let _ = MoveWindow(s.h_grid, 0, y, rc.right, (rc.bottom - y).max(0), true);
    }
}

unsafe extern "system" fn tw_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    super::guard::run("threeway", hwnd, msg, wparam, lparam, true, || unsafe {
        tw_wndproc_inner(hwnd, msg, wparam, lparam)
    })
}

unsafe fn tw_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(r) = dlg_common(hwnd, msg, wparam) {
        return r;
    }
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ThreeWin) };
            let dpi = dpi_of(hwnd);
            s.font = font::make_ui_font(dpi);
            s.mono = font::make_mono_font(dpi);
            // エディションが 3-way ビューを供給するならコンテンツ全体を拡張に任せる
            let mut used_ext = false;
            {
                let m = unsafe { &*s.model };
                if m.edition
                    .provides(diffloupe_edition::ExtensionPoint::ThreeWayView)
                {
                    if let Some(ext) = s.ext.clone() {
                        let ctx = ExtContext {
                            parent: hwnd,
                            main: s._main,
                            model: m,
                            font: s.font,
                            dpi,
                        };
                        if let Some(v) = ext.three_way_view(&ctx) {
                            s.ext_hwnd = v.hwnd;
                            used_ext = true;
                            unsafe {
                                let _ = SetWindowTextW(
                                    hwnd,
                                    windows::core::PCWSTR(
                                        wstr(crate::strings::THREE_WAY_MERGE_TITLE).as_ptr(),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            if !used_ext {
                tw_create_std(hwnd);
            }
            crate::win::theme::apply_title_bar(hwnd, crate::win::theme::cached().dark);
            tw_layout(hwnd);
            LRESULT(0)
        }
        _ => tw_wndproc_rest(hwnd, msg, wparam, lparam),
    }
}

/// 従来(Free)の 3-way 子コントロール群を作る
fn tw_create_std(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ThreeWin) };
    let m = unsafe { &*s.model };
    s.h_base = mk(
        hwnd,
        "BUTTON",
        crate::strings::BASE_FILE,
        BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
        WINDOW_EX_STYLE(0),
        0,
        0,
        0,
        0,
        ID_BTN_BASE,
    );
    s.h_left = mk(
        hwnd,
        "BUTTON",
        crate::strings::LEFT_FILE,
        BS_PUSHBUTTON as u32,
        WINDOW_EX_STYLE(0),
        0,
        0,
        0,
        0,
        ID_BTN_LEFT,
    );
    s.h_right = mk(
        hwnd,
        "BUTTON",
        crate::strings::RIGHT_FILE,
        BS_PUSHBUTTON as u32,
        WINDOW_EX_STYLE(0),
        0,
        0,
        0,
        0,
        ID_BTN_RIGHT,
    );
    s.h_eb = mk(
        hwnd,
        "EDIT",
        "",
        ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP.0,
        WS_EX_CLIENTEDGE,
        0,
        0,
        0,
        0,
        ID_ED_BASE,
    );
    s.h_el = mk(
        hwnd,
        "EDIT",
        "",
        ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP.0,
        WS_EX_CLIENTEDGE,
        0,
        0,
        0,
        0,
        ID_ED_LEFT,
    );
    s.h_er = mk(
        hwnd,
        "EDIT",
        "",
        ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP.0,
        WS_EX_CLIENTEDGE,
        0,
        0,
        0,
        0,
        ID_ED_RIGHT,
    );
    s.h_run = mk(
        hwnd,
        "BUTTON",
        crate::strings::COMPARE_THREE,
        (BS_PUSHBUTTON | BS_DEFPUSHBUTTON) as u32,
        WINDOW_EX_STYLE(0),
        0,
        0,
        0,
        0,
        ID_BTN_RUN,
    );
    let _ = s.h_run;
    s.h_sum = mk(
        hwnd,
        "STATIC",
        "",
        SS_LEFT.0 | SS_CENTERIMAGE.0 | SS_NOPREFIX.0,
        WINDOW_EX_STYLE(0),
        0,
        0,
        0,
        0,
        ID_TW_SUM,
    );
    s.h_grid = super::grid::create(hwnd, 0, 0, 0, 0, true, ID_TW_GRID);
    if let Some(p) = &m.three_way_base {
        ctext(s.h_eb, &p.to_string_lossy());
    }
    if let Some(p) = &m.three_way_left {
        ctext(s.h_el, &p.to_string_lossy());
    }
    if let Some(p) = &m.three_way_right {
        ctext(s.h_er, &p.to_string_lossy());
    }
    style_children(hwnd, s.font, s.h_grid);
    set_font(s.h_grid, s.mono);
}

/// 従来(Free)の 3-way シートの WM_CREATE 以外のメッセージ処理
unsafe fn tw_wndproc_rest(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let st = Box::from_raw(ptr as *mut ThreeWin);
                    let _ = DeleteObject(HGDIOBJ(st.font.0));
                    let _ = DeleteObject(HGDIOBJ(st.mono.0));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_SIZE => {
            tw_layout(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ThreeWin) };
            let m = unsafe { &mut *s.model };
            match id {
                ID_BTN_BASE | ID_BTN_LEFT | ID_BTN_RIGHT => {
                    let title = match id {
                        ID_BTN_BASE => crate::strings::PICK_BASE,
                        ID_BTN_LEFT => crate::strings::PICK_LEFT_FILE,
                        _ => crate::strings::PICK_RIGHT_FILE,
                    };
                    if let Some(p) = super::picker::pick_file(hwnd, title) {
                        match id {
                            ID_BTN_BASE => {
                                m.three_way_base = Some(p.clone());
                                ctext(s.h_eb, &p.to_string_lossy());
                            }
                            ID_BTN_LEFT => {
                                m.three_way_left = Some(p.clone());
                                ctext(s.h_el, &p.to_string_lossy());
                            }
                            _ => {
                                m.three_way_right = Some(p.clone());
                                ctext(s.h_er, &p.to_string_lossy());
                            }
                        }
                    }
                }
                ID_BTN_RUN => {
                    let (Some(b), Some(l), Some(r)) = (
                        m.three_way_base.clone(),
                        m.three_way_left.clone(),
                        m.three_way_right.clone(),
                    ) else {
                        return LRESULT(0);
                    };
                    let cc = m.settings.content_comparison();
                    let text = |p: &PathBuf| -> String {
                        match diffloupe_core::text_loader::load(p) {
                            Ok(diffloupe_core::text_loader::TextLoadResult::Text(t, _)) => t,
                            _ => String::new(),
                        }
                    };
                    let diff = diffloupe_core::three_way::ThreeWayDiff::make(
                        &text(&b),
                        &text(&l),
                        &text(&r),
                        &cc,
                    );
                    let sum = format!(
                        "{} {}   {} {}",
                        crate::strings::TW_CONFLICTS,
                        diff.conflict_count,
                        crate::strings::TW_CHANGES,
                        diff.total_changed_count()
                    );
                    ctext(s.h_sum, &sum);
                    let items = crate::grid_model::build_threeway_items(&diff);
                    let cols = crate::grid_model::threeway_columns();
                    super::grid::set_model(
                        s.h_grid,
                        crate::grid_model::GridModel {
                            columns: cols,
                            items,
                        },
                        true,
                    );
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => unsafe {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        },
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ---------- 任意 ref 入力 ----------

/// モーダルな1行入力。OK なら Some(文字列)
pub fn input_ref(parent: HWND, title: &str, initial: &str) -> Option<String> {
    struct Ctx {
        out: Option<String>,
        edit: HWND,
        done: bool,
    }
    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        super::guard::run("input", hwnd, msg, wparam, lparam, false, || unsafe {
            proc_inner(hwnd, msg, wparam, lparam)
        })
    }

    unsafe fn proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, _l: LPARAM) -> LRESULT {
        if let Some(r) = dlg_common(hwnd, msg, wparam) {
            return r;
        }
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
        if ptr == 0 {
            // WM_CREATE 等、ユーザーデータ設定前
            return unsafe { DefWindowProcW(hwnd, msg, wparam, _l) };
        }
        let ctx = unsafe { &mut *(ptr as *mut Ctx) };
        match msg {
            WM_COMMAND => match (wparam.0 & 0xFFFF) as i32 {
                1 => {
                    ctx.out =
                        Some(get_edit_text(ctx.edit).trim().to_string()).filter(|s| !s.is_empty());
                    ctx.done = true;
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                2 => {
                    ctx.done = true;
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                _ => {}
            },
            WM_CLOSE => {
                ctx.done = true;
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, _l) },
        }
        LRESULT(0)
    }

    let class = wstr("DiffLoupeRefInput");
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(proc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let mut ctx = Box::new(Ctx {
        out: None,
        edit: HWND::default(),
        done: false,
    });
    let dpi = dpi_of(parent);
    let sc = |v: i32| v * dpi as i32 / 96;
    let ui = font::make_ui_font(dpi);
    let row = (font::font_height(ui) + sc(10)).max(sc(24));
    let (pad, gap) = (sc(12), sc(8));
    let btn_w = (font::measure(ui, crate::strings::APPLY).cx + sc(40))
        .max(font::measure(ui, crate::strings::CANCEL).cx + sc(40))
        .max(sc(88));
    // クライアント寸法 → ウィンドウ外寸
    let client_w = sc(420);
    let client_h = pad * 2 + row * 2 + gap;
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_CLIPCHILDREN;
    let mut r = RECT {
        left: 0,
        top: 0,
        right: client_w,
        bottom: client_h,
    };
    unsafe {
        let _ = windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(
            &mut r,
            style,
            false,
            WINDOW_EX_STYLE(0),
            dpi,
        );
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(title).as_ptr()),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            r.right - r.left,
            r.bottom - r.top,
            Some(parent),
            None,
            Some(hinst()),
            Some(ctx.as_mut() as *mut Ctx as *const _),
        )
        .unwrap_or_default()
    };
    ctx.edit = mk(
        hwnd,
        "EDIT",
        "",
        ES_AUTOHSCROLL as u32 | WS_TABSTOP.0,
        WS_EX_CLIENTEDGE,
        pad,
        pad,
        client_w - pad * 2,
        row,
        100,
    );
    let e = ctx.edit;
    ctext(e, initial);
    let cue = wstr(crate::strings::REF_INPUT_CUE);
    unsafe {
        SendMessageW(
            e,
            EM_SETCUEBANNER,
            Some(WPARAM(1)),
            Some(LPARAM(cue.as_ptr() as isize)),
        );
    }
    let by = pad + row + gap;
    mk(
        hwnd,
        "BUTTON",
        crate::strings::APPLY,
        (BS_PUSHBUTTON | BS_DEFPUSHBUTTON) as u32 | WS_TABSTOP.0,
        WINDOW_EX_STYLE(0),
        client_w - pad - btn_w * 2 - gap,
        by,
        btn_w,
        row,
        1,
    );
    mk(
        hwnd,
        "BUTTON",
        crate::strings::CANCEL,
        BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
        WINDOW_EX_STYLE(0),
        client_w - pad - btn_w,
        by,
        btn_w,
        row,
        2,
    );
    style_children(hwnd, ui, HWND::default());
    crate::win::theme::apply_title_bar(hwnd, crate::win::theme::cached().dark);
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx.as_mut() as *mut Ctx as isize);
        let _ = EnableWindow(parent, false);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(e));
    }
    // モーダルループ
    unsafe {
        let mut m = MSG::default();
        while !ctx.done && GetMessageW(&mut m, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
        let _ = EnableWindow(parent, true);
        let _ = SetForegroundWindow(parent);
        let _ = DeleteObject(HGDIOBJ(ui.0));
    }
    ctx.out
}
