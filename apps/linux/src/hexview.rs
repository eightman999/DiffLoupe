//! Hex バイナリ比較ビュー。16バイト/行でオフセット・Hex・ASCII を並べ、
//! 差分バイトを強調する(読み取り専用)。

use std::path::Path;
use std::rc::Rc;

use diffloupe_core::hex_diff::{self, HexComparison, HexRow};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::strings as s;

const DIFF_BG: &str = "#a45200"; // 差分バイトの背景(両テーマで読める暗めオレンジ)

fn fmt_size(n: i64) -> String {
    if n < 0 {
        return s::HEX_UNKNOWN.to_string();
    }
    format!("{n} bytes")
}

pub struct HexView {
    pub widget: gtk4::Box,
    header: gtk4::Label,
    warn: gtk4::Label,
    store: gio::ListStore,
    empty: gtk4::Label,
}

fn byte_span(b: u8, diff: bool) -> String {
    let h = format!("{b:02x} ");
    if diff {
        format!("<span background=\"{DIFF_BG}\" foreground=\"white\">{h}</span>")
    } else {
        h
    }
}

fn ascii_span(c: char, diff: bool) -> String {
    let t = glib::markup_escape_text(&c.to_string());
    if diff {
        format!("<span background=\"{DIFF_BG}\" foreground=\"white\">{t}</span>")
    } else {
        t.to_string()
    }
}

fn row_markup(row: &HexRow) -> String {
    let diffs: std::collections::HashSet<usize> =
        row.differing_byte_offsets.iter().copied().collect();
    let mut out = format!("{:08x}  ", row.offset);
    let cell = |bytes: &Option<Vec<u8>>, ascii: &Option<String>| -> (String, String) {
        let mut hex_part = String::new();
        let mut ascii_part = String::new();
        match (bytes, ascii) {
            (Some(bs), Some(as_)) => {
                for (i, b) in bs.iter().enumerate() {
                    hex_part.push_str(&byte_span(*b, diffs.contains(&i)));
                }
                for (i, c) in as_.chars().enumerate() {
                    ascii_part.push_str(&ascii_span(c, diffs.contains(&i)));
                }
                // 末尾行が16バイト未満の場合はパディング
                for _ in bs.len()..16 {
                    hex_part.push_str("   ");
                }
            }
            _ => {
                for _ in 0..16 {
                    hex_part.push_str(s::HEX_MISSING);
                    hex_part.push(' ');
                }
                for _ in 0..16 {
                    ascii_part.push(' ');
                }
            }
        }
        (hex_part, ascii_part)
    };
    let (lh, la) = cell(&row.left_bytes, &row.left_ascii);
    let (rh, ra) = cell(&row.right_bytes, &row.right_ascii);
    out.push_str(&lh);
    out.push_str("  ");
    out.push_str(&rh);
    out.push_str("  ");
    out.push_str(&la);
    out.push_str("  ");
    out.push_str(&ra);
    out
}

impl HexView {
    pub fn new() -> Rc<Self> {
        let header = gtk4::Label::new(None);
        header.set_xalign(0.0);
        header.set_margin_start(8);
        header.set_margin_top(4);

        let warn = gtk4::Label::new(None);
        warn.set_xalign(0.0);
        warn.set_margin_start(8);
        warn.add_css_class("dl-empty");
        warn.set_visible(false);

        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk4::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let l = gtk4::Label::new(None);
            l.set_xalign(0.0);
            l.set_use_markup(true);
            l.set_selectable(false);
            l.add_css_class("dl-mono");
            item.set_child(Some(&l));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let Some(any) = item
                .item()
                .and_then(|o| o.downcast::<glib::BoxedAnyObject>().ok())
            else {
                return;
            };
            let Some(lbl) = item.child().and_then(|w| w.downcast::<gtk4::Label>().ok()) else {
                return;
            };
            let markup = any.borrow::<String>().clone();
            lbl.set_markup(&markup);
        });
        let sel = gtk4::NoSelection::new(Some(store.clone()));
        let list = gtk4::ListView::new(Some(sel), Some(factory));
        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_vexpand(true);

        let empty = gtk4::Label::new(None);
        empty.add_css_class("dl-empty");
        empty.set_visible(false);

        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.append(&header);
        widget.append(&warn);
        widget.append(&empty);
        widget.append(&scroll);
        widget.set_vexpand(true);
        widget.set_hexpand(true);

        Rc::new(Self {
            widget,
            header,
            warn,
            store,
            empty,
        })
    }

    /// 左右ファイルを比較して表示する。失敗時はエラーメッセージを表示。
    pub fn load(&self, left: Option<&Path>, right: Option<&Path>, max_bytes: usize) {
        let cmp: HexComparison = match (left, right) {
            (Some(l), Some(r)) => match hex_diff::compare_urls(l, r, max_bytes) {
                Ok(c) => c,
                Err(e) => {
                    self.show_message(&s::msg_load_error(&e.to_string()));
                    return;
                }
            },
            (Some(_), None) | (None, Some(_)) => {
                let p = left.or(right).unwrap();
                let data = std::fs::read(p).unwrap_or_default();
                if left.is_some() {
                    hex_diff::compare(&data, &[], max_bytes)
                } else {
                    hex_diff::compare(&[], &data, max_bytes)
                }
            }
            _ => {
                self.show_message(s::MSG_NO_TARGET);
                return;
            }
        };
        self.empty.set_visible(false);
        let compared = cmp.left_data.len().min(cmp.right_data.len());
        self.header.set_text(&format!(
            "左: {}   右: {}   {}",
            fmt_size(cmp.left_total_size),
            fmt_size(cmp.right_total_size),
            s::hex_compared(compared)
        ));
        if cmp.truncated {
            self.warn.set_text(&s::hex_truncated("8 MiB"));
            self.warn.set_visible(true);
        } else {
            self.warn.set_visible(false);
        }
        self.store.remove_all();
        for i in 0..cmp.row_count() {
            self.store
                .append(&glib::BoxedAnyObject::new(row_markup(&cmp.row(i))));
        }
        if cmp.is_equal() && cmp.row_count() == 0 {
            self.show_message(s::MSG_IDENTICAL);
        }
    }

    pub fn show_message(&self, msg: &str) {
        self.store.remove_all();
        self.header.set_text("");
        self.warn.set_visible(false);
        self.empty.set_text(msg);
        self.empty.set_visible(true);
    }
}
