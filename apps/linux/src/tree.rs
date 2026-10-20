//! 結果ツリー。`DiffNode` → `gio::ListStore`(BoxedAnyObject)の階層を作り、
//! `TreeListModel` + `TreeExpander` + `ListView` で表示する。
//! コールバックは `set_on_select` / `set_on_context` で後から結線する
//! (ウィンドウ構築時の循環参照を避けるため)。

use std::cell::RefCell;
use std::rc::Rc;

use diffloupe_core::{DiffNode, DiffStatus};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::model::status_badge;
use crate::strings as s;

/// ツリー1行分のデータ。子は別 ListStore(TreeListModel が再帰する)。
pub struct TreeItem {
    pub rel_path: String,
    pub name: String,
    pub is_dir: bool,
    pub status: DiffStatus,
    pub children: Option<gio::ListStore>,
}

fn store_from(node: &DiffNode) -> gio::ListStore {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    if let Some(children) = &node.children {
        for c in children {
            store.append(&glib::BoxedAnyObject::new(TreeItem {
                rel_path: c.id.clone(),
                name: c.name.clone(),
                is_dir: c.is_directory,
                status: c.status,
                children: c.children.as_ref().map(|_| store_from(c)),
            }));
        }
    }
    store
}

fn status_css(status: DiffStatus) -> &'static str {
    match status {
        DiffStatus::Identical => "dl-badge-identical",
        DiffStatus::Different => "dl-badge-different",
        DiffStatus::LeftOnly => "dl-badge-leftonly",
        DiffStatus::RightOnly => "dl-badge-rightonly",
        DiffStatus::Error => "dl-badge-error",
    }
}

type SelectCb = Rc<dyn Fn(String)>;
type ContextCb = Rc<dyn Fn(String, f64, f64, gtk4::Widget)>;

/// 結果ツリーのウィジェット一式。
pub struct ResultTree {
    pub root_widget: gtk4::Stack,
    list: gtk4::ListView,
    store_top: RefCell<Option<gio::ListStore>>,
    on_select: RefCell<Option<SelectCb>>,
    on_context: RefCell<Option<ContextCb>>,
}

