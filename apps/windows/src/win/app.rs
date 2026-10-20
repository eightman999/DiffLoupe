//! メインウィンドウ。フォルダ選択バー・結果ツリー・詳細ペイン・ステータスバー・
//! メニュー・D&D・比較スレッド・フォルダ監視を統括する。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{
    SS_CENTER, SS_CENTERIMAGE, SS_LEFT, SS_LEFTNOWORDWRAP,
};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, HDROP,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::appmodel::*;
use crate::detail::{self, DetailContent, DiffNav};
use crate::grid_model;
use crate::settings::WindowFrame;
use crate::viewmodel::{self, ResultFilter};
use crate::win::{dialogs, grid, imagepane, ops, picker, theme, wide::wstr};

pub const WM_APP_COMPARE_PROGRESS: u32 = WM_APP + 1;
pub const WM_APP_COMPARE_DONE: u32 = WM_APP + 2;
pub const WM_APP_WATCH_CHANGED: u32 = WM_APP + 3;
pub const WM_APP_DETAIL_LOADED: u32 = WM_APP + 4;
pub const WM_APP_SETTINGS_CHANGED: u32 = WM_APP + 5;

const MAIN_CLASS: &str = "DiffLoupeMainWindow";

// 子コントロール ID
mod id {
    pub const BTN_LEFT: i32 = 1;
    pub const EDIT_LEFT: i32 = 2;
    pub const CMB_LREF: i32 = 3;
    pub const BTN_RIGHT: i32 = 4;
    pub const EDIT_RIGHT: i32 = 5;
    pub const CMB_RREF: i32 = 6;
    pub const BTN_SWAP: i32 = 7;
    pub const CMB_HISTORY: i32 = 8;
    pub const BTN_3WAY: i32 = 9;
    pub const BTN_SYNC: i32 = 10;
    pub const BTN_REPORT: i32 = 11;
    pub const BTN_SETTINGS: i32 = 12;
    pub const BTN_COMPARE: i32 = 13;
    pub const EDIT_SEARCH: i32 = 14;
    pub const CMB_FILTER: i32 = 15;
    pub const TREE: i32 = 16;
    pub const HDR1: i32 = 17;
    pub const HDR2: i32 = 18;
    pub const BTN_PDIFF: i32 = 19;
    pub const BTN_NDIFF: i32 = 20;
    pub const LBL_POS: i32 = 21;
    pub const EDIT_DSEARCH: i32 = 22;
    pub const BTN_PHIT: i32 = 23;
    pub const BTN_NHIT: i32 = 24;
    pub const CHK_CASE: i32 = 25;
    pub const CHK_ALL: i32 = 26;
    pub const GRID: i32 = 27;
    pub const MSG: i32 = 28;
    pub const STATUS: i32 = 29;
    pub const PROGRESS: i32 = 30;
    pub const IMGPANE: i32 = 31;
}

// メニューコマンド
mod cmd {
    pub const COMPARE: u16 = 1001;
    pub const SWAP: u16 = 1002;
    pub const SETTINGS: u16 = 1003;
    pub const EXIT: u16 = 1004;
    pub const REP_HTML: u16 = 1005;
    pub const REP_CSV: u16 = 1006;
    pub const REP_JSON: u16 = 1007;
    pub const REP_MD: u16 = 1008;
    pub const SYNC_L2R: u16 = 1009;
    pub const SYNC_R2L: u16 = 1010;
    pub const THREEWAY: u16 = 1011;
    pub const ABOUT: u16 = 1012;
    pub const PREV_DIFF: u16 = 1013;
    pub const NEXT_DIFF: u16 = 1014;
    pub const NEXT_HIT: u16 = 1015;
    pub const PREV_HIT: u16 = 1016;
}

// コンテキストメニュー
mod ctx {
    pub const OPEN_L: u16 = 1101;
    pub const REVEAL_L: u16 = 1102;
    pub const COPY_L: u16 = 1103;
    pub const OPEN_R: u16 = 1104;
    pub const REVEAL_R: u16 = 1105;
    pub const COPY_R: u16 = 1106;
}

struct NodeInfo {
    id: String,
    status: diffloupe_core::DiffStatus,
    is_dir: bool,
}

pub struct App {
    pub model: AppModel,
    pub hwnd: HWND,
    pub font: HFONT,
    pub mono_font: HFONT,
    // ヘッダ
    h_btn_left: HWND,
    h_edit_left: HWND,
    h_cmb_lref: HWND,
    h_btn_right: HWND,
    h_edit_right: HWND,
    h_cmb_rref: HWND,
    h_btn_swap: HWND,
    h_cmb_history: HWND,
    h_btn_3way: HWND,
    h_btn_sync: HWND,
    h_btn_report: HWND,
    h_btn_settings: HWND,
    h_btn_compare: HWND,
    // フィルタ行
    h_edit_search: HWND,
    h_cmb_filter: HWND,
    // コンテンツ
    h_tree: HWND,
    h_grid: HWND,
    h_msg: HWND,
    h_imgpane: HWND,
    // 詳細ヘッダ/ツールバー
    h_hdr1: HWND,
    h_hdr2: HWND,
    h_pdiff: HWND,
    h_ndiff: HWND,
    h_pos: HWND,
    h_dsearch: HWND,
    h_phit: HWND,
    h_nhit: HWND,
    h_case: HWND,
    h_all: HWND,
    // ステータス
    h_status: HWND,
    h_progress: HWND,
    nodes: Vec<NodeInfo>,
    detail: Option<DetailContent>,
    nav: DiffNav,
    detail_gen: u64,
    cancel: Option<Arc<diffloupe_core::CancellationToken>>,
    watcher: Option<diffloupe_core::watcher::DirectoryWatcher>,
    split_ratio: f64, // ツリー側の幅比率
    settings_hwnd: HWND,
    sync_hwnd: HWND,
    tw_hwnd: HWND,
    /// スモークテストモード(将来の挙動切替用に保持)
    _is_smoke: bool,
}

