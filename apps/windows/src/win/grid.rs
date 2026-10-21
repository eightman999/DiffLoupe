//! カスタム描画グリッド("DiffLoupeGrid")。テキスト差分(2列)・Hex(5列)・
//! 3-way(3列)を共通部品で描画する。行番号・行全体の配色・行内差分の強調・
//! 折りたたみヘッダ・縦横スクロール・キーボード操作を提供する。
//!
//! 列が全て weight>0 のとき(差分/3-way)は「分割表示」: 各列は表示幅を等分し、
//! 行番号は固定、本文だけを同じ量だけ横スクロールする。weight==0 の列を含む
//! とき(Hex)は「表表示」: 内容幅の列を並べ、表全体を横スクロールする。
//!
//! フォントは親が `WM_SETFONT` で渡す(所有しない)。描画は `paint_into` に集約し、
//! `WM_PAINT`(ダブルバッファ)・`WM_PRINTCLIENT`・スモーク用の直接描画で共有する。

use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::SetScrollInfo;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::grid_model::{CellTone, GridItem, GridModel};
use crate::win::dpi::scale;
use crate::win::theme::{self, Theme};
use crate::win::wide::wstr;

pub const CLASS_NAME: &str = "DiffLoupeGrid";
/// 折りたたみヘッダがクリックされた(親へ通知。wParam=group_id)
pub const WM_APP_GRID_FOLD: u32 = WM_APP + 40;
/// 通常行がクリックされた(親へ通知。wParam=source_id。Pro のハンク選択などに使う)
pub const WM_APP_GRID_CLICK: u32 = WM_APP + 41;

const TAB_SPACES: &str = "    ";

