//! 設定・コピー計画・3-way・ref入力・バージョン情報の各ウィンドウ。
//! すべてモードレス(親ウィンドウのハンドルと AppModel へのポインタを受け取る)。

use std::path::PathBuf;
use windows::core::PWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::SS_LEFT;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::appmodel::AppModel;
use crate::win::wide::wstr;

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
    font: HFONT,
    model: *mut AppModel,
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
const ID_LBL_EXCL: i32 = 211;
const ID_LBL_INCL: i32 = 212;
const ID_CHK_AUTOREF: i32 = 213;
const ID_PRESET_BASE: i32 = 220; // +i
const ID_BTN_CLOSE: i32 = 230;
const ID_LBL_EDITION: i32 = 231;

const SET_CLASS: &str = "DiffLoupeSettings";

pub fn open_settings(main: HWND, model: *mut AppModel, font: HFONT) -> HWND {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let dpi = unsafe { GetDpiForWindow(main) } as i32;
    let sc = |v: i32| v * dpi / 96;
    let class = wstr(SET_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(settings_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let st = Box::new(SettingsWin { main, font, model });
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::SETTINGS_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            sc(520),
            sc(660),
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

fn settings_create(hwnd: HWND) {
    let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
    let m = unsafe { &mut *s.model };
    let sc = |v: i32| crate::win::dpi::scale(hwnd, v);
    let mut y = 8;
    let mut ypos = y;
    // エディション
    let grp = mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_EDITION,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(56),
        0,
    );
    y = ypos + 18;
    let lbl = mk(
        hwnd,
        "STATIC",
        "",
        SS_LEFT.0,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(y),
        sc(476),
        sc(20),
        ID_LBL_EDITION,
    );
    let ed_note = format!(
        "{}{}",
        m.edition.name(),
        if m.edition.max_folder_entries().is_some() {
            crate::strings::EDITION_LIMIT_NOTE
        } else {
            ""
        }
    );
    ctext(
        lbl,
        &format!(
            "{}: {}  {}",
            crate::strings::CURRENT_EDITION,
            m.edition.name(),
            ed_note
        ),
    );
    ypos += 64;

    // 比較
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_COMPARE,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(92),
        0,
    );
    y = ypos + 18;
    let c1 = mk(
        hwnd,
        "BUTTON",
        crate::strings::OPT_RECURSIVE,
        BS_AUTOCHECKBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(y),
        sc(476),
        sc(20),
        ID_CHK_RECURSIVE,
    );
    y += 24;
    let c2 = mk(
        hwnd,
        "BUTTON",
        crate::strings::OPT_DEFAULT_EXCLUDES,
        BS_AUTOCHECKBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(y),
        sc(476),
        sc(20),
        ID_CHK_DEFEXCL,
    );
    y += 24;
    let c3 = mk(
        hwnd,
        "BUTTON",
        crate::strings::OPT_RENAMES,
        BS_AUTOCHECKBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(y),
        sc(476),
        sc(20),
        ID_CHK_RENAMES,
    );
    checkbox(c1, m.settings.recursive_comparison);
    checkbox(c2, m.settings.use_default_excludes);
    checkbox(c3, m.settings.detect_renames);
    ypos += 100;

    // 内容比較の正規化
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_NORMALIZE,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(150),
        0,
    );
    y = ypos + 18;
    let checks = [
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
    ];
    for (id, text, on) in checks {
        let c = mk(
            hwnd,
            "BUTTON",
            text,
            BS_AUTOCHECKBOX as u32,
            WINDOW_EX_STYLE(0),
            sc(16),
            sc(y),
            sc(300),
            sc(20),
            id,
        );
        checkbox(c, on);
        y += 22;
    }
    mk(
        hwnd,
        "STATIC",
        crate::strings::OPT_STRUCTURED,
        SS_LEFT.0,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(y + 2),
        sc(80),
        sc(20),
        0,
    );
    let cmb = mk(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32,
        WINDOW_EX_STYLE(0),
        sc(100),
        sc(y),
        sc(200),
        sc(200),
        ID_CMB_STRUCT,
    );
    let cur = m.settings.content_structured_mode.clone();
    for (i, (_m, label)) in crate::settings::AppSettings::structured_modes()
        .iter()
        .enumerate()
    {
        unsafe {
            let t = wstr(label);
            SendMessageW(
                cmb,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(t.as_ptr() as isize)),
            );
            let _ = i;
        }
    }
    let sel = crate::settings::AppSettings::structured_modes()
        .iter()
        .position(|(mm, _)| mm.raw_value() == cur)
        .unwrap_or(0);
    unsafe {
        SendMessageW(cmb, CB_SETCURSEL, Some(WPARAM(sel)), Some(LPARAM(0)));
    }
    ypos += 158;

    // 除外パターン
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_EXCLUDE,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(120),
        0,
    );
    let ed = mk(
        hwnd,
        "EDIT",
        "",
        (ES_MULTILINE | ES_WANTRETURN) as u32 | WS_VSCROLL.0,
        WS_EX_CLIENTEDGE,
        sc(16),
        sc(ypos + 18),
        sc(476),
        sc(70),
        ID_ED_EXCL,
    );
    ctext(ed, &m.settings.user_exclude_patterns);
    mk(
        hwnd,
        "STATIC",
        crate::strings::PATTERN_HINT,
        SS_LEFT.0,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(ypos + 92),
        sc(476),
        sc(20),
        ID_LBL_EXCL,
    );
    ypos += 128;

    // インクルード
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_INCLUDE,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(120),
        0,
    );
    let ed2 = mk(
        hwnd,
        "EDIT",
        "",
        (ES_MULTILINE | ES_WANTRETURN) as u32 | WS_VSCROLL.0,
        WS_EX_CLIENTEDGE,
        sc(16),
        sc(ypos + 18),
        sc(476),
        sc(70),
        ID_ED_INCL,
    );
    ctext(ed2, &m.settings.user_include_patterns);
    mk(
        hwnd,
        "STATIC",
        crate::strings::INCLUDE_NOTE,
        SS_LEFT.0,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(ypos + 92),
        sc(476),
        sc(20),
        ID_LBL_INCL,
    );
    ypos += 128;

    // 自動再比較
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_AUTO_REFRESH,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(52),
        0,
    );
    let ar = mk(
        hwnd,
        "BUTTON",
        crate::strings::OPT_AUTO_REFRESH,
        BS_AUTOCHECKBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(16),
        sc(ypos + 20),
        sc(300),
        sc(20),
        ID_CHK_AUTOREF,
    );
    checkbox(ar, m.settings.auto_refresh_enabled);
    ypos += 60;

    // プリセット
    let presets = diffloupe_core::FILTER_PRESETS;
    let gh = 24 + presets.len() as i32 * 24;
    mk(
        hwnd,
        "BUTTON",
        crate::strings::SEC_PRESETS,
        BS_GROUPBOX as u32,
        WINDOW_EX_STYLE(0),
        sc(8),
        sc(ypos),
        sc(496),
        sc(gh),
        0,
    );
    let sel_ids: Vec<String> = m.settings.selected_preset_ids();
    for (i, p) in presets.iter().enumerate() {
        let c = mk(
            hwnd,
            "BUTTON",
            p.display_name,
            BS_AUTOCHECKBOX as u32,
            WINDOW_EX_STYLE(0),
            sc(16),
            sc(ypos + 20 + i as i32 * 22),
            sc(476),
            sc(20),
            ID_PRESET_BASE + i as i32,
        );
        checkbox(c, sel_ids.iter().any(|id| id == p.id));
    }
    ypos += gh + 8;

    let btn = mk(
        hwnd,
        "BUTTON",
        crate::strings::CLOSE,
        (BS_PUSHBUTTON | BS_DEFPUSHBUTTON) as u32,
        WINDOW_EX_STYLE(0),
        sc(420),
        sc(ypos + 4),
        sc(84),
        sc(26),
        ID_BTN_CLOSE,
    );
    let _ = btn;
    let _ = grp;
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
            settings_create(hwnd);
            // 全子コントロールにフォント適用
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
            unsafe {
                let mut child = GetWindow(hwnd, GW_CHILD);
                while let Ok(c) = child {
                    set_font(c, s.font);
                    child = GetWindow(c, GW_HWNDNEXT);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut SettingsWin);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsWin) };
            let m = unsafe { &mut *s.model };
            let get = |id: i32| unsafe { GetDlgItem(Some(hwnd), id).unwrap_or_default() };
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
    h_dir: HWND,
    h_list: HWND,
    h_sum: HWND,
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
    font: HFONT,
    direction: diffloupe_core::sync_plan::SyncDirection,
) -> HWND {
    let class = wstr(SYNC_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(sync_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let m = unsafe { &mut *model };
    m.prepare_sync_plan(direction);
    let st = Box::new(SyncWin {
        _main: main,
        font,
        model,
        h_dir: HWND::default(),
        h_list: HWND::default(),
        h_sum: HWND::default(),
    });
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::SYNC_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            520,
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

unsafe fn sync_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
            let m = unsafe { &*s.model };
            s.h_dir = mk(
                hwnd,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32,
                WINDOW_EX_STYLE(0),
                70,
                10,
                160,
                200,
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
            mk(
                hwnd,
                "STATIC",
                crate::strings::SYNC_DIRECTION,
                SS_LEFT.0,
                WINDOW_EX_STYLE(0),
                12,
                13,
                56,
                20,
                0,
            );
            s.h_sum = mk(
                hwnd,
                "STATIC",
                "",
                SS_LEFT.0,
                WINDOW_EX_STYLE(0),
                12,
                40,
                730,
                20,
                ID_SUMMARY,
            );
            s.h_list = mk_pcw(
                hwnd,
                WC_LISTVIEW,
                "",
                LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
                WS_EX_CLIENTEDGE,
                12,
                66,
                730,
                400,
                ID_LIST,
            );
            let headers = [
                crate::strings::SYNC_COL_ACTION,
                crate::strings::SYNC_COL_PATH,
                crate::strings::SYNC_COL_SOURCE,
                crate::strings::SYNC_COL_SIZE,
                crate::strings::SYNC_COL_REASON,
            ];
            let widths = [130, 250, 200, 80, 170];
            for (i, (h, w)) in headers.iter().zip(widths).enumerate() {
                let t = wstr(h);
                let col = LVCOLUMNW {
                    mask: LVCF_TEXT | LVCF_WIDTH,
                    cx: w,
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
            set_font(s.h_dir, s.font);
            set_font(s.h_list, s.font);
            set_font(s.h_sum, s.font);
            sync_fill(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut SyncWin);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SyncWin) };
            let w = (lparam.0 & 0xFFFF) as i32;
            let h = ((lparam.0 >> 16) & 0xFFFF) as i32;
            unsafe {
                let _ = MoveWindow(s.h_list, 12, 66, w - 24, h - 78, true);
            }
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
    model: *mut AppModel,
    h_base: HWND,
    h_left: HWND,
    h_right: HWND,
    h_eb: HWND,
    h_el: HWND,
    h_er: HWND,
    h_grid: HWND,
    h_sum: HWND,
}

pub fn open_threeway(main: HWND, model: *mut AppModel, font: HFONT) -> HWND {
    let class = wstr(TW_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst(),
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(tw_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let st = Box::new(ThreeWin {
        _main: main,
        font,
        model,
        h_base: HWND::default(),
        h_left: HWND::default(),
        h_right: HWND::default(),
        h_eb: HWND::default(),
        h_el: HWND::default(),
        h_er: HWND::default(),
        h_grid: HWND::default(),
        h_sum: HWND::default(),
    });
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::THREE_WAY_TITLE).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            960,
            620,
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
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let w = rc.right;
    unsafe {
        let _ = MoveWindow(s.h_base, 8, 8, 96, 24, true);
        let _ = MoveWindow(s.h_eb, 108, 8, w - 120, 22, true);
        let _ = MoveWindow(s.h_left, 8, 38, 96, 24, true);
        let _ = MoveWindow(s.h_el, 108, 38, w - 120, 22, true);
        let _ = MoveWindow(s.h_right, 8, 68, 96, 24, true);
        let _ = MoveWindow(s.h_er, 108, 68, w - 230, 22, true);
        if let Ok(b) = GetDlgItem(Some(hwnd), ID_BTN_RUN) {
            let _ = MoveWindow(b, w - 116, 68, 108, 24, true);
        }
        let _ = MoveWindow(s.h_sum, 8, 100, w - 16, 20, true);
        let _ = MoveWindow(s.h_grid, 0, 126, w, rc.bottom - 126, true);
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
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            let s = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ThreeWin) };
            let m = unsafe { &*s.model };
            s.h_base = mk(
                hwnd,
                "BUTTON",
                crate::strings::BASE_FILE,
                BS_PUSHBUTTON as u32,
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
                ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
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
                ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
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
                ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
                WS_EX_CLIENTEDGE,
                0,
                0,
                0,
                0,
                ID_ED_RIGHT,
            );
            mk(
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
            s.h_sum = mk(
                hwnd,
                "STATIC",
                "",
                SS_LEFT.0,
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
            for c in [
                s.h_base, s.h_left, s.h_right, s.h_eb, s.h_el, s.h_er, s.h_sum,
            ] {
                set_font(c, s.font);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut ThreeWin);
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
        let ctx = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Ctx) };
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
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let mut ctx = Box::new(Ctx {
        out: None,
        edit: HWND::default(),
        done: false,
    });
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(title).as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            360,
            110,
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
        ES_AUTOHSCROLL as u32,
        WS_EX_CLIENTEDGE,
        10,
        12,
        330,
        22,
        100,
    );
    let e = ctx.edit;
    ctext(e, initial);
    mk(
        hwnd,
        "BUTTON",
        crate::strings::APPLY,
        (BS_PUSHBUTTON | BS_DEFPUSHBUTTON) as u32,
        WINDOW_EX_STYLE(0),
        180,
        44,
        78,
        24,
        1,
    );
    mk(
        hwnd,
        "BUTTON",
        crate::strings::CANCEL,
        BS_PUSHBUTTON as u32,
        WINDOW_EX_STYLE(0),
        264,
        44,
        78,
        24,
        2,
    );
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx.as_mut() as *mut Ctx as isize);
        let _ = EnableWindow(parent, false);
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
    }
    ctx.out
}