/// 文字列フォント(Yu Gothic UI 系)を作る
pub fn make_font(hwnd: HWND, mono: bool) -> HFONT {
    let dpi = unsafe { GetDpiForWindow(hwnd) } as i32;
    let size = -(dpi * 9 / 72);
    let face = if mono { "MS Gothic" } else { "Yu Gothic UI" };
    let mut lf = LOGFONTW {
        lfHeight: size,
        lfWeight: 400,
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    let mut fw = wstr(face);
    fw.truncate(LF_FACESIZE as usize - 1);
    lf.lfFaceName[..fw.len() - 1].copy_from_slice(&fw[..fw.len() - 1]);
    unsafe { CreateFontIndirectW(&lf) }
}

fn get_edit(hwnd: HWND) -> String {
    dialogs::get_edit_text(hwnd)
}

impl App {
    fn sc(&self, v: i32) -> i32 {
        crate::win::dpi::scale(self.hwnd, v)
    }

    fn set_text(&self, hwnd: HWND, text: &str) {
        unsafe {
            let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wstr(text).as_ptr()));
        }
    }

    fn enable(&self, hwnd: HWND, on: bool) {
        unsafe {
            let _ = EnableWindow(hwnd, on);
        }
    }

    fn show(&self, hwnd: HWND, on: bool) {
        unsafe {
            let _ = ShowWindow(hwnd, if on { SW_SHOW } else { SW_HIDE });
        }
    }

    // ---------- レイアウト ----------
    fn layout(&self) {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        let w = rc.right;
        let h = rc.bottom;
        let pad = self.sc(8);
        let row = self.sc(24);
        let mv = |c: HWND, x: i32, y: i32, w: i32, hh: i32| unsafe {
            let _ = MoveWindow(c, x, y, w, hh, true);
        };
        // 1行目: 左フォルダ / 参照combo / 交換 / 右
        let btnw = self.sc(96);
        let combow = self.sc(150);
        let swapw = self.sc(70);
        let mut y = pad;
        mv(self.h_btn_left, pad, y, btnw, row);
        let edit_w = (w - pad * 2 - btnw * 2 - combow * 2 - swapw - self.sc(24)) / 2;
        mv(self.h_edit_left, pad + btnw + 4, y, edit_w, row);
        let has_lrepo = self.model.left_git_repo.is_some();
        if has_lrepo {
            mv(
                self.h_cmb_lref,
                pad + btnw + 4 + edit_w + 4,
                y,
                combow,
                self.sc(200),
            );
        }
        self.show(self.h_cmb_lref, has_lrepo);
        mv(self.h_btn_swap, w / 2 - swapw / 2, y + row + 4, swapw, row);
        y += row + 8;
        mv(self.h_btn_right, pad, y, btnw, row);
        mv(self.h_edit_right, pad + btnw + 4, y, edit_w, row);
        let has_rrepo = self.model.right_git_repo.is_some();
        if has_rrepo {
            mv(
                self.h_cmb_rref,
                pad + btnw + 4 + edit_w + 4,
                y,
                combow,
                self.sc(200),
            );
        }
        self.show(self.h_cmb_rref, has_rrepo);
        y += row + pad;
        // 2行目: 履歴 / 3-way / コピー計画 / レポート / 設定 / 比較
        let mut x = pad;
        let bw = [
            self.sc(90),
            self.sc(96),
            self.sc(96),
            self.sc(110),
            self.sc(72),
            self.sc(96),
        ];
        for (i, c) in [
            self.h_cmb_history,
            self.h_btn_3way,
            self.h_btn_sync,
            self.h_btn_report,
            self.h_btn_settings,
            self.h_btn_compare,
        ]
        .iter()
        .enumerate()
        {
            mv(*c, x, y, bw[i], if i == 0 { self.sc(200) } else { row });
            x += bw[i] + 6;
        }
        y += row + pad;
        // フィルタ行
        mv(self.h_edit_search, pad, y, self.sc(240), row);
        mv(
            self.h_cmb_filter,
            pad + self.sc(240) + 6,
            y,
            self.sc(120),
            self.sc(200),
        );
        y += row + 6;
        // コンテンツ
        let status_h = self.sc(26);
        let content_h = h - y - status_h;
        let tree_w = (w as f64 * self.split_ratio) as i32;
        mv(self.h_tree, 0, y, tree_w, content_h);
        // 詳細ペイン
        let dx = tree_w + self.sc(3);
        let dw = (w - dx).max(0);
        let hdr_h = self.sc(44);
        mv(self.h_hdr1, dx + pad, y + 2, dw - pad * 2, self.sc(18));
        mv(
            self.h_hdr2,
            dx + pad,
            y + self.sc(20),
            dw - pad * 2,
            self.sc(22),
        );
        let tb = y + hdr_h;
        let mut tx = dx + pad;
        mv(self.h_pdiff, tx, tb, self.sc(24), row);
        tx += self.sc(28);
        mv(self.h_ndiff, tx, tb, self.sc(24), row);
        tx += self.sc(28);
        mv(self.h_pos, tx, tb + 4, self.sc(56), self.sc(16));
        tx += self.sc(60);
        mv(self.h_dsearch, tx, tb, self.sc(160), row);
        tx += self.sc(164);
        mv(self.h_phit, tx, tb, self.sc(24), row);
        tx += self.sc(28);
        mv(self.h_nhit, tx, tb, self.sc(24), row);
        tx += self.sc(30);
        mv(self.h_case, tx, tb, self.sc(120), row);
        tx += self.sc(124);
        mv(self.h_all, tx, tb, self.sc(100), row);
        let content_y = tb + row + 4;
        let content_h2 = h - content_y - status_h;
        mv(self.h_grid, dx, content_y, dw, content_h2);
        mv(self.h_imgpane, dx, content_y, dw, content_h2);
        mv(self.h_msg, dx + pad, content_y, dw - pad * 2, content_h2);
        // ステータスバー
        mv(
            self.h_status,
            pad,
            h - status_h + 2,
            w - pad * 2 - self.sc(170),
            row,
        );
        mv(
            self.h_progress,
            w - pad - self.sc(160),
            h - status_h + 4,
            self.sc(160),
            self.sc(16),
        );
    }

    // ---------- 状態反映 ----------
    fn refresh_chrome(&self) {
        let running = self.model.phase.is_running();
        self.enable(self.h_btn_left, !running);
        self.enable(self.h_btn_right, !running);
        self.enable(self.h_btn_swap, !running);
        self.enable(self.h_cmb_history, !running);
        self.enable(self.h_btn_3way, !running);
        self.enable(self.h_btn_sync, self.model.result.is_some());
        self.enable(self.h_btn_report, self.model.result.is_some());
        self.enable(self.h_btn_compare, self.model.can_compare() || running);
        self.set_text(
            self.h_btn_compare,
            if running {
                crate::strings::CANCEL
            } else {
                crate::strings::COMPARE
            },
        );
        self.show(self.h_progress, running);
        // フォルダ表示
        let lp = self
            .model
            .left_folder
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rp = self
            .model
            .right_folder
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.set_text(self.h_edit_left, &lp);
        self.set_text(self.h_edit_right, &rp);
    }

    fn refresh_status(&self) {
        let s = match self.model.phase {
            Phase::Scanning { found } => format!(
                "{} {} {}",
                crate::strings::STATUS_SCANNING,
                found,
                crate::strings::ITEMS_UNIT
            ),
            Phase::Comparing { done, total } => {
                format!("{} {}/{}", crate::strings::STATUS_COMPARING, done, total)
            }
            Phase::Renaming { done, total } => {
                format!("{} {}/{}", crate::strings::STATUS_RENAMING, done, total)
            }
            Phase::Idle => {
                if let Some(e) = &self.model.error_message {
                    e.clone()
                } else if let Some(r) = &self.model.result {
                    let mut parts: Vec<String> = viewmodel::summary_labels(&r.summary)
                        .iter()
                        .map(|(l, c)| format!("{} {}", l, c))
                        .collect();
                    parts.push(format!(
                        "{} {} {}",
                        crate::strings::TOTAL_FILES,
                        r.summary.total_files(),
                        crate::strings::FILES_UNIT
                    ));
                    parts.join("   ")
                } else {
                    crate::strings::STATUS_PROMPT.into()
                }
            }
        };
        let watch = if self.watcher.is_some() {
            format!("   |  {}", crate::strings::STATUS_WATCHING)
        } else if self.model.auto_refresh_paused_reason.is_some() {
            format!("   |  {}", crate::strings::STATUS_PAUSED)
        } else {
            String::new()
        };
        self.set_text(self.h_status, &format!("{}{}", s, watch));
        unsafe {
            let (done, total) = match self.model.phase {
                Phase::Comparing { done, total } => (done, total.max(1)),
                Phase::Renaming { done, total } => (done, total.max(1)),
                _ => (0, 100),
            };
            SendMessageW(
                self.h_progress,
                PBM_SETRANGE32,
                Some(WPARAM(0)),
                Some(LPARAM(total as isize)),
            );
            SendMessageW(
                self.h_progress,
                PBM_SETPOS,
                Some(WPARAM(done)),
                Some(LPARAM(0)),
            );
        }
    }

    fn refresh_history(&self) {
        unsafe {
            while SendMessageW(
                self.h_cmb_history,
                CB_DELETESTRING,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0 >= 0
            {}
            let t = wstr(crate::strings::HISTORY);
            SendMessageW(
                self.h_cmb_history,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(t.as_ptr() as isize)),
            );
            for item in &self.model.settings.recent_comparisons {
                let ln = PathBuf::from(&item.left_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| item.left_path.clone());
                let rn = PathBuf::from(&item.right_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| item.right_path.clone());
                let ls = item
                    .left_git_ref
                    .as_ref()
                    .map(|r| format!(" @ {}", r))
                    .unwrap_or_default();
                let rs = item
                    .right_git_ref
                    .as_ref()
                    .map(|r| format!(" @ {}", r))
                    .unwrap_or_default();
                let label = format!("{}{} ⇄ {}{}", ln, ls, rn, rs);
                let w = wstr(&label);
                SendMessageW(
                    self.h_cmb_history,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(w.as_ptr() as isize)),
                );
            }
            SendMessageW(
                self.h_cmb_history,
                CB_SETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }
    }

    fn refresh_ref_combo(&self, side: Side) {
        let (h, repo, cur) = match side {
            Side::Left => (
                self.h_cmb_lref,
                &self.model.left_git_repo,
                &self.model.left_git_ref,
            ),
            Side::Right => (
                self.h_cmb_rref,
                &self.model.right_git_repo,
                &self.model.right_git_ref,
            ),
        };
        unsafe {
            while SendMessageW(h, CB_DELETESTRING, Some(WPARAM(0)), Some(LPARAM(0))).0 >= 0 {}
            let Some(repo) = repo else { return };
            let wt = wstr(crate::strings::WORKING_TREE);
            SendMessageW(
                h,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wt.as_ptr() as isize)),
            );
            for r in repo.list_refs() {
                let w = wstr(&r);
                SendMessageW(
                    h,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(w.as_ptr() as isize)),
                );
            }
            let ct = wstr(crate::strings::CUSTOM_REF);
            SendMessageW(
                h,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(ct.as_ptr() as isize)),
            );
            let sel = cur
                .as_ref()
                .map(|c| {
                    // 一致する項目の index を探す
                    let n = SendMessageW(h, CB_GETCOUNT, Some(WPARAM(0)), Some(LPARAM(0))).0;
                    (1..n)
                        .find(|&i| {
                            let mut buf = vec![0u16; 512];
                            SendMessageW(
                                h,
                                CB_GETLBTEXT,
                                Some(WPARAM(i as usize)),
                                Some(LPARAM(buf.as_mut_ptr() as isize)),
                            );
                            let end = buf.iter().position(|&c| c == 0).unwrap_or(0);
                            String::from_utf16_lossy(&buf[..end]) == *c
                        })
                        .unwrap_or(n - 1) // 未知なら「任意のref」位置
                })
                .unwrap_or(0);
            SendMessageW(h, CB_SETCURSEL, Some(WPARAM(sel as usize)), Some(LPARAM(0)));
        }
    }

    fn rebuild_tree(&self) {
        unsafe {
            SendMessageW(
                self.h_tree,
                TVM_DELETEITEM,
                Some(WPARAM(0)),
                Some(LPARAM(TVI_ROOT.0)),
            );
            SendMessageW(self.h_tree, WM_SETREDRAW, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        app.nodes.clear();
        let nodes = self.model.display_nodes();
        fn insert(app: &App, parent: HTREEITEM, node: &diffloupe_core::DiffNode) {
            let idx = {
                let a = unsafe { &mut *(GetWindowLongPtrW(app.hwnd, GWLP_USERDATA) as *mut App) };
                a.nodes.push(NodeInfo {
                    id: node.id.clone(),
                    status: node.status,
                    is_dir: node.is_directory,
                });
                a.nodes.len() - 1
            };
            let mut text = wstr(&node.name);
            let item = TVINSERTSTRUCTW {
                hParent: parent,
                hInsertAfter: HTREEITEM(TVI_LAST.0),
                Anonymous: TVINSERTSTRUCTW_0 {
                    itemex: TVITEMEXW {
                        mask: TVIF_TEXT | TVIF_PARAM,
                        pszText: PWSTR(text.as_mut_ptr()),
                        cchTextMax: text.len() as i32 - 1,
                        lParam: LPARAM(idx as isize),
                        ..Default::default()
                    },
                },
            };
            let hitem: HTREEITEM = unsafe {
                HTREEITEM(
                    SendMessageW(
                        app.h_tree,
                        TVM_INSERTITEMW,
                        Some(WPARAM(0)),
                        Some(LPARAM(&item as *const _ as isize)),
                    )
                    .0,
                )
            };
            if let Some(children) = &node.children {
                for c in children {
                    insert(app, hitem, c);
                }
            }
            // 子を展開
            unsafe {
                SendMessageW(
                    app.h_tree,
                    TVM_EXPAND,
                    Some(WPARAM(TVE_EXPAND.0 as usize)),
                    Some(LPARAM(hitem.0 as isize)),
                );
            }
        }
        for n in &nodes {
            insert(self, HTREEITEM(TVI_ROOT.0), n);
        }
        unsafe {
            SendMessageW(self.h_tree, WM_SETREDRAW, Some(WPARAM(1)), Some(LPARAM(0)));
            let _ = InvalidateRect(Some(self.h_tree), None, true);
        }
    }

    // ---------- 詳細ペイン ----------
    fn load_detail(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        app.detail_gen += 1;
        let gen = app.detail_gen;
        app.nav.reset();
        let entry = self.model.selected_entry().cloned();
        let cc = self.model.settings.content_comparison();
        // HWND は Send でないため生ポインタを usize として持ち回る
        let hwnd = self.hwnd.0 as usize;
        match entry {
            None => {
                app.detail = Some(DetailContent::Empty);
                self.show_detail_content();
            }
            Some(e) => {
                // 軽い即時判定(メッセージ系)は同期、その他はワーカー
                app.detail = Some(DetailContent::Message(
                    crate::strings::DETAIL_LOADING.into(),
                ));
                self.show_detail_content();
                std::thread::spawn(move || {
                    let d = detail::compute_detail(&e, &cc);
                    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
                    unsafe {
                        let _ = PostMessageW(
                            Some(hwnd),
                            WM_APP_DETAIL_LOADED,
                            WPARAM(gen as usize),
                            LPARAM(Box::into_raw(Box::new(d)) as isize),
                        );
                    }
                });
            }
        }
    }

    fn show_detail_content(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let entry = self.model.selected_entry();
        // ヘッダ
        let (h1, h2) = match (entry, &app.detail) {
            (Some(e), _) => {
                let badge = viewmodel::status_badge_label(e.status, e.is_directory);
                let mut l1 = format!("{}    [{}]", e.relative_path, badge);
                if let Some(rc) = &e.rename_candidate {
                    let tag = if rc.is_unique_match {
                        crate::strings::RENAME_UNIQUE
                    } else {
                        crate::strings::RENAME_CANDIDATE
                    };
                    l1.push_str(&format!(
                        "   {} → {}{}",
                        tag,
                        rc.counterpart_path,
                        if rc.is_unique_match {
                            String::new()
                        } else {
                            format!(" (同内容 {}件)", rc.total_matches)
                        }
                    ));
                }
                let mut l2 = format!(
                    "{}  左:{} 右:{}    {}  左:{} 右:{}    {}  左:{} 右:{}",
                    crate::strings::META_SIZE,
                    detail::size_text(e.left_size),
                    detail::size_text(e.right_size),
                    crate::strings::META_MTIME,
                    detail::date_text_utc(e.left_modification_date),
                    detail::date_text_utc(e.right_modification_date),
                    crate::strings::META_PERM,
                    detail::posix_text(e.left_posix),
                    detail::posix_text(e.right_posix),
                );
                if let Some(DetailContent::Diff(d)) = &app.detail {
                    l2.push_str(&format!(
                        "    左: {}  /  右: {}",
                        d.left_encoding, d.right_encoding
                    ));
                    if let Some(si) = &d.structured_info {
                        l2.push_str(&format!("    [{}]", si.badge_text()));
                    }
                }
                (l1, l2)
            }
            (None, _) => (crate::strings::DETAIL_PLACEHOLDER.into(), String::new()),
        };
        self.set_text(self.h_hdr1, &h1);
        self.set_text(self.h_hdr2, &h2);

        // コンテンツ切り替え
        self.show(self.h_grid, false);
        self.show(self.h_imgpane, false);
        self.show(self.h_msg, false);
        let toolbar_show = matches!(app.detail, Some(DetailContent::Diff(_)));
        for c in [
            self.h_pdiff,
            self.h_ndiff,
            self.h_pos,
            self.h_dsearch,
            self.h_phit,
            self.h_nhit,
            self.h_case,
            self.h_all,
        ] {
            self.show(c, toolbar_show);
        }
        match &app.detail {
            Some(DetailContent::Diff(d)) => {
                let items = app.nav.display_items(&d.rows);
                let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                grid::set_model(
                    self.h_grid,
                    grid_model::GridModel {
                        columns: grid_model::diff_columns(
                            self.model
                                .display_label(Side::Left)
                                .as_deref()
                                .unwrap_or(crate::strings::COL_LEFT),
                            self.model
                                .display_label(Side::Right)
                                .as_deref()
                                .unwrap_or(crate::strings::COL_RIGHT),
                        ),
                        items: grid_items,
                    },
                    true,
                );
                self.update_marks();
                self.show(self.h_grid, true);
            }
            Some(DetailContent::Hex { left, right }) => {
                let comp = diffloupe_core::hex_diff::compare_urls(
                    left,
                    right,
                    diffloupe_core::hex_diff::DEFAULT_BYTE_CAP,
                );
                match comp {
                    Ok(c) => {
                        grid::set_model(
                            self.h_grid,
                            grid_model::GridModel {
                                columns: grid_model::hex_columns(),
                                items: grid_model::build_hex_items(&c),
                            },
                            true,
                        );
                        if c.truncated {
                            self.set_text(
                                self.h_hdr2,
                                &format!("{}\n{}", h2, crate::strings::HEX_TRUNCATED),
                            );
                        }
                        self.show(self.h_grid, true);
                    }
                    Err(e) => {
                        self.set_text(self.h_msg, &format!("読み込みエラー: {}", e));
                        self.show(self.h_msg, true);
                    }
                }
            }
            Some(DetailContent::Image { left, right }) => {
                unsafe {
                    if !app.h_imgpane.is_invalid() {
                        let _ = DestroyWindow(app.h_imgpane);
                    }
                }
                app.h_imgpane =
                    imagepane::create(self.hwnd, id::IMGPANE, left.clone(), right.clone());
                self.layout();
                self.show(self.h_imgpane, true);
            }
            Some(DetailContent::Message(m)) => {
                self.set_text(self.h_msg, m);
                self.show(self.h_msg, true);
            }
            Some(DetailContent::Empty) | None => {
                self.set_text(self.h_msg, crate::strings::DETAIL_PLACEHOLDER);
                self.show(self.h_msg, true);
            }
        }
    }

    fn update_marks(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        if let Some(DetailContent::Diff(d)) = &app.detail {
            let hits: HashSet<usize> = app.nav.search_hit_row_ids(&d.rows).into_iter().collect();
            let cur_hit = app
                .nav
                .current_search_hit
                .and_then(|i| app.nav.search_hit_row_ids(&d.rows).get(i).copied());
            let cur_diff = app
                .nav
                .diff_index
                .and_then(|i| DiffNav::difference_row_ids(&d.rows).get(i).copied());
            grid::set_marks(self.h_grid, hits, cur_hit, cur_diff);
            // 差分位置ラベル
            self.set_text(self.h_pos, &app.nav.diff_position_text(&d.rows));
        }
    }

    // ---------- 比較 ----------
    fn start_compare(&self, user_initiated: bool) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let Some(plan) = app.model.plan_compare(user_initiated) else {
            return;
        };
        let token = Arc::new(diffloupe_core::CancellationToken::new());
        app.cancel = Some(token.clone());
        app.stop_watcher();
        let hwnd = self.hwnd.0 as usize;
        let token2 = token.clone();
        std::thread::spawn(move || {
            let progress = move |p: diffloupe_core::ComparisonProgress| unsafe {
                let hwnd = HWND(hwnd as *mut core::ffi::c_void);
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_APP_COMPARE_PROGRESS,
                    WPARAM(0),
                    LPARAM(Box::into_raw(Box::new(p)) as isize),
                );
            };
            let outcome = AppModel::run_compare(&plan, &progress, Some(&token2));
            let hwnd = HWND(hwnd as *mut core::ffi::c_void);
            unsafe {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_APP_COMPARE_DONE,
                    WPARAM(0),
                    LPARAM(Box::into_raw(Box::new(outcome)) as isize),
                );
            }
        });
        self.refresh_chrome();
        self.refresh_status();
    }

    fn stop_watcher(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        if let Some(w) = app.watcher.take() {
            w.stop();
        }
        app.model.watcher_active = false;
    }

    fn reconfigure_watcher(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        self.stop_watcher();
        if !app.model.should_watch() {
            return;
        }
        let roots = app.model.watcher_roots();
        let excluded = app.model.watcher_excluded_names();
        let hwnd = self.hwnd.0 as usize;
        let w = new_watcher(roots, excluded, move || unsafe {
            let hwnd = HWND(hwnd as *mut core::ffi::c_void);
            let _ = PostMessageW(Some(hwnd), WM_APP_WATCH_CHANGED, WPARAM(0), LPARAM(0));
        });
        w.start();
        app.watcher = Some(w);
        app.model.watcher_active = true;
    }

    fn on_folder_selected(&self, side: Side, path: PathBuf) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        if !app.model.apply_folder(path, side) {
            return;
        }
        self.refresh_ref_combo(side);
        self.layout();
        self.refresh_chrome();
        self.reconfigure_watcher();
    }

    fn do_report(&self, format: diffloupe_core::report::ReportFormat) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let name = AppModel::default_report_file_name(format);
        let ext = format.file_extension();
        let Some(path) = picker::save_file(
            self.hwnd,
            crate::strings::REPORT_SAVE_TITLE,
            &name,
            ext,
            ext,
        ) else {
            return;
        };
        // 上書き確認
        if path.exists() {
            let msg = wstr(&format!(
                "「{}」{}",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                crate::strings::OVERWRITE_SUFFIX
            ));
            let title = wstr(crate::strings::OVERWRITE_TITLE);
            let r = unsafe {
                MessageBoxW(
                    Some(self.hwnd),
                    windows::core::PCWSTR(msg.as_ptr()),
                    windows::core::PCWSTR(title.as_ptr()),
                    MB_YESNO | MB_ICONWARNING,
                )
            };
            if r != IDYES {
                return;
            }
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if let Some(text) = app.model.render_report(format, secs) {
            if let Err(e) = std::fs::write(&path, text) {
                app.model.error_message =
                    Some(format!("{}{}", crate::strings::REPORT_SAVE_FAILED, e));
                self.refresh_status();
            }
        }
    }

    fn context_menu(&self, entry_path: &str, pt: POINT) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let Some(entry) = app
            .model
            .result
            .as_ref()
            .and_then(|r| r.entry_by_path(entry_path))
            .cloned()
        else {
            return;
        };
        unsafe {
            let menu = CreatePopupMenu().unwrap_or_default();
            let mkl = |m: HMENU, text: &str, id: u16| {
                let _ = AppendMenuW(
                    m,
                    MF_STRING,
                    id as usize,
                    windows::core::PCWSTR(wstr(text).as_ptr()),
                );
            };
            if entry.left_path.is_some() {
                let sub = CreatePopupMenu().unwrap_or_default();
                mkl(sub, crate::strings::MENU_OPEN, ctx::OPEN_L);
                mkl(sub, crate::strings::MENU_REVEAL, ctx::REVEAL_L);
                mkl(sub, crate::strings::MENU_COPY_PATH, ctx::COPY_L);
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP,
                    sub.0 as usize,
                    windows::core::PCWSTR(wstr(crate::strings::MENU_LEFT).as_ptr()),
                );
            }
            if entry.right_path.is_some() {
                let sub = CreatePopupMenu().unwrap_or_default();
                mkl(sub, crate::strings::MENU_OPEN, ctx::OPEN_R);
                mkl(sub, crate::strings::MENU_REVEAL, ctx::REVEAL_R);
                mkl(sub, crate::strings::MENU_COPY_PATH, ctx::COPY_R);
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP,
                    sub.0 as usize,
                    windows::core::PCWSTR(wstr(crate::strings::MENU_RIGHT).as_ptr()),
                );
            }
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD, pt.x, pt.y, Some(0), self.hwnd, None);
            let _ = DestroyMenu(menu);
            let e = &entry;
            match cmd.0 as u16 {
                ctx::OPEN_L => {
                    if let Some(p) = &e.left_path {
                        ops::open_path(p);
                    }
                }
                ctx::REVEAL_L => {
                    if let Some(p) = &e.left_path {
                        ops::reveal_in_explorer(p);
                    }
                }
                ctx::COPY_L => {
                    if let Some(p) = &e.left_path {
                        ops::copy_to_clipboard(self.hwnd, &p.to_string_lossy());
                    }
                }
                ctx::OPEN_R => {
                    if let Some(p) = &e.right_path {
                        ops::open_path(p);
                    }
                }
                ctx::REVEAL_R => {
                    if let Some(p) = &e.right_path {
                        ops::reveal_in_explorer(p);
                    }
                }
                ctx::COPY_R => {
                    if let Some(p) = &e.right_path {
                        ops::copy_to_clipboard(self.hwnd, &p.to_string_lossy());
                    }
                }
                _ => {}
            }
        }
    }

    fn save_frame(&self) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let mut wp = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetWindowPlacement(self.hwnd, &mut wp).is_err() {
                return;
            }
        }
        let r = wp.rcNormalPosition;
        app.model.settings.window_frame = Some(WindowFrame {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
            maximized: wp.showCmd == SW_SHOWMAXIMIZED.0 as u32,
        });
        app.model.save_settings();
    }

    /// スモークテスト用: 任意エントリを選択して詳細を同期でロードする
    pub fn select_entry_sync(&self, rel_path: &str) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        app.model.selection = Some(rel_path.to_string());
        let cc = app.model.settings.content_comparison();
        let d = app
            .model
            .selected_entry()
            .map(|e| detail::compute_detail(&e.clone(), &cc));
        app.detail = d;
        self.show_detail_content();
    }

    /// スモークテスト用: 差分のあるテキストエントリを選ぶ(なければ最初のエントリ)
    pub fn select_first_different(&self) -> Option<String> {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let result = app.model.result.as_ref()?;
        let pick = result
            .entries
            .iter()
            .find(|e| e.status == diffloupe_core::DiffStatus::Different && !e.is_directory)
            .or_else(|| result.entries.iter().find(|e| !e.is_directory))?;
        let p = pick.relative_path.clone();
        self.select_entry_sync(&p);
        Some(p)
    }

    /// スモークテスト用: バイナリ差分エントリを選ぶ
    pub fn select_first_binary(&self) -> Option<String> {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        let result = app.model.result.as_ref()?;
        let pick = result.entries.iter().find(|e| {
            e.status == diffloupe_core::DiffStatus::Different
                && !e.is_directory
                && e.left_path
                    .as_ref()
                    .map(|p| {
                        diffloupe_core::text_loader::load(p)
                            .map(|t| {
                                matches!(t, diffloupe_core::text_loader::TextLoadResult::Binary)
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
        })?;
        let p = pick.relative_path.clone();
        self.select_entry_sync(&p);
        Some(p)
    }

    /// スモークテスト用: 画像比較ビューを直接開く
    pub fn show_image_compare(&self, left: PathBuf, right: PathBuf) {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        app.detail = Some(DetailContent::Image { left, right });
        self.show_detail_content();
    }

    /// スモークテスト用: 比較を同期的に実行
    pub fn run_compare_sync(&self, left: &std::path::Path, right: &std::path::Path) -> bool {
        let app = unsafe { &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut App) };
        if !app.model.apply_folder(left.to_path_buf(), Side::Left) {
            return false;
        }
        if !app.model.apply_folder(right.to_path_buf(), Side::Right) {
            return false;
        }
        let Some(plan) = app.model.plan_compare(true) else {
            return false;
        };
        let outcome = AppModel::run_compare(&plan, &|_| {}, None);
        app.model.finish_compare(outcome);
        self.rebuild_tree();
        self.refresh_chrome();
        self.refresh_status();
        app.model.result.is_some()
    }
}

fn mk_child(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    ex: WINDOW_EX_STYLE,
    id: i32,
) -> HWND {
    dialogs::mk(parent, class, text, style, ex, 0, 0, 10, 10, id)
}

fn build_menu(hwnd: HWND) {
    unsafe {
        let menu = CreateMenu().unwrap_or_default();
        let mk_item = |m: HMENU, text: &str, id: u16| {
            let _ = AppendMenuW(
                m,
                MF_STRING,
                id as usize,
                windows::core::PCWSTR(wstr(text).as_ptr()),
            );
        };
        let file = CreatePopupMenu().unwrap_or_default();
        mk_item(file, crate::strings::MENU_COMPARE, cmd::COMPARE);
        mk_item(file, crate::strings::MENU_SWAP, cmd::SWAP);
        let _ = AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null());
        mk_item(file, crate::strings::MENU_SETTINGS, cmd::SETTINGS);
        let _ = AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null());
        mk_item(file, crate::strings::MENU_EXIT, cmd::EXIT);
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            file.0 as usize,
            windows::core::PCWSTR(wstr(crate::strings::MENU_FILE).as_ptr()),
        );
        let rep = CreatePopupMenu().unwrap_or_default();
        mk_item(rep, crate::strings::REPORT_HTML, cmd::REP_HTML);
        mk_item(rep, crate::strings::REPORT_CSV, cmd::REP_CSV);
        mk_item(rep, crate::strings::REPORT_JSON, cmd::REP_JSON);
        mk_item(rep, crate::strings::REPORT_MD, cmd::REP_MD);
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            rep.0 as usize,
            windows::core::PCWSTR(wstr(crate::strings::MENU_REPORT).as_ptr()),
        );
        let sync = CreatePopupMenu().unwrap_or_default();
        mk_item(sync, crate::strings::MENU_SYNC_L2R, cmd::SYNC_L2R);
        mk_item(sync, crate::strings::MENU_SYNC_R2L, cmd::SYNC_R2L);
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            sync.0 as usize,
            windows::core::PCWSTR(wstr(crate::strings::MENU_SYNC_PLAN).as_ptr()),
        );
        let tw = CreatePopupMenu().unwrap_or_default();
        mk_item(tw, crate::strings::MENU_THREEWAY_OPEN, cmd::THREEWAY);
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            tw.0 as usize,
            windows::core::PCWSTR(wstr(crate::strings::MENU_THREEWAY).as_ptr()),
        );
        let help = CreatePopupMenu().unwrap_or_default();
        mk_item(help, crate::strings::MENU_ABOUT, cmd::ABOUT);
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            help.0 as usize,
            windows::core::PCWSTR(wstr(crate::strings::MENU_HELP).as_ptr()),
        );
        let _ = SetMenu(hwnd, Some(menu));
    }
}

