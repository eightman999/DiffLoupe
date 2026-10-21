//! コピー計画プレビュー(読み取り専用)。`SyncPlan::make` の結果を表にする。
//! 実行系の操作は Pro の `GtkEditionExtension::sync_plan_actions` が供給する
//! (Free では領域なし)。

use std::rc::Rc;

use diffloupe_core::sync_plan::{SyncDirection, SyncItem, SyncPlan};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::edition_ext::GtkEditionExtension;
use crate::imageops::human_size;
use crate::model::AppModel;
use crate::strings as s;

fn action_name(item: &SyncItem) -> &'static str {
    use diffloupe_core::sync_plan::SyncAction::*;
    match item.action {
        CopyFile => "コピー",
        CreateDirectory => "ディレクトリ作成",
        Conflict => "差分(上書き候補)",
        PreservedDestination => "保持",
        Blocked => "エラー",
    }
}

fn action_css(item: &SyncItem) -> &'static str {
    use diffloupe_core::sync_plan::SyncAction::*;
    match item.action {
        CopyFile | CreateDirectory => "dl-badge-identical",
        Conflict => "dl-badge-different",
        PreservedDestination => "dl-badge-leftonly",
        Blocked => "dl-badge-error",
    }
}

/// 計画テーブル行
struct PlanRow {
    status: &'static str,
    status_css: &'static str,
    path: String,
    source: String,
    size: String,
    note: String,
}

fn to_row(item: &SyncItem) -> PlanRow {
    PlanRow {
        status: action_name(item),
        status_css: action_css(item),
        path: item.relative_path.clone(),
        source: item.source.clone().unwrap_or_default(),
        size: if item.bytes > 0 {
            human_size(item.bytes as u64)
        } else {
            s::DASH.to_string()
        },
        note: item.reason.clone(),
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
            if w > 0 {
                l.set_width_chars(w);
                l.set_max_width_chars(w);
            }
            l
        };
        let badge = mk(14);
        badge.add_css_class("dl-badge");
        let path = mk(28);
        let source = mk(30);
        let size = mk(10);
        let note = mk(0);
        note.set_hexpand(true);
        b.append(&badge);
        b.append(&path);
        b.append(&source);
        b.append(&size);
        b.append(&note);
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
        let row = any.borrow::<PlanRow>();
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
        for text in [&row.path, &row.source, &row.size, &row.note] {
            if let Some(l) = next() {
                l.set_text(text);
            }
        }
    });
    let sel = gtk4::NoSelection::new(Some(store.clone()));
    gtk4::ListView::new(Some(sel), Some(factory))
}

/// コピー計画シートを開く。
pub fn open(
    parent: &impl IsA<gtk4::Window>,
    model: &Rc<AppModel>,
    edition_ext: &Rc<dyn GtkEditionExtension>,
    direction: SyncDirection,
) {
    let Some(result) = model.result.borrow().clone() else {
        return;
    };

    let win = gtk4::Window::new();
    win.set_title(Some(s::PLAN_TITLE));
    win.set_transient_for(Some(parent));
    win.set_modal(true);
    win.set_destroy_with_parent(true);
    win.set_default_size(880, 540);

    let dir_dd = gtk4::DropDown::from_strings(&[s::DIR_LTR, s::DIR_RTL]);
    dir_dd.set_selected(match direction {
        SyncDirection::LeftToRight => 0,
        SyncDirection::RightToLeft => 1,
    });

    let summary = gtk4::Label::new(None);
    summary.set_xalign(0.0);
    let roots = gtk4::Label::new(None);
    roots.set_xalign(0.0);
    roots.add_css_class("dl-empty");

    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let list = build_list(&store);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_vexpand(true);

    let empty = gtk4::Label::new(Some(s::PLAN_EMPTY));
    empty.add_css_class("dl-empty");
    empty.set_visible(false);

    // ヘッダ列
    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    for (t, w) in [
        (s::PLAN_COL_STATUS, 14),
        (s::PLAN_COL_PATH, 28),
        (s::PLAN_COL_SOURCE, 30),
        (s::PLAN_COL_SIZE, 10),
    ] {
        let l = gtk4::Label::new(Some(t));
        l.set_xalign(0.0);
        l.set_width_chars(w);
        l.set_max_width_chars(w);
        l.add_css_class("heading");
        head.append(&l);
    }
    let l = gtk4::Label::new(Some(s::PLAN_COL_NOTE));
    l.set_xalign(0.0);
    l.add_css_class("heading");
    head.append(&l);
    head.set_margin_start(8);

    let actions_area = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bar.append(&gtk4::Label::new(Some(s::PLAN_DIRECTION)));
    bar.append(&dir_dd);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&spacer);
    let close = gtk4::Button::with_label(s::CLOSE);
    close.set_tooltip_text(Some(s::CLOSE_TOOLTIP));
    bar.append(&close);
    bar.set_margin_start(8);
    bar.set_margin_end(8);
    bar.set_margin_top(8);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    content.append(&bar);
    content.append(&roots);
    content.append(&summary);
    content.append(&head);
    content.append(&scroll);
    content.append(&empty);
    content.append(&actions_area);
    win.set_child(Some(&content));

    let fill = {
        let store = store.clone();
        let summary = summary.clone();
        let roots = roots.clone();
        let empty = empty.clone();
        let actions_area = actions_area.clone();
        let model = model.clone();
        let edition_ext = edition_ext.clone();
        move |dir: SyncDirection| {
            let plan = SyncPlan::make(&result, dir);
            roots.set_text(&format!("{} → {}", plan.source_root, plan.destination_root));
            summary.set_text(&format!(
                "{}   {}   {}   {}   {}   ({} {})",
                s::plan_copy(plan.summary.copy_files),
                s::plan_dirs(plan.summary.create_directories),
                s::plan_diffs(plan.summary.conflicts),
                s::plan_kept(plan.summary.preserved_destinations),
                s::plan_errors(plan.summary.blocked),
                human_size(plan.summary.total_bytes.max(0) as u64),
                s::META_SIZE
            ));
            store.remove_all();
            for item in &plan.items {
                store.append(&glib::BoxedAnyObject::new(to_row(item)));
            }
            // 案内は計画の項目が無いときだけ出す
            empty.set_visible(plan.items.is_empty());
            // エディション拡張の操作領域(Free では何も出さない)
            while let Some(c) = actions_area.first_child() {
                actions_area.remove(&c);
            }
            if let Some(w) = edition_ext.sync_plan_actions(&model, &plan) {
                actions_area.append(&w);
            }
        }
    };
    fill(direction);
    dir_dd.connect_selected_notify(move |dd| {
        fill(if dd.selected() == 0 {
            SyncDirection::LeftToRight
        } else {
            SyncDirection::RightToLeft
        });
    });
    let w = win.clone();
    close.connect_clicked(move |_| w.close());
    win.present();
}