struct Grid {
    model: GridModel,
    /// 親が所有するフォント(未設定時は DEFAULT_GUI_FONT)
    font: HFONT,
    row_h: i32,
    hdr_h: i32,
    scroll_x: i32,
    scroll_y: i32, // 先頭可視行インデックス
    col_w: Vec<i32>,
    gutter_w: i32,
    /// 分割表示か(全列 weight>0)
    split: bool,
    /// 横スクロールの全体幅 / 1 ページ幅
    span_w: i32,
    page_w: i32,
    search_hits: HashSet<usize>,
    current_hit: Option<usize>,
    current_diff: Option<usize>,
    // 診断用
    paint_count: u32,
    last_rows: (i32, i32),
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

pub fn create(parent: HWND, x: i32, y: i32, w: i32, h: i32, _mono: bool, id: i32) -> HWND {
    let class = wstr(CLASS_NAME);
    let g = Box::new(Grid {
        model: GridModel {
            columns: vec![],
            items: vec![],
        },
        font: HFONT::default(),
        row_h: 18,
        hdr_h: 0,
        scroll_x: 0,
        scroll_y: 0,
        col_w: vec![],
        gutter_w: 0,
        split: true,
        span_w: 0,
        page_w: 1,
        search_hits: HashSet::new(),
        current_hit: None,
        current_diff: None,
        paint_count: 0,
        last_rows: (0, 0),
    });
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
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

fn cur_font(g: &Grid) -> HFONT {
    if g.font.is_invalid() {
        HFONT(unsafe { GetStockObject(DEFAULT_GUI_FONT) }.0)
    } else {
        g.font
    }
}

pub fn set_model(hwnd: HWND, model: GridModel, _mono: bool) {
    let g = state(hwnd);
    g.model = model;
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

fn client(hwnd: HWND) -> RECT {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    rc
}

fn page_rows(g: &Grid, rc: &RECT) -> i32 {
    ((rc.bottom - g.hdr_h) / g.row_h.max(1)).max(1)
}

fn max_scroll_y(g: &Grid, rc: &RECT) -> i32 {
    (g.model.items.len() as i32 - page_rows(g, rc)).max(0)
}

fn max_scroll_x(g: &Grid) -> i32 {
    (g.span_w - g.page_w).max(0)
}

fn scroll_to_row(hwnd: HWND, idx: usize) {
    let g = state(hwnd);
    let rc = client(hwnd);
    let visible = page_rows(g, &rc);
    let idx = idx as i32;
    if idx < g.scroll_y {
        g.scroll_y = idx;
    } else if idx >= g.scroll_y + visible {
        g.scroll_y = idx - visible + 1;
    }
    g.scroll_y = g.scroll_y.clamp(0, max_scroll_y(g, &rc));
    update_scrollbars(hwnd);
}

fn text_width(hdc: HDC, text: &[u16]) -> i32 {
    if text.is_empty() {
        return 0;
    }
    let mut size = SIZE::default();
    unsafe {
        let _ = GetTextExtentPoint32W(hdc, text, &mut size);
    }
    size.cx
}

/// 表示用に UTF-16 化(タブは空白へ展開)
fn display_wide(s: &str) -> Vec<u16> {
    if s.contains('\t') {
        s.replace('\t', TAB_SPACES).encode_utf16().collect()
    } else {
        s.encode_utf16().collect()
    }
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

fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(hdc, rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// 列幅・行番号幅・スクロール範囲を再計算する
fn relayout(hwnd: HWND) {
    let g = state(hwnd);
    let rc = client(hwnd);
    let view_w = rc.right;
    let ncols = g.model.columns.len();
    g.col_w = vec![0; ncols];
    g.split = ncols > 0 && g.model.columns.iter().all(|c| c.weight > 0.0);
    let has_title = g.model.columns.iter().any(|c| !c.title.is_empty());
    let hdc = unsafe { GetDC(Some(hwnd)) };
    let old = unsafe { SelectObject(hdc, HGDIOBJ(cur_font(g).0)) };
    let mut tm = TEXTMETRICW::default();
    unsafe {
        let _ = GetTextMetricsW(hdc, &mut tm);
    }
    g.row_h = tm.tmHeight + scale(hwnd, 4);
    g.hdr_h = if has_title {
        g.row_h + scale(hwnd, 2)
    } else {
        0
    };
    let pad = scale(hwnd, 6);

    // 行番号幅: 最大桁数(先頭 64 行ではなく全セルの最長文字列長で決める)
    let mut max_gutter_len = 0usize;
    for item in &g.model.items {
        if let GridItem::Row { cells, .. } = item {
            for c in cells {
                if let Some(t) = &c.gutter {
                    max_gutter_len = max_gutter_len.max(t.len());
                }
            }
        }
    }
    g.gutter_w = if max_gutter_len > 0 {
        let digits = "9".repeat(max_gutter_len.max(2));
        text_width(hdc, &display_wide(&digits)) + pad * 2
    } else {
        0
    };

    if g.split {
        // 表示幅を weight で等分。横スクロールは本文のみ。
        let total: f64 = g.model.columns.iter().map(|c| c.weight).sum();
        for (i, c) in g.model.columns.iter().enumerate() {
            g.col_w[i] = (view_w as f64 * c.weight / total) as i32;
        }
        // 文字数の多い上位セルだけを実測して最長幅を求める
        let mut max_text = 0;
        for ci in 0..ncols {
            let mut lens: Vec<(usize, &str)> = g
                .model
                .items
                .iter()
                .filter_map(|it| match it {
                    GridItem::Row { cells, .. } => cells.get(ci).map(|c| (c.text.len(), &*c.text)),
                    _ => None,
                })
                .collect();
            lens.sort_by_key(|(l, _)| std::cmp::Reverse(*l));
            for (_, t) in lens.into_iter().take(48) {
                max_text = max_text.max(text_width(hdc, &display_wide(t)));
            }
        }
        let col_min = g.col_w.iter().copied().min().unwrap_or(0);
        g.page_w = (col_min - g.gutter_w).max(1);
        g.span_w = (max_text + pad * 2).max(g.page_w);
    } else {
        // 表表示: 見出しと代表セルから内容幅を決める
        let mut fixed_total = 0;
        for (i, c) in g.model.columns.iter().enumerate() {
            let mut w = text_width(hdc, &display_wide(&c.title)) + pad * 2;
            for item in g.model.items.iter().take(256) {
                if let GridItem::Row { cells, .. } = item {
                    if let Some(cell) = cells.get(i) {
                        w = w.max(text_width(hdc, &display_wide(&cell.text)) + pad * 2);
                    }
                }
            }
            if c.weight > 0.0 {
                w = w.max(scale(hwnd, 120));
            }
            g.col_w[i] = w;
            fixed_total += w;
        }
        // 余白があれば最後の列を広げて行の背景を右端まで届かせる
        if fixed_total < view_w {
            if let Some(last) = g.col_w.last_mut() {
                *last += view_w - fixed_total;
                fixed_total = view_w;
            }
        }
        g.span_w = fixed_total;
        g.page_w = view_w.max(1);
    }
    unsafe {
        SelectObject(hdc, old);
        ReleaseDC(Some(hwnd), hdc);
    }
    g.scroll_y = g.scroll_y.clamp(0, max_scroll_y(g, &rc));
    g.scroll_x = g.scroll_x.clamp(0, max_scroll_x(g));
    update_scrollbars(hwnd);
}

fn update_scrollbars(hwnd: HWND) {
    let g = state(hwnd);
    let rc = client(hwnd);
    let nrows = g.model.items.len() as i32;
    let mut si = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: (nrows - 1).max(0),
        nPage: page_rows(g, &rc) as u32,
        nPos: g.scroll_y,
        ..Default::default()
    };
    unsafe {
        let _ = SetScrollInfo(hwnd, SB_VERT, &si, true);
        si.nMax = (g.span_w - 1).max(0);
        si.nPage = g.page_w.max(1) as u32;
        si.nPos = g.scroll_x;
        let _ = SetScrollInfo(hwnd, SB_HORZ, &si, true);
    }
}

/// 強調範囲を持つセグメントと通常セグメントに分けて描画する(返り値: 描画幅)
fn draw_highlighted_text(
    hdc: HDC,
    x: i32,
    y: i32,
    text: &str,
    hl: &[std::ops::Range<usize>],
    hl_col: COLORREF,
    line_h: i32,
) -> i32 {
    // 強調範囲は元文字列の UTF-16 位置。タブ展開はセグメント単位で行う。
    let orig: Vec<u16> = text.encode_utf16().collect();
    let mut cx = x;
    let mut pos = 0usize;
    let mut sorted: Vec<_> = hl.to_vec();
    sorted.sort_by_key(|r| r.start);
    let seg = |hdc: HDC, cx: i32, s: &[u16]| -> i32 {
        let text = String::from_utf16_lossy(s);
        let w = display_wide(&text);
        let width = text_width(hdc, &w);
        if !w.is_empty() {
            unsafe {
                let _ = TextOutW(hdc, cx, y, &w);
            }
        }
        width
    };
    for r in sorted {
        let (s, e) = (r.start.min(orig.len()), r.end.min(orig.len()));
        if s > pos {
            cx += seg(hdc, cx, &orig[pos..s]);
        }
        if e > s {
            let w = text_width(
                hdc,
                &display_wide(&String::from_utf16_lossy(&orig[s.max(pos)..e])),
            );
            fill(
                hdc,
                &RECT {
                    left: cx,
                    top: y,
                    right: cx + w,
                    bottom: y + line_h,
                },
                hl_col,
            );
            cx += seg(hdc, cx, &orig[s.max(pos)..e]);
        }
        pos = e.max(pos);
    }
    if pos < orig.len() {
        cx += seg(hdc, cx, &orig[pos..]);
    }
    cx - x
}

fn draw_label(hdc: HDC, text: &str, rect: &mut RECT, flags: DRAW_TEXT_FORMAT) {
    let mut w: Vec<u16> = text.encode_utf16().collect();
    if w.is_empty() {
        return;
    }
    unsafe {
        DrawTextW(hdc, &mut w, rect, flags | DT_SINGLELINE | DT_NOPREFIX);
    }
}

/// クライアント領域全体を `hdc`(クライアント座標)へ描く
fn paint_into(hwnd: HWND, hdc: HDC) {
    let g = state(hwnd);
    g.paint_count += 1;
    let theme = theme::cached();
    let rc = client(hwnd);
    fill(hdc, &rc, theme.window_bg);
    unsafe {
        let old_font = SelectObject(hdc, HGDIOBJ(cur_font(g).0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, theme.text);
        let pad = scale(hwnd, 6);
        let line_h = g.row_h - scale(hwnd, 2);
        let n = g.model.items.len() as i32;
        let first = g.scroll_y.max(0);
        let last = (first + (rc.bottom - g.hdr_h + g.row_h - 1) / g.row_h.max(1) + 1).min(n);
        g.last_rows = (first, last);

        // 列の開始 x
        let sx_table = if g.split { 0 } else { -g.scroll_x };
        let mut col_x = Vec::with_capacity(g.col_w.len());
        let mut x = sx_table;
        for w in &g.col_w {
            col_x.push(x);
            x += w;
        }

        for row_idx in first..last {
            let item = &g.model.items[row_idx as usize];
            let top = g.hdr_h + (row_idx - first) * g.row_h;
            let row_rect = RECT {
                left: 0,
                top,
                right: rc.right,
                bottom: top + g.row_h,
            };
            match item {
                GridItem::FoldHeader { group_id, count } => {
                    let _ = group_id;
                    fill(hdc, &row_rect, theme.info_bg);
                    SetTextColor(hdc, theme.dim_text);
                    let mut tr = row_rect;
                    draw_label(
                        hdc,
                        &format!("{} {}", count, crate::strings::FOLD_SHOW_LINES),
                        &mut tr,
                        DT_CENTER | DT_VCENTER,
                    );
                    SetTextColor(hdc, theme.text);
                }
                GridItem::Row { cells, source_id } => {
                    for (ci, cell) in cells.iter().enumerate() {
                        let Some(&cell_left) = col_x.get(ci) else {
                            continue;
                        };
                        let cell_right = cell_left + g.col_w[ci];
                        if cell_right <= 0 || cell_left >= rc.right {
                            continue;
                        }
                        let cell_rect = RECT {
                            left: cell_left.max(0),
                            top,
                            right: cell_right.min(rc.right),
                            bottom: top + g.row_h,
                        };
                        fill(hdc, &cell_rect, tone_color(&theme, cell.tone));
                        // 行番号
                        let has_gutter = cell.gutter.is_some() || (g.split && g.gutter_w > 0);
                        let gw = if has_gutter { g.gutter_w } else { 0 };
                        if let Some(gtxt) = cell.gutter.as_deref() {
                            let mut tr = RECT {
                                left: cell_left,
                                top,
                                right: cell_left + gw - pad,
                                bottom: top + g.row_h,
                            };
                            SetTextColor(hdc, theme.dim_text);
                            draw_label(hdc, gtxt, &mut tr, DT_RIGHT | DT_VCENTER);
                        }
                        // 本文(列内にクリップ)
                        let text_left = cell_left + gw;
                        let off = if g.split { -g.scroll_x } else { 0 };
                        let saved = SaveDC(hdc);
                        IntersectClipRect(
                            hdc,
                            text_left.max(0),
                            top,
                            cell_right.min(rc.right),
                            top + g.row_h,
                        );
                        SetTextColor(hdc, theme.text);
                        let ty = top + (g.row_h - line_h) / 2;
                        draw_highlighted_text(
                            hdc,
                            text_left + pad + off,
                            ty,
                            &cell.text,
                            &cell.highlights,
                            hl_color(&theme, cell.tone),
                            line_h,
                        );
                        let _ = RestoreDC(hdc, saved);
                        // 列区切り線
                        if ci > 0 {
                            fill(
                                hdc,
                                &RECT {
                                    left: cell_left,
                                    top,
                                    right: cell_left + 1,
                                    bottom: top + g.row_h,
                                },
                                theme.border,
                            );
                        }
                    }
                    // 検索ヒット印: 行右端の短い帯
                    if g.search_hits.contains(source_id) {
                        let col = if g.current_hit == Some(*source_id) {
                            theme.current_hit
                        } else {
                            theme.search_hit
                        };
                        fill(
                            hdc,
                            &RECT {
                                left: rc.right - scale(hwnd, 4),
                                top,
                                right: rc.right,
                                bottom: top + g.row_h,
                            },
                            col,
                        );
                    }
                    if g.current_diff == Some(*source_id) {
                        fill(
                            hdc,
                            &RECT {
                                left: 0,
                                top,
                                right: scale(hwnd, 3),
                                bottom: top + g.row_h,
                            },
                            theme.accent,
                        );
                    }
                }
            }
        }

        // 見出し行(縦スクロールしない)
        if g.hdr_h > 0 {
            let hr = RECT {
                left: 0,
                top: 0,
                right: rc.right,
                bottom: g.hdr_h,
            };
            fill(hdc, &hr, theme.info_bg);
            SetTextColor(hdc, theme.dim_text);
            for (ci, col) in g.model.columns.iter().enumerate() {
                let Some(&cl) = col_x.get(ci) else { continue };
                let mut tr = RECT {
                    left: cl + pad,
                    top: 0,
                    right: cl + g.col_w[ci] - pad,
                    bottom: g.hdr_h,
                };
                draw_label(
                    hdc,
                    &col.title,
                    &mut tr,
                    DT_LEFT | DT_VCENTER | DT_END_ELLIPSIS,
                );
                if ci > 0 {
                    fill(
                        hdc,
                        &RECT {
                            left: cl,
                            top: 0,
                            right: cl + 1,
                            bottom: g.hdr_h,
                        },
                        theme.border,
                    );
                }
            }
            fill(
                hdc,
                &RECT {
                    left: 0,
                    top: g.hdr_h - 1,
                    right: rc.right,
                    bottom: g.hdr_h,
                },
                theme.border,
            );
        }
        SelectObject(hdc, old_font);
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    if hdc.is_invalid() {
        // BeginPaint に失敗しても必ず EndPaint して再描画ループを避ける
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        return;
    }
    unsafe {
        let rc = client(hwnd);
        let (w, h) = (rc.right.max(1), rc.bottom.max(1));
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        if mem.is_invalid() || bmp.is_invalid() {
            paint_into(hwnd, hdc);
        } else {
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
        }
        if !bmp.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(bmp.0));
        }
        if !mem.is_invalid() {
            let _ = DeleteDC(mem);
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    super::guard::run("grid", hwnd, msg, wparam, lparam, true, || unsafe {
        wndproc_inner(hwnd, msg, wparam, lparam)
    })
}

fn redraw(hwnd: HWND) {
    update_scrollbars(hwnd);
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

unsafe fn wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            relayout(hwnd);
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
            // 親が NULL を渡しても(フォント生成前の呼び出し)現在のフォントを保つ
            if wparam.0 != 0 {
                state(hwnd).font = HFONT(wparam.0 as *mut _);
            }
            relayout(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_GETDLGCODE => LRESULT((DLGC_WANTARROWS | DLGC_WANTCHARS) as isize),
        WM_SIZE => {
            relayout(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_PRINTCLIENT => {
            // PrintWindow / WM_PRINT からのキャプチャ要求: 渡された DC へ直接描く
            paint_into(hwnd, HDC(wparam.0 as *mut _));
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
            let lines = delta / WHEEL_DELTA as i32 * 3;
            let g = state(hwnd);
            let rc = client(hwnd);
            g.scroll_y = (g.scroll_y - lines).clamp(0, max_scroll_y(g, &rc));
            redraw(hwnd);
            LRESULT(0)
        }
        WM_MOUSEHWHEEL => {
            let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
            let g = state(hwnd);
            g.scroll_x = (g.scroll_x + delta / WHEEL_DELTA as i32 * 48).clamp(0, max_scroll_x(g));
            redraw(hwnd);
            LRESULT(0)
        }
        WM_VSCROLL => {
            let g = state(hwnd);
            let rc = client(hwnd);
            let max = max_scroll_y(g, &rc);
            let page = page_rows(g, &rc);
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
            redraw(hwnd);
            LRESULT(0)
        }
        WM_HSCROLL => {
            let g = state(hwnd);
            let max = max_scroll_x(g);
            let page = g.page_w.max(1);
            let step = scale(hwnd, 24);
            let code = (wparam.0 & 0xFFFF) as u16;
            g.scroll_x = match code {
                x if x == SB_LINELEFT.0 as u16 => g.scroll_x - step,
                x if x == SB_LINERIGHT.0 as u16 => g.scroll_x + step,
                x if x == SB_PAGELEFT.0 as u16 => g.scroll_x - page,
                x if x == SB_PAGERIGHT.0 as u16 => g.scroll_x + page,
                x if x == SB_LEFT.0 as u16 => 0,
                x if x == SB_RIGHT.0 as u16 => max,
                x if x == SB_THUMBPOSITION.0 as u16 || x == SB_THUMBTRACK.0 as u16 => {
                    ((wparam.0 >> 16) & 0xFFFF) as i32
                }
                _ => g.scroll_x,
            };
            g.scroll_x = g.scroll_x.clamp(0, max);
            redraw(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = (lparam.0 >> 16) as i32 as i16 as i32; // GET_Y_LPARAM
            let g = state(hwnd);
            if y >= g.hdr_h {
                let row = g.scroll_y + (y - g.hdr_h) / g.row_h.max(1);
                match g.model.items.get(row.max(0) as usize) {
                    Some(GridItem::FoldHeader { group_id, .. }) => unsafe {
                        let parent = GetParent(hwnd).unwrap_or_default();
                        let _ = SendMessageW(
                            parent,
                            WM_APP_GRID_FOLD,
                            Some(WPARAM(*group_id)),
                            Some(LPARAM(0)),
                        );
                    },
                    Some(GridItem::Row { source_id, .. }) => unsafe {
                        let parent = GetParent(hwnd).unwrap_or_default();
                        let _ = SendMessageW(
                            parent,
                            WM_APP_GRID_CLICK,
                            Some(WPARAM(*source_id)),
                            Some(LPARAM(0)),
                        );
                    },
                    _ => {}
                }
            }
            unsafe {
                let _ = SetFocus(Some(hwnd));
            };
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let g = state(hwnd);
            let rc = client(hwnd);
            let max = max_scroll_y(g, &rc);
            let page = page_rows(g, &rc);
            let step = scale(hwnd, 24);
            use windows::Win32::UI::Input::KeyboardAndMouse::*;
            match VIRTUAL_KEY(wparam.0 as u16) {
                x if x == VK_UP => g.scroll_y = (g.scroll_y - 1).max(0),
                x if x == VK_DOWN => g.scroll_y = (g.scroll_y + 1).min(max),
                x if x == VK_PRIOR => g.scroll_y = (g.scroll_y - page).max(0),
                x if x == VK_NEXT => g.scroll_y = (g.scroll_y + page).min(max),
                x if x == VK_HOME => g.scroll_y = 0,
                x if x == VK_END => g.scroll_y = max,
                x if x == VK_LEFT => g.scroll_x = (g.scroll_x - step).max(0),
                x if x == VK_RIGHT => g.scroll_x = (g.scroll_x + step).min(max_scroll_x(g)),
                _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
            redraw(hwnd);
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

/// スモーク診断用: 行数・列数・可視範囲などをまとめた 1 行テキスト。
/// 「データが空」「描画されていない」「キャプチャ側の問題」の切り分けに使う。
pub fn debug_info(hwnd: HWND) -> String {
    let g = state(hwnd);
    let rc = client(hwnd);
    let first_text = g
        .model
        .items
        .iter()
        .find_map(|i| match i {
            GridItem::Row { cells, .. } => cells.first().map(|c| c.text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    format!(
        "grid: items={} cols={} col_w={:?} client={}x{} visible={} hdr_h={} row_h={} \
         gutter_w={} split={} scroll=({},{}) span_w={} page_w={} font_set={} \
         paint_count={} last_rows={:?} first_cell={:?}",
        g.model.items.len(),
        g.model.columns.len(),
        g.col_w,
        rc.right,
        rc.bottom,
        unsafe { IsWindowVisible(hwnd) }.as_bool(),
        g.hdr_h,
        g.row_h,
        g.gutter_w,
        g.split,
        g.scroll_x,
        g.scroll_y,
        g.span_w,
        g.page_w,
        !g.font.is_invalid(),
        g.paint_count,
        g.last_rows,
        first_text.chars().take(40).collect::<String>(),
    )
}

/// スモーク用: グリッドをメモリ DC(32bpp DIB)へ直接描き、(BGRA, 幅, 高さ) を返す。
/// PrintWindow が子ウィンドウを取りこぼす場合でも実表示と同じ内容を確認できる。
pub fn render_to_bgra(hwnd: HWND) -> Option<(Vec<u8>, i32, i32)> {
    let rc = client(hwnd);
    let (w, h) = (rc.right, rc.bottom);
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let mem = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteDC(mem);
            return None;
        }
        let old = SelectObject(mem, HGDIOBJ(bmp.0));
        paint_into(hwnd, mem);
        let px = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        Some((px, w, h))
    }
}