fn create_children(hwnd: HWND) {
    use windows::Win32::UI::Controls::*;
    let b = |id, text| {
        mk_child(
            hwnd,
            "BUTTON",
            text,
            BS_PUSHBUTTON as u32,
            WINDOW_EX_STYLE(0),
            id,
        )
    };
    let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
    app.h_btn_left = b(id::BTN_LEFT, crate::strings::LEFT_FOLDER);
    app.h_edit_left = mk_child(
        hwnd,
        "EDIT",
        "",
        (ES_AUTOHSCROLL | ES_READONLY) as u32,
        WS_EX_CLIENTEDGE,
        id::EDIT_LEFT,
    );
    app.h_cmb_lref = mk_child(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32,
        WINDOW_EX_STYLE(0),
        id::CMB_LREF,
    );
    app.h_btn_right = b(id::BTN_RIGHT, crate::strings::RIGHT_FOLDER);
    app.h_edit_right = mk_child(
        hwnd,
        "EDIT",
        "",
        (ES_AUTOHSCROLL | ES_READONLY) as u32,
        WS_EX_CLIENTEDGE,
        id::EDIT_RIGHT,
    );
    app.h_cmb_rref = mk_child(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32,
        WINDOW_EX_STYLE(0),
        id::CMB_RREF,
    );
    app.h_btn_swap = b(id::BTN_SWAP, crate::strings::SWAP);
    app.h_cmb_history = mk_child(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32,
        WINDOW_EX_STYLE(0),
        id::CMB_HISTORY,
    );
    app.h_btn_3way = b(id::BTN_3WAY, crate::strings::THREE_WAY);
    app.h_btn_sync = b(id::BTN_SYNC, crate::strings::COPY_PLAN);
    app.h_btn_report = b(id::BTN_REPORT, crate::strings::SAVE_REPORT);
    app.h_btn_settings = b(id::BTN_SETTINGS, crate::strings::SETTINGS);
    app.h_btn_compare = b(id::BTN_COMPARE, crate::strings::COMPARE);
    app.h_edit_search = mk_child(
        hwnd,
        "EDIT",
        "",
        ES_AUTOHSCROLL as u32,
        WS_EX_CLIENTEDGE,
        id::EDIT_SEARCH,
    );
    app.h_cmb_filter = mk_child(
        hwnd,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32,
        WINDOW_EX_STYLE(0),
        id::CMB_FILTER,
    );
    for f in ResultFilter::ALL {
        let t = wstr(f.display_name());
        unsafe {
            SendMessageW(
                app.h_cmb_filter,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(t.as_ptr() as isize)),
            );
        }
    }
    unsafe {
        SendMessageW(
            app.h_cmb_filter,
            CB_SETCURSEL,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
        let cue = wstr(crate::strings::SEARCH_CUE);
        SendMessageW(
            app.h_edit_search,
            EM_SETCUEBANNER,
            Some(WPARAM(1)),
            Some(LPARAM(cue.as_ptr() as isize)),
        );
        let cue2 = wstr(crate::strings::DIFF_SEARCH_CUE);
        // ツリー
        app.h_tree = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            WC_TREEVIEWW,
            PCWSTR::null(),
            WS_CHILD
                | WS_VISIBLE
                | WINDOW_STYLE(TVS_HASLINES | TVS_HASBUTTONS | TVS_LINESATROOT | TVS_SHOWSELALWAYS),
            0,
            0,
            10,
            10,
            Some(hwnd),
            Some(HMENU(id::TREE as isize as *mut _)),
            Some(dialogs::hinst()),
            None,
        )
        .unwrap_or_default();
        app.h_grid = grid::create(hwnd, 0, 0, 0, 0, true, id::GRID);
        app.h_msg = mk_child(
            hwnd,
            "STATIC",
            "",
            SS_CENTER.0 | SS_CENTERIMAGE.0,
            WINDOW_EX_STYLE(0),
            id::MSG,
        );
        app.h_hdr1 = mk_child(hwnd, "STATIC", "", SS_LEFT.0, WINDOW_EX_STYLE(0), id::HDR1);
        app.h_hdr2 = mk_child(
            hwnd,
            "STATIC",
            "",
            SS_LEFTNOWORDWRAP.0,
            WINDOW_EX_STYLE(0),
            id::HDR2,
        );
        app.h_pdiff = b(id::BTN_PDIFF, "↑");
        app.h_ndiff = b(id::BTN_NDIFF, "↓");
        app.h_pos = mk_child(
            hwnd,
            "STATIC",
            "",
            SS_LEFT.0,
            WINDOW_EX_STYLE(0),
            id::LBL_POS,
        );
        app.h_dsearch = mk_child(
            hwnd,
            "EDIT",
            "",
            ES_AUTOHSCROLL as u32,
            WS_EX_CLIENTEDGE,
            id::EDIT_DSEARCH,
        );
        SendMessageW(
            app.h_dsearch,
            EM_SETCUEBANNER,
            Some(WPARAM(1)),
            Some(LPARAM(cue2.as_ptr() as isize)),
        );
        app.h_phit = b(id::BTN_PHIT, "↑");
        app.h_nhit = b(id::BTN_NHIT, "↓");
        app.h_case = mk_child(
            hwnd,
            "BUTTON",
            "Aa",
            BS_AUTOCHECKBOX as u32,
            WINDOW_EX_STYLE(0),
            id::CHK_CASE,
        );
        app.h_all = mk_child(
            hwnd,
            "BUTTON",
            crate::strings::SHOW_ALL_ROWS,
            BS_AUTOCHECKBOX as u32,
            WINDOW_EX_STYLE(0),
            id::CHK_ALL,
        );
        app.h_status = mk_child(
            hwnd,
            "STATIC",
            crate::strings::STATUS_PROMPT,
            SS_LEFT.0,
            WINDOW_EX_STYLE(0),
            id::STATUS,
        );
        app.h_progress = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PROGRESS_CLASSW,
            PCWSTR::null(),
            WS_CHILD,
            0,
            0,
            10,
            10,
            Some(hwnd),
            Some(HMENU(id::PROGRESS as isize as *mut _)),
            Some(dialogs::hinst()),
            None,
        )
        .unwrap_or_default();
    }
    // フォント適用
    let font = app.font;
    let mono = app.mono_font;
    unsafe {
        let mut child = GetWindow(hwnd, GW_CHILD);
        while let Ok(c) = child {
            SendMessageW(
                c,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            child = GetWindow(c, GW_HWNDNEXT);
        }
        SendMessageW(
            app.h_grid,
            WM_SETFONT,
            Some(WPARAM(mono.0 as usize)),
            Some(LPARAM(1)),
        );
        SendMessageW(
            app.h_tree,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let cs = lparam.0 as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            create_children(hwnd);
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            app.rebuild_tree();
            app.refresh_history();
            app.refresh_chrome();
            app.refresh_status();
            app.layout();
            DragAcceptFiles(hwnd, true);
            theme::apply_title_bar(hwnd, theme::detect_dark());
            LRESULT(0)
        }
        WM_SIZE => {
            let app = unsafe { &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const App) };
            app.layout();
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let rc = lparam.0 as *const RECT;
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    (*rc).left,
                    (*rc).top,
                    (*rc).right - (*rc).left,
                    (*rc).bottom - (*rc).top,
                    SWP_NOZORDER,
                );
            }
            app.layout();
            LRESULT(0)
        }
        WM_CLOSE => {
            let app = unsafe { &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const App) };
            app.save_frame();
            app.stop_watcher();
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_DROPFILES => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let drop = HDROP(wparam.0 as *mut _);
            unsafe {
                let n = DragQueryFileW(drop, 0xFFFFFFFF, None);
                if n == 1 {
                    let mut buf = vec![0u16; 1024];
                    let len = DragQueryFileW(drop, 0, Some(&mut buf));
                    let path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
                    let mut pt = POINT::default();
                    let _ = DragQueryPoint(drop, &mut pt);
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let side = if pt.y < rc.bottom / 3 {
                        Side::Left
                    } else {
                        Side::Right
                    };
                    app.model.accept_dropped_folder(&path, side);
                    app.on_folder_selected_refresh();
                }
                DragFinish(drop);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            match id {
                x if x == cmd::COMPARE as i32 => app.start_compare(true),
                x if x == cmd::SWAP as i32 => {
                    if app.model.swap_sides() {
                        app.rebuild_tree();
                        app.refresh_chrome();
                        app.refresh_status();
                        app.reconfigure_watcher();
                    }
                }
                x if x == cmd::SETTINGS as i32 || x == id::BTN_SETTINGS => {
                    app.settings_hwnd = dialogs::open_settings(hwnd, &mut app.model, app.font);
                }
                x if x == cmd::EXIT as i32 => unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                },
                x if x == cmd::REP_HTML as i32 => {
                    app.do_report(diffloupe_core::report::ReportFormat::Html)
                }
                x if x == cmd::REP_CSV as i32 => {
                    app.do_report(diffloupe_core::report::ReportFormat::Csv)
                }
                x if x == cmd::REP_JSON as i32 => {
                    app.do_report(diffloupe_core::report::ReportFormat::Json)
                }
                x if x == cmd::REP_MD as i32 => {
                    app.do_report(diffloupe_core::report::ReportFormat::Markdown)
                }
                x if x == cmd::SYNC_L2R as i32 || x == id::BTN_SYNC => {
                    app.sync_hwnd = dialogs::open_sync_plan(
                        hwnd,
                        &mut app.model,
                        app.font,
                        diffloupe_core::sync_plan::SyncDirection::LeftToRight,
                    );
                }
                x if x == cmd::SYNC_R2L as i32 => {
                    app.sync_hwnd = dialogs::open_sync_plan(
                        hwnd,
                        &mut app.model,
                        app.font,
                        diffloupe_core::sync_plan::SyncDirection::RightToLeft,
                    );
                }
                x if x == cmd::THREEWAY as i32 || x == id::BTN_3WAY => {
                    app.tw_hwnd = dialogs::open_threeway(hwnd, &mut app.model, app.font);
                }
                x if x == cmd::ABOUT as i32 => {
                    let t = wstr(crate::strings::ABOUT_TEXT);
                    let c = wstr(crate::strings::TITLE);
                    unsafe {
                        MessageBoxW(
                            Some(hwnd),
                            windows::core::PCWSTR(t.as_ptr()),
                            windows::core::PCWSTR(c.as_ptr()),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                }
                x if x == cmd::PREV_DIFF as i32 || x == id::BTN_PDIFF => app.nav_diff(-1),
                x if x == cmd::NEXT_DIFF as i32 || x == id::BTN_NDIFF => app.nav_diff(1),
                x if x == cmd::PREV_HIT as i32 || x == id::BTN_PHIT => app.nav_hit(-1),
                x if x == cmd::NEXT_HIT as i32 || x == id::BTN_NHIT => app.nav_hit(1),
                id::BTN_LEFT => {
                    if let Some(p) = picker::pick_folder(hwnd, crate::strings::PICK_LEFT) {
                        app.on_folder_selected(Side::Left, p);
                    }
                }
                id::BTN_RIGHT => {
                    if let Some(p) = picker::pick_folder(hwnd, crate::strings::PICK_RIGHT) {
                        app.on_folder_selected(Side::Right, p);
                    }
                }
                id::BTN_SWAP => {
                    if app.model.swap_sides() {
                        app.rebuild_tree();
                        app.refresh_chrome();
                        app.refresh_status();
                        app.reconfigure_watcher();
                    }
                }
                id::BTN_COMPARE => {
                    if app.model.phase.is_running() {
                        if let Some(c) = &app.cancel {
                            c.cancel();
                        }
                    } else {
                        app.start_compare(true);
                    }
                }
                id::CMB_LREF | id::CMB_RREF => {
                    if code == CBN_SELCHANGE as u16 {
                        let side = if id == id::CMB_LREF {
                            Side::Left
                        } else {
                            Side::Right
                        };
                        let h = if id == id::CMB_LREF {
                            app.h_cmb_lref
                        } else {
                            app.h_cmb_rref
                        };
                        let sel = unsafe {
                            SendMessageW(h, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)))
                        }
                        .0 as usize;
                        let count = unsafe {
                            SendMessageW(h, CB_GETCOUNT, Some(WPARAM(0)), Some(LPARAM(0)))
                        }
                        .0 as usize;
                        if sel == 0 {
                            app.model.select_git_ref(side, None);
                        } else if sel == count - 1 {
                            // 任意のrefを入力
                            if let Some(r) = dialogs::input_ref(
                                hwnd,
                                crate::strings::REF_INPUT_TITLE,
                                app.model.git_ref(side).map(|s| s.as_str()).unwrap_or(""),
                            ) {
                                app.model.select_git_ref(side, Some(r));
                            }
                        } else {
                            let mut buf = vec![0u16; 512];
                            unsafe {
                                SendMessageW(
                                    h,
                                    CB_GETLBTEXT,
                                    Some(WPARAM(sel)),
                                    Some(LPARAM(buf.as_mut_ptr() as isize)),
                                );
                            }
                            let end = buf.iter().position(|&c| c == 0).unwrap_or(0);
                            app.model
                                .select_git_ref(side, Some(String::from_utf16_lossy(&buf[..end])));
                        }
                        app.refresh_ref_combo(side);
                        app.reconfigure_watcher();
                    }
                }
                id::CMB_HISTORY => {
                    if code == CBN_SELCHANGE as u16 {
                        let sel = unsafe {
                            SendMessageW(
                                app.h_cmb_history,
                                CB_GETCURSEL,
                                Some(WPARAM(0)),
                                Some(LPARAM(0)),
                            )
                        }
                        .0 as usize;
                        if sel >= 1 {
                            let item = app.model.settings.recent_comparisons.get(sel - 1).cloned();
                            if let Some(item) = item {
                                if app.model.restore_comparison(&item) {
                                    app.refresh_ref_combo(Side::Left);
                                    app.refresh_ref_combo(Side::Right);
                                    app.layout();
                                    app.refresh_chrome();
                                    app.reconfigure_watcher();
                                    app.start_compare(true);
                                }
                            }
                        }
                    }
                }
                id::EDIT_SEARCH => {
                    if code == EN_CHANGE as u16 {
                        app.model.result_search_text = get_edit(app.h_edit_search);
                        app.rebuild_tree();
                    }
                }
                id::EDIT_DSEARCH => {
                    if code == EN_CHANGE as u16 {
                        app.nav.search_text = get_edit(app.h_dsearch);
                        if let Some(DetailContent::Diff(d)) = &app.detail {
                            app.nav.on_search_changed(&d.rows);
                            // 折りたたみ展開を反映して再描画
                            let items = app.nav.display_items(&d.rows);
                            let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                            let cols = grid::model_columns(app.h_grid);
                            grid::set_model(
                                app.h_grid,
                                grid_model::GridModel {
                                    columns: cols,
                                    items: grid_items,
                                },
                                true,
                            );
                        }
                        app.update_marks();
                    }
                }
                id::CHK_CASE => {
                    app.nav.search_case_sensitive = dialogs::is_checked(app.h_case);
                    if let Some(DetailContent::Diff(d)) = &app.detail {
                        app.nav.on_search_changed(&d.rows);
                        let items = app.nav.display_items(&d.rows);
                        let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                        let cols = grid::model_columns(app.h_grid);
                        grid::set_model(
                            app.h_grid,
                            grid_model::GridModel {
                                columns: cols,
                                items: grid_items,
                            },
                            true,
                        );
                    }
                    app.update_marks();
                }
                id::CHK_ALL => {
                    app.nav.show_all_context_rows = dialogs::is_checked(app.h_all);
                    if let Some(DetailContent::Diff(d)) = &app.detail {
                        let items = app.nav.display_items(&d.rows);
                        let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                        let cols = grid::model_columns(app.h_grid);
                        grid::set_model(
                            app.h_grid,
                            grid_model::GridModel {
                                columns: cols,
                                items: grid_items,
                            },
                            true,
                        );
                    }
                }
                id::CMB_FILTER => {
                    if code == CBN_SELCHANGE as u16 {
                        let sel = unsafe {
                            SendMessageW(
                                app.h_cmb_filter,
                                CB_GETCURSEL,
                                Some(WPARAM(0)),
                                Some(LPARAM(0)),
                            )
                        }
                        .0 as usize;
                        if let Some(f) = ResultFilter::ALL.get(sel) {
                            app.model.result_filter = *f;
                            app.rebuild_tree();
                        }
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let hdr = unsafe { &*(lparam.0 as *const NMHDR) };
            if hdr.idFrom as i32 == id::TREE && hdr.code == TVN_SELCHANGEDW {
                let nmtv = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let idx = nmtv.itemNew.lParam.0 as usize;
                if let Some(n) = app.nodes.get(idx) {
                    app.model.selection = Some(n.id.clone());
                    app.load_detail();
                }
            } else if hdr.idFrom as i32 == id::TREE && hdr.code == NM_RCLICK {
                let nmtv = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let _ = nmtv;
                // 右クリック位置の項目を特定
                let mut pt = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                    let mut tvpt = pt;
                    let _ = ScreenToClient(app.h_tree, &mut tvpt);
                    let mut info = TVHITTESTINFO {
                        pt: tvpt,
                        ..Default::default()
                    };
                    let hit: HTREEITEM = HTREEITEM(
                        SendMessageW(
                            app.h_tree,
                            TVM_HITTEST,
                            Some(WPARAM(0)),
                            Some(LPARAM(&mut info as *mut _ as isize)),
                        )
                        .0,
                    );
                    if hit.0 != 0 {
                        let mut item = TVITEMW {
                            mask: TVIF_PARAM,
                            hItem: hit,
                            ..Default::default()
                        };
                        SendMessageW(
                            app.h_tree,
                            TVM_GETITEMW,
                            Some(WPARAM(0)),
                            Some(LPARAM(&mut item as *mut _ as isize)),
                        );
                        let idx = item.lParam.0 as usize;
                        if let Some(n) = app.nodes.get(idx) {
                            let path = n.id.clone();
                            app.model.selection = Some(path.clone());
                            app.context_menu(&path, pt);
                        }
                    }
                }
            } else if hdr.idFrom as i32 == id::TREE && hdr.code == NM_CUSTOMDRAW {
                let nmcd = unsafe { &mut *(lparam.0 as *mut NMTVCUSTOMDRAW) };
                match nmcd.nmcd.dwDrawStage {
                    CDDS_PREPAINT => return LRESULT(CDRF_NOTIFYITEMDRAW as isize),
                    CDDS_ITEMPREPAINT => {
                        let idx = nmcd.nmcd.lItemlParam.0 as usize;
                        if let Some(n) = app.nodes.get(idx) {
                            let c = match n.status {
                                diffloupe_core::DiffStatus::Identical => theme::rgb(20, 140, 60),
                                diffloupe_core::DiffStatus::Different => theme::rgb(200, 40, 40),
                                diffloupe_core::DiffStatus::LeftOnly => theme::rgb(30, 100, 210),
                                diffloupe_core::DiffStatus::RightOnly => theme::rgb(140, 70, 200),
                                diffloupe_core::DiffStatus::Error => theme::rgb(200, 120, 0),
                            };
                            nmcd.clrText = c;
                        }
                        return LRESULT(CDRF_NOTIFYPOSTPAINT as isize);
                    }
                    CDDS_ITEMPOSTPAINT => {
                        let idx = nmcd.nmcd.lItemlParam.0 as usize;
                        if let Some(n) = app.nodes.get(idx) {
                            // ラベル右にバッジテキスト
                            let mut rc = nmcd.nmcd.rc;
                            let hdc = nmcd.nmcd.hdc;
                            let label = viewmodel::status_badge_label(n.status, n.is_dir);
                            let tw = wstr(label);
                            let mut tvec = tw[..tw.len() - 1].to_vec();
                            let mut sz = SIZE::default();
                            unsafe {
                                let _ = GetTextExtentPoint32W(hdc, &tvec, &mut sz);
                            }
                            rc.left = rc.right + app.sc(6);
                            rc.right = rc.left + sz.cx + app.sc(10);
                            let t = theme::current();
                            let col = match n.status {
                                diffloupe_core::DiffStatus::Identical => theme::rgb(20, 140, 60),
                                diffloupe_core::DiffStatus::Different => theme::rgb(200, 40, 40),
                                diffloupe_core::DiffStatus::LeftOnly => theme::rgb(30, 100, 210),
                                diffloupe_core::DiffStatus::RightOnly => theme::rgb(140, 70, 200),
                                diffloupe_core::DiffStatus::Error => theme::rgb(200, 120, 0),
                            };
                            let _ = t;
                            unsafe {
                                let brush = CreateSolidBrush(col);
                                // 薄色背景: 20% 相当を自前ブレンド
                                let bgc = theme::rgb(
                                    (255 * 4 / 5 + col.0 as i32 / 5) as u8,
                                    (255 * 4 / 5 + ((col.0 >> 8) & 0xFF) as i32 / 5) as u8,
                                    (255 * 4 / 5 + ((col.0 >> 16) & 0xFF) as i32 / 5) as u8,
                                );
                                let b2 = CreateSolidBrush(bgc);
                                FillRect(hdc, &rc, b2);
                                let _ = DeleteObject(HGDIOBJ(b2.0));
                                let _ = DeleteObject(HGDIOBJ(brush.0));
                                SetBkMode(hdc, TRANSPARENT);
                                SetTextColor(hdc, col);
                                DrawTextW(
                                    hdc,
                                    &mut tvec,
                                    &mut rc,
                                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                                );
                            }
                        }
                        return LRESULT(CDRF_DODEFAULT as isize);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        m if m == WM_APP_COMPARE_PROGRESS => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let p = unsafe { *Box::from_raw(lparam.0 as *mut diffloupe_core::ComparisonProgress) };
            if app.model.phase.is_running() {
                app.model.phase = phase_from_progress(p);
            }
            app.refresh_status();
            LRESULT(0)
        }
        m if m == WM_APP_COMPARE_DONE => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let outcome = unsafe { *Box::from_raw(lparam.0 as *mut CompareOutcome) };
            app.cancel = None;
            let recompare = app.model.finish_compare(outcome);
            app.rebuild_tree();
            app.refresh_chrome();
            app.refresh_status();
            app.refresh_history();
            if let Some(alert) = &app.model.entry_limit_alert {
                let msg = wstr(&alert.message);
                let title = wstr(crate::strings::ENTRY_LIMIT_TITLE);
                unsafe {
                    MessageBoxW(
                        Some(hwnd),
                        windows::core::PCWSTR(msg.as_ptr()),
                        windows::core::PCWSTR(title.as_ptr()),
                        MB_OK | MB_ICONWARNING,
                    );
                }
                app.model.entry_limit_alert = None;
            }
            app.reconfigure_watcher();
            app.load_detail();
            if recompare && app.model.auto_recompare() == AutoRefreshEffect::Recompare {
                app.start_compare(false);
            }
            LRESULT(0)
        }
        m if m == WM_APP_WATCH_CHANGED => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            if app
                .model
                .handle_watcher_change(app.model.phase.is_running())
                == AutoRefreshEffect::Recompare
            {
                app.start_compare(false);
            }
            app.refresh_status();
            LRESULT(0)
        }
        m if m == WM_APP_DETAIL_LOADED => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            if wparam.0 as u64 == app.detail_gen {
                let d = unsafe { *Box::from_raw(lparam.0 as *mut DetailContent) };
                app.detail = Some(d);
                app.show_detail_content();
            } else {
                unsafe {
                    drop(Box::from_raw(lparam.0 as *mut DetailContent));
                }
            }
            LRESULT(0)
        }
        m if m == WM_APP_SETTINGS_CHANGED => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            app.reconfigure_watcher();
            app.refresh_status();
            LRESULT(0)
        }
        m if m == grid::WM_APP_GRID_FOLD => {
            let app = unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) };
            let gid = wparam.0;
            app.nav.expanded_groups.insert(gid);
            if let Some(DetailContent::Diff(d)) = &app.detail {
                let items = app.nav.display_items(&d.rows);
                let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                let cols = grid::model_columns(app.h_grid);
                grid::set_model(
                    app.h_grid,
                    grid_model::GridModel {
                        columns: cols,
                        items: grid_items,
                    },
                    true,
                );
            }
            app.update_marks();
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

