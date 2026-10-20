//! 読み取り専用3-way差分ビュー。base/left/right の3ファイルを選んで
//! `ThreeWayDiff::make` の結果を3列+状態列で表示する。

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use diffloupe_core::text_loader::{self, TextLoadResult};
use diffloupe_core::three_way::{ThreeWayDiff, ThreeWayDiffStatus};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::model::AppModel;
use crate::strings as s;

struct TwRow {
    status_css: &'static str,
    status: &'static str,
    base: String,
    left: String,
    right: String,
}

fn status_name(st: ThreeWayDiffStatus) -> &'static str {
    match st {
        ThreeWayDiffStatus::Unchanged => s::TW_SAME,
        ThreeWayDiffStatus::LeftChanged => "左のみ変更",
        ThreeWayDiffStatus::RightChanged => "右のみ変更",
        ThreeWayDiffStatus::BothChangedSame => "両者同一変更",
        ThreeWayDiffStatus::Conflict => "競合",
    }
}

fn status_css(st: ThreeWayDiffStatus) -> &'static str {
    match st {
        ThreeWayDiffStatus::Unchanged => "dl-badge-identical",
        ThreeWayDiffStatus::LeftChanged => "dl-badge-leftonly",
        ThreeWayDiffStatus::RightChanged => "dl-badge-rightonly",
        ThreeWayDiffStatus::BothChangedSame => "dl-badge-different",
        ThreeWayDiffStatus::Conflict => "dl-badge-error",
    }
}

fn fmt_cell(num: Option<usize>, text: &Option<String>) -> String {
    match (num, text) {
        (Some(n), Some(t)) => format!("{n:>5}  {t}"),
        (None, Some(t)) => format!("       {t}"),
        (Some(n), None) => format!("{n:>5}"),
        _ => String::new(),
    }
}

fn build_list(store: &gio::ListStore) -> gtk4::ListView {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let mk = |w: i32| {
            let l = gtk4::Label::new(None);
            l.set_xalign(0.0);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            l.set_width_chars(w);
            l.set_max_width_chars(w);
            l
        };
        let badge = mk(12);
        badge.add_css_class("dl-badge");
        let base = mk(26);
        let left = mk(26);
        let right = mk(26);
        right.set_hexpand(true);
        for l in [&base, &left, &right] {
            l.add_css_class("dl-mono");
        }
        b.append(&badge);
        b.append(&base);
        b.append(&left);
        b.append(&right);
        item.set_child(Some(&b));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
        let Some(any) = item
            .item()
            .and_then(|o| o.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        let Some(b) = item.child().and_then(|w| w.downcast::<gtk4::Box>().ok()) else {
            return;
        };
        let row = any.borrow::<TwRow>();
        let mut w = b.first_child();
        let mut next = || {
            let cur = w.take();
            w = cur.as_ref().and_then(|c| c.next_sibling());
            cur.and_then(|c| c.downcast::<gtk4::Label>().ok())
        };
        if let Some(l) = next() {
            l.set_text(row.status);
            for c in [
                "dl-badge-identical",
                "dl-badge-different",
                "dl-badge-leftonly",
                "dl-badge-rightonly",
                "dl-badge-error",
            ] {
                l.remove_css_class(c);
            }
            l.add_css_class(row.status_css);
        }
        for text in [&row.base, &row.left, &row.right] {
            if let Some(l) = next() {
                l.set_text(text);
            }
        }
    });
    let sel = gtk4::NoSelection::new(Some(store.clone()));
    gtk4::ListView::new(Some(sel), Some(factory))
}

/// ファイル選択行(ラベル + パス + 選択ボタン)。
fn picker_row(
    win: &gtk4::Window,
    label: &str,
    dialog_title: &'static str,
    slot: Rc<RefCell<Option<PathBuf>>>,
    on_change: Rc<dyn Fn()>,
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let lab = gtk4::Label::new(Some(label));
    lab.set_width_chars(6);
    let path = gtk4::Label::new(Some(s::PATH_PLACEHOLDER));
    path.set_xalign(0.0);
    path.set_hexpand(true);
    path.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    let btn = gtk4::Button::with_label(s::SELECT);
    row.append(&lab);
    row.append(&path);
    row.append(&btn);
    let w = win.clone();
    btn.connect_clicked(move |_| {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title(dialog_title);
        let slot = slot.clone();
        let path = path.clone();
        let on_change = on_change.clone();
        dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(p) = file.path() {
                    *slot.borrow_mut() = Some(p.clone());
                    path.set_text(&p.to_string_lossy());
                    on_change();
                }
            }
        });
    });
    row
}

fn load_text(p: &std::path::Path, label: &str) -> Result<String, String> {
    match text_loader::load(p) {
        Ok(TextLoadResult::Text(t, _)) => Ok(t),
        Ok(TextLoadResult::TooLarge) => Err(s::tw_too_large(label)),
        Ok(TextLoadResult::Binary) => Err(s::tw_not_text(
            label,
            &p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )),
        Err(e) => Err(s::msg_load_error(&e.to_string())),
    }
}

