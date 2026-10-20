//! カスタム描画グリッド("DiffLoupeGrid")。テキスト差分(2列)・Hex(5列)・
//! 3-way(3列)を共通部品で描画する。行単位配色・インライン強調・折りたたみ
//! ヘッダ・縦横スクロール・キーボード操作を提供する。

use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::SetScrollInfo;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::grid_model::{CellTone, GridItem, GridModel};
use crate::win::theme::{self, Theme};
use crate::win::wide::wstr;

pub const CLASS_NAME: &str = "DiffLoupeGrid";
/// 折りたたみヘッダがクリックされた(親へ通知。wParam=group_id)
pub const WM_APP_GRID_FOLD: u32 = WM_APP + 40;

struct Grid {
    model: GridModel,
    theme: Theme,
    font: HFONT,
    row_h: i32,
    scroll_x: i32,
    scroll_y: i32, // 先頭可視行インデックス
    col_w: Vec<i32>,
    content_w: i32,
    total_h: i32,
    search_hits: HashSet<usize>,
    current_hit: Option<usize>,
    current_diff: Option<usize>,
    mono: bool,
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

pub fn create(parent: HWND, x: i32, y: i32, w: i32, h: i32, mono: bool, id: i32) -> HWND {
    let class = wstr(CLASS_NAME);
    let g = Box::new(Grid {
        model: GridModel {
            columns: vec![],
            items: vec![],
        },
        theme: theme::current(),
        font: HFONT::default(),
        row_h: 18,
        scroll_x: 0,
        scroll_y: 0,
        col_w: vec![],
        content_w: 0,
        total_h: 0,
        search_hits: HashSet::new(),
        current_hit: None,
        current_diff: None,
        mono,
    });
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_HSCROLL | WS_TABSTOP | WS_CLIPCHILDREN,
            x,
            y,
            w,
            h,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            GetModuleHandleW(None).ok().map(|m| m.into()),
            Some(Box::into_raw(g) as *const _),
        )
        .unwrap_or_default()
    }
}

fn state(hwnd: HWND) -> &'static mut Grid {
    unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Grid) }
}