impl ResultTree {
    pub fn new() -> Rc<Self> {
        let factory = gtk4::SignalListItemFactory::new();
        // setup 時点では self が未完成なので Weak の置き場を後で埋める
        let self_slot: Rc<RefCell<Option<std::rc::Weak<ResultTree>>>> = Rc::new(RefCell::new(None));

        let setup_slot = self_slot.clone();
        factory.connect_setup(move |_, item| {
            let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let expander = gtk4::TreeExpander::new();
            let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let icon = gtk4::Image::new();
            let name = gtk4::Label::new(None);
            name.set_xalign(0.0);
            name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            let badge = gtk4::Label::new(None);
            badge.add_css_class("dl-badge");
            row_box.append(&icon);
            row_box.append(&name);
            row_box.append(&badge);
            expander.set_child(Some(&row_box));
            expander.set_margin_top(2);
            expander.set_margin_bottom(2);

            // 右クリック: bind で更新されるパスを共有する
            let path_rc = Rc::new(RefCell::new(String::new()));
            let click = gtk4::GestureClick::new();
            click.set_button(3);
            {
                let path_rc = path_rc.clone();
                let widget_ref = row_box.clone();
                let self_slot = setup_slot.clone();
                click.connect_released(move |_g, _n, x, y| {
                    let p = path_rc.borrow().clone();
                    let Some(this) = self_slot.borrow().as_ref().and_then(|w| w.upgrade()) else {
                        return;
                    };
                    if !p.is_empty() {
                        this.emit_context(p, x, y, widget_ref.clone().upcast());
                    }
                });
            }
            row_box.add_controller(click);
            unsafe {
                row_box.set_data("dl-path-rc", path_rc);
            }
            item.set_child(Some(&expander));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let Some(expander) = item
                .child()
                .and_then(|w| w.downcast::<gtk4::TreeExpander>().ok())
            else {
                return;
            };
            let Some(row) = item
                .item()
                .and_then(|o| o.downcast::<gtk4::TreeListRow>().ok())
            else {
                return;
            };
            expander.set_list_row(Some(&row));
            let Some(any) = row
                .item()
                .and_then(|o| o.downcast::<glib::BoxedAnyObject>().ok())
            else {
                return;
            };
            let data = any.borrow::<TreeItem>();
            let Some(row_box) = expander
                .child()
                .and_then(|w| w.downcast::<gtk4::Box>().ok())
            else {
                return;
            };
            let mut w = row_box.first_child();
            let mut next = || {
                let cur = w.take();
                w = cur.as_ref().and_then(|c| c.next_sibling());
                cur
            };
            let (Some(icon), Some(name_lbl), Some(badge_lbl)) = (
                next().and_then(|x| x.downcast::<gtk4::Image>().ok()),
                next().and_then(|x| x.downcast::<gtk4::Label>().ok()),
                next().and_then(|x| x.downcast::<gtk4::Label>().ok()),
            ) else {
                return;
            };
            icon.set_icon_name(Some(if data.is_dir {
                "folder-symbolic"
            } else {
                "text-x-generic-symbolic"
            }));
            name_lbl.set_text(&data.name);
            badge_lbl.set_text(status_badge(data.status, data.is_dir));
            for cls in [
                "dl-badge-identical",
                "dl-badge-different",
                "dl-badge-leftonly",
                "dl-badge-rightonly",
                "dl-badge-error",
            ] {
                badge_lbl.remove_css_class(cls);
            }
            badge_lbl.add_css_class(status_css(data.status));
            unsafe {
                if let Some(rc) = row_box
                    .data::<Rc<RefCell<String>>>("dl-path-rc")
                    .map(|p| p.as_ref().clone())
                {
                    *rc.borrow_mut() = data.rel_path.clone();
                }
            }
        });

        let empty = gio::ListStore::new::<glib::BoxedAnyObject>();
        let tree_model = gtk4::TreeListModel::new(
            empty.clone().upcast::<gio::ListModel>(),
            false,
            false,
            |_| None,
        );
        let selection = gtk4::SingleSelection::new(Some(tree_model));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        let list = gtk4::ListView::new(Some(selection), Some(factory));
        list.set_show_separators(false);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&list));
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);

        let stack = gtk4::Stack::new();
        let ph = gtk4::Label::new(Some(s::EMPTY_RESULT));
        ph.add_css_class("dl-empty");
        ph.set_vexpand(true);
        stack.add_named(&ph, Some("empty"));
        stack.add_named(&scrolled, Some("tree"));
        stack.set_visible_child_name("empty");
        stack.set_vexpand(true);
        stack.set_hexpand(true);

        let this = Rc::new(Self {
            root_widget: stack,
            list,
            store_top: RefCell::new(None),
            on_select: RefCell::new(None),
            on_context: RefCell::new(None),
        });
        *self_slot.borrow_mut() = Some(Rc::downgrade(&this));
        this
    }

    fn emit_select(&self, path: String) {
        if let Some(cb) = self.on_select.borrow().as_ref() {
            cb(path);
        }
    }
    fn emit_context(&self, path: String, x: f64, y: f64, w: gtk4::Widget) {
        if let Some(cb) = self.on_context.borrow().as_ref() {
            cb(path, x, y, w);
        }
    }

    pub fn set_on_select(&self, cb: SelectCb) {
        *self.on_select.borrow_mut() = Some(cb);
    }
    pub fn set_on_context(&self, cb: ContextCb) {
        *self.on_context.borrow_mut() = Some(cb);
    }

    /// フィルタ済みエントリからツリーを再構築して表示する。
    /// `empty_message` は結果ゼロ時の表示文字列。
    pub fn set_result(self: &Rc<Self>, root: &DiffNode, empty_message: &'static str) {
        let top = store_from(root);
        let n = top.n_items();
        let model = gtk4::TreeListModel::new(
            top.clone().upcast::<gio::ListModel>(),
            false,
            false,
            |item| {
                item.downcast_ref::<glib::BoxedAnyObject>().and_then(|any| {
                    any.borrow::<TreeItem>()
                        .children
                        .clone()
                        .map(|s| s.upcast::<gio::ListModel>())
                })
            },
        );
        let sel = gtk4::SingleSelection::new(Some(model));
        sel.set_autoselect(false);
        sel.set_can_unselect(true);
        let weak = Rc::downgrade(self);
        sel.connect_selection_changed(move |sel, _, _| {
            let Some(this) = weak.upgrade() else { return };
            let Some(row) = sel
                .selected_item()
                .and_then(|o| o.downcast::<gtk4::TreeListRow>().ok())
            else {
                return;
            };
            if let Some(any) = row
                .item()
                .and_then(|o| o.downcast::<glib::BoxedAnyObject>().ok())
            {
                let path = any.borrow::<TreeItem>().rel_path.clone();
                this.emit_select(path);
            }
        });
        self.list.set_model(Some(&sel));
        *self.store_top.borrow_mut() = Some(top);
        if n == 0 {
            if let Some(page) = self.root_widget.child_by_name("empty") {
                if let Some(lbl) = page.downcast_ref::<gtk4::Label>() {
                    lbl.set_text(empty_message);
                }
            }
            self.root_widget.set_visible_child_name("empty");
        } else {
            self.root_widget.set_visible_child_name("tree");
        }
    }

    pub fn clear(&self) {
        let empty = gio::ListStore::new::<glib::BoxedAnyObject>();
        *self.store_top.borrow_mut() = Some(empty.clone());
        let model = gtk4::TreeListModel::new(
            empty.clone().upcast::<gio::ListModel>(),
            false,
            false,
            |_| None,
        );
        let sel = gtk4::SingleSelection::new(Some(model));
        self.list.set_model(Some(&sel));
        self.root_widget.set_visible_child_name("empty");
    }
}