/// 3-way 差分ウィンドウを開く。エディション拡張があればそちらに差し替える
/// (呼び出し側で capabilities().three_way_view を見て分岐済み)。
pub fn open(parent: &impl IsA<gtk4::Window>, model: &Rc<AppModel>) {
    let win = gtk4::Window::new();
    win.set_title(Some(s::TW_TITLE));
    win.set_transient_for(Some(parent));
    win.set_modal(true);
    win.set_default_size(1000, 640);

    let base_path = Rc::new(RefCell::new(None::<PathBuf>));
    let left_path = Rc::new(RefCell::new(None::<PathBuf>));
    let right_path = Rc::new(RefCell::new(None::<PathBuf>));

    let legend = gtk4::Label::new(None);
    legend.set_xalign(0.0);
    let status = gtk4::Label::new(Some(s::TW_PLACEHOLDER));
    status.set_xalign(0.0);
    status.add_css_class("dl-empty");

    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let list = build_list(&store);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_vexpand(true);

    let cc = model.settings.borrow().content;

    let compute = {
        let store = store.clone();
        let legend = legend.clone();
        let status = status.clone();
        let base_path = base_path.clone();
        let left_path = left_path.clone();
        let right_path = right_path.clone();
        move || {
            let (Some(b), Some(l), Some(r)) = (
                base_path.borrow().clone(),
                left_path.borrow().clone(),
                right_path.borrow().clone(),
            ) else {
                status.set_text(s::TW_PLACEHOLDER);
                return;
            };
            status.set_text(s::TW_COMPUTING);
            let bt = match load_text(&b, s::TW_BASE) {
                Ok(t) => t,
                Err(e) => {
                    status.set_text(&e);
                    return;
                }
            };
            let lt = match load_text(&l, s::TW_LEFT) {
                Ok(t) => t,
                Err(e) => {
                    status.set_text(&e);
                    return;
                }
            };
            let rt = match load_text(&r, s::TW_RIGHT) {
                Ok(t) => t,
                Err(e) => {
                    status.set_text(&e);
                    return;
                }
            };
            let diff = ThreeWayDiff::make(&bt, &lt, &rt, &cc);
            store.remove_all();
            for line in &diff.lines {
                store.append(&glib::BoxedAnyObject::new(TwRow {
                    status_css: status_css(line.status),
                    status: status_name(line.status),
                    base: fmt_cell(line.base_number, &line.base),
                    left: fmt_cell(line.left_number, &line.left),
                    right: fmt_cell(line.right_number, &line.right),
                }));
            }
            legend.set_text(&format!(
                "{}   {}   {}   {}",
                s::tw_left_changed(diff.left_changed_count),
                s::tw_right_changed(diff.right_changed_count),
                s::tw_both_same(diff.both_changed_same_count),
                s::tw_conflicts(diff.conflict_count),
            ));
            status.set_text(&format!(
                "{}   {}{}",
                s::tw_lines(diff.lines.len()),
                s::tw_conflicts(diff.conflict_count),
                if diff.truncated {
                    format!(
                        "   {}",
                        s::tw_truncated(diffloupe_core::three_way::MAX_TOTAL_LINES)
                    )
                } else {
                    String::new()
                }
            ));
        }
    };
    let compute = Rc::new(compute);

    let pickers = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    pickers.append(&picker_row(
        &win,
        s::TW_BASE,
        s::SELECT_BASE_FILE,
        base_path.clone(),
        compute.clone(),
    ));
    pickers.append(&picker_row(
        &win,
        s::TW_LEFT,
        s::SELECT_LEFT_FILE,
        left_path.clone(),
        compute.clone(),
    ));
    pickers.append(&picker_row(
        &win,
        s::TW_RIGHT,
        s::SELECT_RIGHT_FILE,
        right_path.clone(),
        compute.clone(),
    ));
    pickers.set_margin_start(8);
    pickers.set_margin_end(8);
    pickers.set_margin_top(8);

    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    for (t, w) in [
        (s::TW_COL_STATUS, 12),
        (s::TW_COL_BASE, 26),
        (s::TW_COL_LEFT, 26),
        (s::TW_COL_RIGHT, 26),
    ] {
        let l = gtk4::Label::new(Some(t));
        l.set_xalign(0.0);
        l.set_width_chars(w);
        l.set_max_width_chars(w);
        l.add_css_class("heading");
        head.append(&l);
    }
    head.set_margin_start(8);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    content.append(&pickers);
    content.append(&legend);
    content.append(&status);
    content.append(&head);
    content.append(&scroll);
    win.set_child(Some(&content));

    // Ctrl+W で閉じる
    let key = gtk4::EventControllerKey::new();
    let w = win.clone();
    key.connect_key_pressed(move |_c, k, _code, state| {
        if k == gtk4::gdk::Key::w && state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            w.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    win.add_controller(key);
    win.present();
}