impl App {
    fn nav_diff(&mut self, offset: isize) {
        if let Some(DetailContent::Diff(d)) = &self.detail {
            if let Some(row_id) = self.nav.jump_to_difference(&d.rows, offset) {
                if let Some(idx) = self.nav.reveal_row(&d.rows, row_id) {
                    let items = self.nav.display_items(&d.rows);
                    let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                    let cols = grid::model_columns(self.h_grid);
                    grid::set_model(
                        self.h_grid,
                        grid_model::GridModel {
                            columns: cols,
                            items: grid_items,
                        },
                        true,
                    );
                    grid::scroll_to_source(self.h_grid, row_id);
                    let _ = idx;
                }
            }
        }
        self.update_marks();
    }

    fn nav_hit(&mut self, offset: isize) {
        if let Some(DetailContent::Diff(d)) = &self.detail {
            if let Some(row_id) = self.nav.jump_to_search_hit(&d.rows, offset) {
                let _ = self.nav.reveal_row(&d.rows, row_id);
                let items = self.nav.display_items(&d.rows);
                let grid_items = grid_model::build_diff_items(&items, &d.highlights);
                let cols = grid::model_columns(self.h_grid);
                grid::set_model(
                    self.h_grid,
                    grid_model::GridModel {
                        columns: cols,
                        items: grid_items,
                    },
                    true,
                );
                grid::scroll_to_source(self.h_grid, row_id);
            }
        }
        self.update_marks();
    }