pub fn set_model(hwnd: HWND, model: GridModel, mono: bool) {
    let g = state(hwnd);
    g.model = model;
    g.mono = mono;
    g.col_w.clear();
    g.scroll_x = 0;
    g.scroll_y = 0;
    g.search_hits.clear();
    g.current_hit = None;
    g.current_diff = None;
    relayout(hwnd);
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

pub fn set_marks(
    hwnd: HWND,
    search_hits: HashSet<usize>,
    current_hit: Option<usize>,
    current_diff: Option<usize>,
) {
    let g = state(hwnd);
    g.search_hits = search_hits;
    g.current_hit = current_hit;
    g.current_diff = current_diff;
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

/// source_id を含む表示アイテムへスクロール(なければ false)
pub fn scroll_to_source(hwnd: HWND, source_id: usize) -> bool {
    let g = state(hwnd);
    let Some(idx) = g.model.row_item_index_by_source(source_id) else {
        return false;
    };
    scroll_to_row(hwnd, idx);
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
    true
}

fn scroll_to_row(hwnd: HWND, idx: usize) {
    let g = state(hwnd);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let visible = (rc.bottom / g.row_h).max(1);
    let idx = idx as i32;
    if idx < g.scroll_y {
        g.scroll_y = idx;
    } else if idx >= g.scroll_y + visible {
        g.scroll_y = idx - visible + 1;
    }
    update_scrollbars(hwnd);
}

fn text_width(hdc: HDC, text: &[u16]) -> i32 {
    let mut size = SIZE::default();
    unsafe {
        let _ = GetTextExtentPoint32W(hdc, text, &mut size);
    }
    size.cx
}

fn tone_color(theme: &Theme, tone: CellTone) -> COLORREF {
    match tone {
        CellTone::Normal => theme.window_bg,
        CellTone::Added => theme.added_bg,
        CellTone::Removed => theme.removed_bg,
        CellTone::Modified => theme.modified_bg,
        CellTone::Info => theme.info_bg,
    }
}

fn hl_color(theme: &Theme, tone: CellTone) -> COLORREF {
    match tone {
        CellTone::Added => theme.added_hl,
        CellTone::Removed => theme.removed_hl,
        CellTone::Modified => theme.modified_hl,
        _ => theme.modified_hl,
    }
}

fn relayout(hwnd: HWND) {
    let g = state(hwnd);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let view_w = rc.right;
    g.total_h = g.model.items.len() as i32 * g.row_h;
    // 列幅: weight>0 は残り幅を比率で分割、weight=0 はタイトル幅
    let ncols = g.model.columns.len();
    g.col_w = vec![0; ncols];
    let hdc = unsafe { GetDC(Some(hwnd)) };
    if !g.font.is_invalid() && !hdc.is_invalid() {
        unsafe {
            SelectObject(hdc, HGDIOBJ(g.font.0));
        }
    }
    // 固定列: ヘッダまたは代表幅
    let mut fixed = 0;
    let mut weights = 0.0f64;
    for (i, c) in g.model.columns.iter().enumerate() {
        if c.weight <= 0.0 {
            let mut w = text_width(hdc, &wstr(&c.title)) + crate::win::dpi::scale(hwnd, 20);
            // 代表サンプル(先頭 64 行)で内容幅を推定
            for item in g.model.items.iter().take(64) {
                if let GridItem::Row { cells, .. } = item {
                    if let Some(cell) = cells.get(i) {
                        w = w.max(
                            text_width(hdc, &wstr(&cell.text)) + crate::win::dpi::scale(hwnd, 12),
                        );
                    }
                }
            }
            g.col_w[i] = w;
            fixed += w;
        } else {
            weights += c.weight;
        }
    }
    unsafe {
        ReleaseDC(Some(hwnd), hdc);
    }
    let remain = (view_w - fixed).max(0);
    for (i, c) in g.model.columns.iter().enumerate() {
        if c.weight > 0.0 {
            g.col_w[i] = (remain as f64 * c.weight / weights) as i32;
        }
    }
    update_scrollbars(hwnd);
}

fn update_scrollbars(hwnd: HWND) {
    let g = state(hwnd);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let view_w = rc.right;
    let view_h = rc.bottom;
    let content_w = g.col_w.iter().sum::<i32>().max(g.content_w);
    let nrows = g.model.items.len() as i32;
    // 縦: 行単位
    let mut si = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: (nrows - 1).max(0),
        nPage: (view_h / g.row_h).max(1) as u32,
        nPos: g.scroll_y,
        ..Default::default()
    };
    unsafe {
        let _ = SetScrollInfo(hwnd, SB_VERT, &si, true);
        si.fMask = SIF_RANGE | SIF_PAGE | SIF_POS;
        si.nMin = 0;
        si.nMax = content_w.max(1) - 1;
        si.nPage = view_w.max(1) as u32;
        si.nPos = g.scroll_x;
        let _ = SetScrollInfo(hwnd, SB_HORZ, &si, true);
    }
}

fn draw_highlighted_text(
    hdc: HDC,
    x: i32,
    y: i32,
    text: &str,
    hl: &[std::ops::Range<usize>],
    hl_col: COLORREF,
) -> i32 {
    // text を UTF-16 化し、強調範囲を持つセグメントと通常セグメントに分割して描画する
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut cx = x;
    let mut pos = 0usize;
    let mut sorted: Vec<_> = hl.to_vec();
    sorted.sort_by_key(|r| r.start);
    let row_h = unsafe { text_metric_height(hdc) };
    for r in sorted {
        let (s, e) = (r.start.min(wide.len()), r.end.min(wide.len()));
        if s > pos {
            cx += draw_seg(hdc, cx, y, &wide[pos..s]);
        }
        if e > s {
            let w = seg_width(hdc, &wide[s..e]);
            unsafe {
                let rect = RECT {
                    left: cx,
                    top: y,
                    right: cx + w,
                    bottom: y + row_h,
                };
                let brush = CreateSolidBrush(hl_col);
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
            cx += draw_seg(hdc, cx, y, &wide[s..e]);
        }
        pos = e.max(pos);
    }
    if pos < wide.len() {
        cx += draw_seg(hdc, cx, y, &wide[pos..]);
    }
    cx - x
}

unsafe fn text_metric_height(hdc: HDC) -> i32 {
    unsafe {
        let mut tm = TEXTMETRICW::default();
        let _ = GetTextMetricsW(hdc, &mut tm);
        tm.tmHeight
    }
}

fn seg_width(hdc: HDC, seg: &[u16]) -> i32 {
    if seg.is_empty() {
        return 0;
    }
    text_width(hdc, seg)
}

fn draw_seg(hdc: HDC, x: i32, y: i32, seg: &[u16]) -> i32 {
    if seg.is_empty() {
        return 0;
    }
    let w = seg_width(hdc, seg);
    unsafe {
        let _ = TextOutW(hdc, x, y, seg);
    }
    w
}

fn paint(hwnd: HWND) {
    let g = state(hwnd);
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    if hdc.is_invalid() {
        return;
    }
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let bg = CreateSolidBrush(g.theme.window_bg);
        FillRect(hdc, &rc, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));
        if g.font.is_invalid() {
            return;
        }
        SelectObject(hdc, HGDIOBJ(g.font.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, g.theme.text);
        let clip_top = ps.rcPaint.top.max(0);
        let clip_bottom = ps.rcPaint.bottom;
        let first = (clip_top / g.row_h + g.scroll_y).max(0);
        let last =
            ((clip_bottom + g.row_h - 1) / g.row_h + g.scroll_y).min(g.model.items.len() as i32);
        let gutter_w = crate::win::dpi::scale(hwnd, 44);
        let pad = crate::win::dpi::scale(hwnd, 4);
        // 列の x オフセット
        let mut col_x = Vec::with_capacity(g.col_w.len());
        let mut x = -g.scroll_x;
        for w in &g.col_w {
            col_x.push(x);
            x += w;
        }
        for row_idx in first..last {
            let item = &g.model.items[row_idx as usize];
            let top = (row_idx - g.scroll_y) * g.row_h;
            let row_rect = RECT {
                left: 0,
                top,
                right: rc.right,
                bottom: top + g.row_h,
            };
            match item {
                GridItem::FoldHeader { group_id, count } => {
                    let brush = CreateSolidBrush(g.theme.info_bg);
                    FillRect(hdc, &row_rect, brush);
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                    SetTextColor(hdc, g.theme.dim_text);
                    let _ = group_id;
                    let label = wstr(&format!("{} {}", count, crate::strings::FOLD_SHOW_LINES));
                    let mut labelv = label[..label.len() - 1].to_vec();
                    let mut tr = row_rect;
                    let _ = DrawTextW(
                        hdc,
                        &mut labelv,
                        &mut tr,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }
                GridItem::Row { cells, source_id } => {
                    for (ci, cell) in cells.iter().enumerate() {
                        let Some(&cx) = col_x.get(ci) else { continue };
                        let cw = g.col_w[ci];
                        let cell_rect = RECT {
                            left: cx.max(0),
                            top,
                            right: (cx + cw).min(rc.right),
                            bottom: top + g.row_h,
                        };
                        if cell_rect.right <= 0 || cell_rect.left >= rc.right {
                            continue;
                        }
                        // 背景
                        let tone = tone_color(&g.theme, cell.tone);
                        let brush = CreateSolidBrush(tone);
                        FillRect(hdc, &cell_rect, brush);
                        let _ = DeleteObject(HGDIOBJ(brush.0));
                        // ガター(行番号)
                        let mut tx = cx + pad;
                        if let Some(gtxt) = cell.gutter.as_deref() {
                            let mut tr = RECT {
                                left: cx + pad,
                                top,
                                right: cx + gutter_w,
                                bottom: top + g.row_h,
                            };
                            SetTextColor(hdc, g.theme.dim_text);
                            let gw = wstr(gtxt);
                            let mut gwv = gw[..gw.len() - 1].to_vec();
                            let _ = DrawTextW(
                                hdc,
                                &mut gwv,
                                &mut tr,
                                DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                            );
                            tx = cx + gutter_w;
                        }
                        // 本文
                        SetTextColor(hdc, g.theme.text);
                        let hlc = hl_color(&g.theme, cell.tone);
                        let ty = top + (g.row_h - text_metric_height(hdc)) / 2;
                        let used =
                            draw_highlighted_text(hdc, tx, ty, &cell.text, &cell.highlights, hlc);
                        let end = tx + used + pad;
                        if end > g.content_w {
                            g.content_w = end;
                        }
                    }
                    // 検索ヒット印: 行右端の短い帯
                    if g.search_hits.contains(source_id) {
                        let col = if g.current_hit == Some(*source_id) {
                            g.theme.current_hit
                        } else {
                            g.theme.search_hit
                        };
                        let brush = CreateSolidBrush(col);
                        let r = RECT {
                            left: rc.right - 4,
                            top,
                            right: rc.right,
                            bottom: top + g.row_h,
                        };
                        FillRect(hdc, &r, brush);
                        let _ = DeleteObject(HGDIOBJ(brush.0));
                    }
                    if g.current_diff == Some(*source_id) {
                        let brush = CreateSolidBrush(g.theme.accent);
                        let r = RECT {
                            left: 0,
                            top,
                            right: 3,
                            bottom: top + g.row_h,
                        };
                        FillRect(hdc, &r, brush);
                        let _ = DeleteObject(HGDIOBJ(brush.0));
                    }
                }
            }
        }
        // 下の余白は window_bg のまま
        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    super::guard::run("grid", hwnd, msg, wparam, lparam, true, || unsafe {
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
            let g = state(hwnd);
            g.font = super::app::make_font(hwnd, g.mono);
            g.row_h = unsafe {
                let hdc = GetDC(Some(hwnd));
                SelectObject(hdc, HGDIOBJ(g.font.0));
                let mut tm = TEXTMETRICW::default();
                let _ = GetTextMetricsW(hdc, &mut tm);
                ReleaseDC(Some(hwnd), hdc);
                tm.tmHeight + crate::win::dpi::scale(hwnd, 5)
            };
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut Grid);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); // 解放済み状態への再入を防ぐ
                }
            }
            LRESULT(0)
        }
        WM_SETFONT => {
            let g = state(hwnd);
            g.font = HFONT(wparam.0 as *mut _);
            let hdc = unsafe { GetDC(Some(hwnd)) };
            unsafe {
                SelectObject(hdc, HGDIOBJ(g.font.0));
                let mut tm = TEXTMETRICW::default();
                let _ = GetTextMetricsW(hdc, &mut tm);
                ReleaseDC(Some(hwnd), hdc);
                g.row_h = tm.tmHeight + crate::win::dpi::scale(hwnd, 5);
            }
            relayout(hwnd);
            LRESULT(0)
        }
        WM_SIZE => {
            relayout(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
            let lines = delta / WHEEL_DELTA as i32 * 3;
            let g = state(hwnd);
            g.scroll_y = (g.scroll_y - lines).clamp(0, (g.model.items.len() as i32 - 1).max(0));
            update_scrollbars(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_VSCROLL => {
            let g = state(hwnd);
            let max = (g.model.items.len() as i32 - 1).max(0);
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let page = (rc.bottom / g.row_h).max(1);
            let code = (wparam.0 & 0xFFFF) as u16;
            g.scroll_y = match code {
                x if x == SB_LINEUP.0 as u16 => g.scroll_y - 1,
                x if x == SB_LINEDOWN.0 as u16 => g.scroll_y + 1,
                x if x == SB_PAGEUP.0 as u16 => g.scroll_y - page,
                x if x == SB_PAGEDOWN.0 as u16 => g.scroll_y + page,
                x if x == SB_TOP.0 as u16 => 0,
                x if x == SB_BOTTOM.0 as u16 => max,
                x if x == SB_THUMBPOSITION.0 as u16 || x == SB_THUMBTRACK.0 as u16 => {
                    ((wparam.0 >> 16) & 0xFFFF) as i32
                }
                _ => g.scroll_y,
            };
            g.scroll_y = g.scroll_y.clamp(0, max);
            update_scrollbars(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_HSCROLL => {
            let g = state(hwnd);
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let page = rc.bottom.max(1);
            let content_w = g.col_w.iter().sum::<i32>().max(g.content_w);
            let max = (content_w - 1).max(0);
            let code = (wparam.0 & 0xFFFF) as u16;
            g.scroll_x = match code {
                x if x == SB_LINELEFT.0 as u16 => g.scroll_x - 24,
                x if x == SB_LINERIGHT.0 as u16 => g.scroll_x + 24,
                x if x == SB_PAGELEFT.0 as u16 => g.scroll_x - page,
                x if x == SB_PAGERIGHT.0 as u16 => g.scroll_x + page,
                x if x == SB_THUMBPOSITION.0 as u16 || x == SB_THUMBTRACK.0 as u16 => {
                    ((wparam.0 >> 16) & 0xFFFF) as i32
                }
                _ => g.scroll_x,
            };
            g.scroll_x = g.scroll_x.clamp(0, max);
            update_scrollbars(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = lparam.0 as i32 as i16 as i32; // GET_Y_LPARAM
            let g = state(hwnd);
            let row = g.scroll_y + y / g.row_h.max(1);
            if let Some(GridItem::FoldHeader { group_id, .. }) =
                g.model.items.get(row.max(0) as usize)
            {
                unsafe {
                    let parent = GetParent(hwnd).unwrap_or_default();
                    let _ = SendMessageW(
                        parent,
                        WM_APP_GRID_FOLD,
                        Some(WPARAM(*group_id)),
                        Some(LPARAM(0)),
                    );
                }
            }
            unsafe {
                let _ = SetFocus(Some(hwnd));
            };
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let g = state(hwnd);
            let max = (g.model.items.len() as i32 - 1).max(0);
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let page = (rc.bottom / g.row_h).max(1);
            use windows::Win32::UI::Input::KeyboardAndMouse::*;
            match VIRTUAL_KEY(wparam.0 as u16) {
                x if x == VK_UP => g.scroll_y = (g.scroll_y - 1).max(0),
                x if x == VK_DOWN => g.scroll_y = (g.scroll_y + 1).min(max),
                x if x == VK_PRIOR => g.scroll_y = (g.scroll_y - page).max(0),
                x if x == VK_NEXT => g.scroll_y = (g.scroll_y + 1).min(max),
                x if x == VK_HOME => g.scroll_y = 0,
                x if x == VK_END => g.scroll_y = max,
                _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
            update_scrollbars(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// 現在の表示アイテム総数(検証用)
pub fn item_count(hwnd: HWND) -> usize {
    state(hwnd).model.items.len()
}

/// 現在の列定義を返す(再構築時に列タイトルを維持するため)
pub fn model_columns(hwnd: HWND) -> Vec<crate::grid_model::ColumnSpec> {
    state(hwnd).model.columns.clone()
}
