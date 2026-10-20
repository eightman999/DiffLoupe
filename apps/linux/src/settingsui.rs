//! 設定ウィンドウ。変更は即時 `Settings` に反映して保存し、次回比較から有効。
//! セクション: 比較 / フィルタ / 内容比較 / リネーム検出 / 自動再比較 / エディション
//! (+ エディション拡張セクション)。

use std::rc::Rc;

use diffloupe_core::FILTER_PRESETS;
use gtk4::prelude::*;

use crate::edition_ext::GtkEditionExtension;
use crate::model::AppModel;
use crate::strings as s;

fn section(title: &str) -> (gtk4::Box, gtk4::Label) {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    t.add_css_class("heading");
    t.set_margin_top(10);
    b.append(&t);
    (b, t)
}

fn switch_row(label: &str, active: bool, on: impl Fn(bool) + 'static) -> gtk4::CheckButton {
    let c = gtk4::CheckButton::with_label(label);
    c.set_active(active);
    c.connect_toggled(move |b| on(b.is_active()));
    c
}

pub fn open(
    parent: &impl IsA<gtk4::Window>,
    model: &Rc<AppModel>,
    ext: &Rc<dyn GtkEditionExtension>,
) {
    let win = gtk4::Window::new();
    win.set_title(Some(s::SETTINGS_TITLE));
    win.set_transient_for(Some(parent));
    win.set_modal(true);
    win.set_default_size(560, 640);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_bottom(12);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&content));
    scroll.set_vexpand(true);
    win.set_child(Some(&scroll));

    let st = model.settings.borrow().clone();

    // ── 比較 ──
    let (sec, _) = section(s::SEC_COMPARE);
    sec.append(&switch_row(s::SET_RECURSIVE, st.recursive, {
        let m = model.clone();
        move |v| {
            m.settings.borrow_mut().recursive = v;
            m.settings.borrow().save();
        }
    }));
    sec.append(&switch_row(
        s::SET_DEFAULT_EXCLUDES,
        st.use_default_excludes,
        {
            let m = model.clone();
            move |v| {
                m.settings.borrow_mut().use_default_excludes = v;
                m.settings.borrow().save();
            }
        },
    ));
    content.append(&sec);

    // ── フィルタ ──
    let (sec, _) = section(s::SEC_FILTER);
    let mk_patterns = |label: &str, text: String, is_exclude: bool| {
        let l = gtk4::Label::new(Some(label));
        l.set_xalign(0.0);
        let buf = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
        buf.set_text(&text);
        let tv = gtk4::TextView::with_buffer(&buf);
        tv.set_monospace(true);
        tv.set_size_request(-1, 90);
        let frame = gtk4::Frame::new(None);
        frame.set_child(Some(&tv));
        sec.append(&l);
        sec.append(&frame);
        let m = model.clone();
        buf.connect_changed(move |b| {
            let t = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
            if is_exclude {
                m.settings.borrow_mut().user_exclude_patterns = t;
            } else {
                m.settings.borrow_mut().user_include_patterns = t;
            }
            m.settings.borrow().save();
        });
    };
    mk_patterns(s::SET_USER_EXCLUDE, st.user_exclude_patterns.clone(), true);
    mk_patterns(s::SET_USER_INCLUDE, st.user_include_patterns.clone(), false);

    let presets_label = gtk4::Label::new(Some(s::SET_PRESETS));
    presets_label.set_xalign(0.0);
    sec.append(&presets_label);
    for preset in FILTER_PRESETS {
        let id = preset.id.to_string();
        let active = st.filter_presets.iter().any(|p| p == preset.id);
        sec.append(&switch_row(preset.display_name, active, {
            let m = model.clone();
            move |v| {
                let mut s = m.settings.borrow_mut();
                if v {
                    if !s.filter_presets.contains(&id) {
                        s.filter_presets.push(id.clone());
                    }
                } else {
                    s.filter_presets.retain(|p| p != &id);
                }
                s.save();
            }
        }));
    }
    content.append(&sec);

    // ── 内容比較 ──
    let (sec, _) = section(s::SEC_CONTENT);
    let bind_flag = |label: &'static str,
                     get: fn(&diffloupe_core::ContentComparison) -> bool,
                     set: fn(&mut diffloupe_core::ContentComparison, bool)| {
        switch_row(label, get(&st.content), {
            let m = model.clone();
            move |v| {
                let mut s = m.settings.borrow_mut();
                set(&mut s.content, v);
                s.save();
            }
        })
    };
    sec.append(&bind_flag(
        s::SET_IGNORE_EOL,
        |c| c.ignore_line_endings,
        |c, v| c.ignore_line_endings = v,
    ));
    sec.append(&bind_flag(
        s::SET_IGNORE_TRAIL_WS,
        |c| c.ignore_trailing_whitespace,
        |c, v| c.ignore_trailing_whitespace = v,
    ));
    sec.append(&bind_flag(
        s::SET_NORMALIZE_WS,
        |c| c.normalize_whitespace,
        |c, v| c.normalize_whitespace = v,
    ));
    sec.append(&bind_flag(
        s::SET_IGNORE_CASE,
        |c| c.ignore_case,
        |c, v| c.ignore_case = v,
    ));
    // 構造化モード
    {
        use diffloupe_core::StructuredComparisonMode::*;
        let modes = [
            (s::STRUCT_DISABLED, Disabled),
            (s::STRUCT_AUTOMATIC, Automatic),
            ("JSON", Json),
            ("XML", Xml),
            ("CSV/TSV", Csv),
        ];
        let names: Vec<&str> = modes.iter().map(|(n, _)| *n).collect();
        let dd = gtk4::DropDown::from_strings(&names);
        dd.set_selected(
            modes
                .iter()
                .position(|(_, m)| *m == st.content.structured_mode)
                .unwrap_or(0) as u32,
        );
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row.append(&gtk4::Label::new(Some(s::SET_STRUCTURED)));
        row.append(&dd);
        sec.append(&row);
        let m = model.clone();
        dd.connect_selected_notify(move |d| {
            let mode = modes[d.selected() as usize].1;
            let mut s = m.settings.borrow_mut();
            s.content.structured_mode = mode;
            s.save();
        });
    }
    content.append(&sec);

    // ── リネーム検出 ──
    let (sec, _) = section(s::SEC_RENAME);
    sec.append(&switch_row(s::SET_DETECT_RENAMES, st.detect_renames, {
        let m = model.clone();
        move |v| {
            m.settings.borrow_mut().detect_renames = v;
            m.settings.borrow().save();
        }
    }));
    content.append(&sec);

    // ── 自動再比較 ──
    let (sec, _) = section(s::SEC_WATCH);
    sec.append(&switch_row(s::SET_AUTO_REFRESH, st.auto_refresh, {
        let m = model.clone();
        move |v| {
            {
                let mut s = m.settings.borrow_mut();
                s.auto_refresh = v;
                s.save();
            }
            // 再ON で一時停止から復帰
            m.reset_auto_runs();
            m.configure_watcher();
        }
    }));
    content.append(&sec);

    // ── エディション ──
    let (sec, _) = section(s::SEC_EDITION);
    sec.append(&gtk4::Label::new(Some(model.edition.name())));
    let limit_line = match model.edition.max_folder_entries() {
        Some(n) => s::edition_limit_line(n),
        None => s::EDITION_UNLIMITED.to_string(),
    };
    let ll = gtk4::Label::new(Some(&limit_line));
    ll.set_xalign(0.0);
    ll.add_css_class("dl-empty");
    sec.append(&ll);
    content.append(&sec);

    // ── エディション拡張セクション(Pro) ──
    if model.edition.capabilities().extra_settings_section {
        if let Some(w) = ext.settings_section(model) {
            content.append(&w);
        }
    }

    win.present();
}