    fn on_folder_selected_refresh(&self) {
        self.refresh_ref_combo(Side::Left);
        self.refresh_ref_combo(Side::Right);
        self.layout();
        self.refresh_chrome();
        self.reconfigure_watcher();
    }
}

/// メインウィンドウ生成(settings_path は %APPDATA%\DiffLoupe\settings.json)
pub fn create_window(settings_path: PathBuf, smoke: bool) -> HWND {
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None) }.unwrap_or_default().into();
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_TREEVIEW_CLASSES
                | ICC_LISTVIEW_CLASSES
                | ICC_BAR_CLASSES
                | ICC_STANDARD_CLASSES
                | ICC_WIN95_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
    }
    grid::register_class(hinst);
    imagepane::register_class(hinst);
    let class = wstr(MAIN_CLASS);
    unsafe {
        let wc = WNDCLASSW {
            hInstance: hinst,
            lpszClassName: windows::core::PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            lpszMenuName: PCWSTR::null(),
            style: CS_HREDRAW | CS_VREDRAW,
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    let mut app = Box::new(App {
        model: AppModel::new(Arc::new(diffloupe_edition::FreeEdition), settings_path),
        hwnd: HWND::default(),
        font: HFONT::default(),
        mono_font: HFONT::default(),
        h_btn_left: HWND::default(),
        h_edit_left: HWND::default(),
        h_cmb_lref: HWND::default(),
        h_btn_right: HWND::default(),
        h_edit_right: HWND::default(),
        h_cmb_rref: HWND::default(),
        h_btn_swap: HWND::default(),
        h_cmb_history: HWND::default(),
        h_btn_3way: HWND::default(),
        h_btn_sync: HWND::default(),
        h_btn_report: HWND::default(),
        h_btn_settings: HWND::default(),
        h_btn_compare: HWND::default(),
        h_edit_search: HWND::default(),
        h_cmb_filter: HWND::default(),
        h_tree: HWND::default(),
        h_grid: HWND::default(),
        h_msg: HWND::default(),
        h_imgpane: HWND::default(),
        h_hdr1: HWND::default(),
        h_hdr2: HWND::default(),
        h_pdiff: HWND::default(),
        h_ndiff: HWND::default(),
        h_pos: HWND::default(),
        h_dsearch: HWND::default(),
        h_phit: HWND::default(),
        h_nhit: HWND::default(),
        h_case: HWND::default(),
        h_all: HWND::default(),
        h_status: HWND::default(),
        h_progress: HWND::default(),
        nodes: Vec::new(),
        detail: None,
        nav: DiffNav::default(),
        detail_gen: 0,
        cancel: None,
        watcher: None,
        split_ratio: 0.38,
        settings_hwnd: HWND::default(),
        sync_hwnd: HWND::default(),
        tw_hwnd: HWND::default(),
        _is_smoke: smoke,
    });
    let frame = app.model.settings.window_frame;
    let (x, y, w, h, maximized) = match frame {
        Some(f) => (f.x, f.y, f.width, f.height, f.maximized),
        None => (CW_USEDEFAULT, CW_USEDEFAULT, 1180, 760, false),
    };
    let hwnd = unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR(class.as_ptr()),
            windows::core::PCWSTR(wstr(crate::strings::TITLE).as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            x,
            y,
            w.max(720),
            h.max(480),
            None,
            None,
            Some(hinst),
            Some(app.as_mut() as *mut App as *const _),
        )
        .unwrap_or_default();
        app.hwnd = hwnd;
        app.font = make_font(hwnd, false);
        app.mono_font = make_font(hwnd, true);
        build_menu(hwnd);
        if maximized {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
        hwnd
    };
    std::mem::forget(app); // App はウィンドウ USERDATA が所有
    hwnd
}

/// アクセラレータ(Ctrl+R 比較 / Ctrl+Shift+S 交換 / Ctrl+↑↓ 差分 / Ctrl+G 検索)
fn make_accel() -> HACCEL {
    let accels = [
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: 0x52, /* R */
            cmd: cmd::COMPARE,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: 0x53, /* S */
            cmd: cmd::SWAP,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: 0x26, /* UP */
            cmd: cmd::PREV_DIFF,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: 0x28, /* DOWN */
            cmd: cmd::NEXT_DIFF,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: 0x47, /* G */
            cmd: cmd::PREV_HIT,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: 0x47, /* G */
            cmd: cmd::NEXT_HIT,
        },
    ];
    unsafe { CreateAcceleratorTableW(&accels) }.unwrap_or_default()
}

pub fn run() -> i32 {
    let dir = appdata_dir();
    let hwnd = create_window(dir.join("settings.json"), false);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let accel = make_accel();
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if TranslateAcceleratorW(hwnd, accel, &msg) == 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    0
}

/// 設定保存先ディレクトリ(%APPDATA%\DiffLoupe。取得失敗時は exe 横)
pub fn appdata_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("DiffLoupe");
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default()
        .join("DiffLoupe")
}

/// スモークテスト用: App への参照(ユーザーデータ)
pub fn app_ref(hwnd: HWND) -> &'static mut App {
    unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App) }
}
