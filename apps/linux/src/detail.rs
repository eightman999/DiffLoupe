//! 詳細ペイン。選択エントリのメタデータヘッダ + テキスト/Hex/画像ビューの切替。
//! Pro の textDetailView 拡張は capability フラグ経由で差し替えられる設計
//! (Free では常に従来ビュー)。

use std::path::Path;
use std::rc::Rc;

use diffloupe_core::structured::{self, StructuredComparisonMode};
use diffloupe_core::{ComparedEntry, DiffStatus};
use gtk4::glib;
use gtk4::prelude::*;

use crate::hexview::HexView;
use crate::imageops;
use crate::imageview::ImageView;
use crate::model::AppModel;
use crate::strings as s;
use crate::textdiff::{TextDiffView, TextLoad};

pub struct DetailPane {
    pub widget: gtk4::Box,
    stack: gtk4::Stack,
    pub text: Rc<TextDiffView>,
    pub hex: Rc<HexView>,
    pub image: Rc<ImageView>,
    title: gtk4::Label,
    enc_label: gtk4::Label,
    meta_label: gtk4::Label,
    rename_label: gtk4::Label,
    structured_label: gtk4::Label,
    message: gtk4::Label,
    /// エディション拡張ビューのファクトリ(既定は常に None)。
    edition_text_view: fn(&Rc<AppModel>, &ComparedEntry) -> Option<gtk4::Widget>,
}

fn fmt_size(v: i64) -> String {
    if v < 0 {
        s::DASH.to_string()
    } else {
        imageops::human_size(v as u64)
    }
}

fn fmt_mtime(v: Option<f64>) -> String {
    match v {
        Some(t) => glib::DateTime::from_unix_local(t as i64)
            .ok()
            .and_then(|d| d.format("%Y-%m-%d %H:%M:%S").ok().map(|v| v.to_string()))
            .unwrap_or_else(|| s::DASH.to_string()),
        None => s::DASH.to_string(),
    }
}

fn fmt_perm(e: &ComparedEntry, left: bool) -> String {
    let m = if left { &e.left_posix } else { &e.right_posix };
    m.as_ref()
        .and_then(|p| p.permissions)
        .map(|p| format!("{p:04o}"))
        .unwrap_or_else(|| s::DASH.to_string())
}

/// 構造化正規化のバッジ情報(左右それぞれ normalize して通知を出す)。
fn structured_badge(
    left: &Path,
    right: &Path,
    mode: StructuredComparisonMode,
) -> Option<(String, String)> {
    if matches!(mode, StructuredComparisonMode::Disabled) {
        return None;
    }
    let ldata = std::fs::read(left).unwrap_or_default();
    let rdata = std::fs::read(right).unwrap_or_default();
    let ln = structured::normalize(&ldata, Some(&left.to_string_lossy()), mode);
    let rn = structured::normalize(&rdata, Some(&right.to_string_lossy()), mode);
    let fmt = ln.format.or(rn.format);
    match (fmt, ln.did_normalize, rn.did_normalize) {
        (Some(f), _, _) if ln.did_normalize || rn.did_normalize => {
            let name = f.display_name();
            Some((s::structured_badge(name), s::structured_badge_tooltip(name)))
        }
        _ => {
            let notice = [ln.notice, rn.notice]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" / ");
            let label = if notice.is_empty() {
                s::STRUCTURED_FAIL_BADGE.to_string()
            } else {
                format!("{} ({})", s::STRUCTURED_FAIL_BADGE, notice)
            };
            Some((label, s::STRUCTURED_FAIL_TOOLTIP.to_string()))
        }
    }
}

impl DetailPane {
    /// `edition_text_view` はウィンドウ側で `GtkEditionExtension` を束ねて渡す。
    pub fn new() -> Rc<Self> {
        let title = gtk4::Label::new(Some(s::DETAIL_HEADER));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        let enc_label = gtk4::Label::new(None);
        enc_label.set_xalign(0.0);
        let meta_label = gtk4::Label::new(None);
        meta_label.set_xalign(0.0);
        meta_label.set_selectable(true);
        let rename_label = gtk4::Label::new(None);
        rename_label.set_xalign(0.0);
        rename_label.set_visible(false);
        let structured_label = gtk4::Label::new(None);
        structured_label.set_xalign(0.0);
        structured_label.set_visible(false);
        structured_label.add_css_class("dl-badge");
        structured_label.add_css_class("dl-badge-leftonly");

        let header_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        header_box.add_css_class("dl-meta");
        header_box.append(&title);
        header_box.append(&enc_label);
        header_box.append(&meta_label);
        header_box.append(&rename_label);
        header_box.append(&structured_label);

        let text = TextDiffView::new();
        let hex = HexView::new();
        let image = ImageView::new();
        let message = gtk4::Label::new(Some(s::MSG_SELECT_FILE));
        message.add_css_class("dl-empty");
        message.set_vexpand(true);

        let stack = gtk4::Stack::new();
        stack.add_named(&message, Some("message"));
        stack.add_named(&text.widget, Some("text"));
        stack.add_named(&hex.widget, Some("hex"));
        stack.add_named(&image.widget, Some("image"));
        stack.set_visible_child_name("message");
        stack.set_vexpand(true);

        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.append(&header_box);
        widget.append(&stack);
        widget.set_vexpand(true);
        widget.set_hexpand(true);

        Rc::new(Self {
            widget,
            stack,
            text,
            hex,
            image,
            title,
            enc_label,
            meta_label,
            rename_label,
            structured_label,
            message,
            edition_text_view: |_m, _e| None,
        })
    }

    /// スモークテスト用: 画像2枚を直接読み込んで画像比較ページを表示する。
    pub(crate) fn show_image_paths(&self, l: &std::path::Path, r: &std::path::Path) {
        self.image.load(l, r);
        self.stack.set_visible_child_name("image");
    }

    pub fn clear(&self) {
        self.text.clear();
        self.title.set_text(s::DETAIL_HEADER);
        self.enc_label.set_text("");
        self.meta_label.set_text("");
        self.rename_label.set_visible(false);
        self.structured_label.set_visible(false);
        self.message.set_text(s::MSG_SELECT_FILE);
        self.stack.set_visible_child_name("message");
    }

    /// エントリ選択時の描画。`entry` の左右パスは比較時のルート(git ref なら
    /// スナップショット内)を指す。
    pub fn show_entry(&self, entry: &ComparedEntry, model: &Rc<AppModel>) {
        self.title.set_text(&entry.relative_path);

        // メタデータヘッダ
        self.meta_label.set_markup(&format!(
            "{}:  左 {} / 右 {}\n{}:  左 {} / 右 {}\n{}:  左 {} / 右 {}",
            s::META_SIZE,
            fmt_size(entry.left_size),
            fmt_size(entry.right_size),
            s::META_MTIME,
            fmt_mtime(entry.left_modification_date),
            fmt_mtime(entry.right_modification_date),
            s::META_PERM,
            fmt_perm(entry, true),
            fmt_perm(entry, false),
        ));

        // リネーム候補バー
        if let Some(rc) = &entry.rename_candidate {
            let label = if rc.is_unique_match {
                s::RENAME_CONFIRMED
            } else {
                s::RENAME_CANDIDATE
            };
            let extra = if rc.total_matches > 1 {
                format!(" {}", s::rename_same_count(rc.total_matches))
            } else {
                String::new()
            };
            self.rename_label
                .set_text(&format!("{label}: → {}{}", rc.counterpart_path, extra));
            self.rename_label.set_visible(true);
        } else {
            self.rename_label.set_visible(false);
        }
        self.structured_label.set_visible(false);
        self.enc_label.set_text("");

        if entry.is_directory {
            self.message.set_text(s::MSG_IS_DIRECTORY);
            self.stack.set_visible_child_name("message");
            return;
        }
        match entry.status {
            DiffStatus::LeftOnly => {
                // 片側のみでも内容確認は可能(右側は空扱い)
                self.show_file(entry, model);
            }
            DiffStatus::RightOnly => {
                self.show_file(entry, model);
            }
            DiffStatus::Error => {
                let msg = entry.error_message.as_deref().unwrap_or("不明");
                self.message.set_text(&s::msg_load_error(msg));
                self.stack.set_visible_child_name("message");
            }
            _ => self.show_file(entry, model),
        }
    }

    /// エディション拡張の差し替えビューを返すページがあれば表示して true。
    fn try_edition_view(&self, entry: &ComparedEntry, model: &Rc<AppModel>) -> bool {
        if !model.edition.capabilities().text_detail_view {
            return false;
        }
        let Some(w) = (self.edition_text_view)(model, entry) else {
            return false;
        };
        if self.stack.child_by_name("edition").is_none() {
            self.stack.add_named(&w, Some("edition"));
        }
        self.stack.set_visible_child_name("edition");
        true
    }

    fn show_file(&self, entry: &ComparedEntry, model: &Rc<AppModel>) {
        if self.try_edition_view(entry, model) {
            return;
        }
        let lp = entry.left_path.clone();
        let rp = entry.right_path.clone();
        let cc = model.settings.borrow().content;

        // 両側が画像拡張子なら画像比較ビュー
        if let (Some(l), Some(r)) = (&lp, &rp) {
            if imageops::is_image_path(l) && imageops::is_image_path(r) {
                self.enc_label.set_text("");
                self.image.load(l, r);
                self.stack.set_visible_child_name("image");
                return;
            }
        }

        let left_exists = lp.as_ref().map(|p| p.exists()).unwrap_or(false);
        let right_exists = rp.as_ref().map(|p| p.exists()).unwrap_or(false);
        match (left_exists, right_exists) {
            (true, false) => {
                self.message.set_text(s::MSG_LEFT_ONLY);
                self.stack.set_visible_child_name("message");
            }
            (false, true) => {
                self.message.set_text(s::MSG_RIGHT_ONLY);
                self.stack.set_visible_child_name("message");
            }
            (false, false) => {
                self.message.set_text(s::MSG_NO_TARGET);
                self.stack.set_visible_child_name("message");
            }
            (true, true) => {
                let (l, r) = (lp.unwrap(), rp.unwrap());
                // 構造化モードのバッジ
                if let Some((label, tip)) = structured_badge(&l, &r, cc.structured_mode) {
                    self.structured_label.set_text(&label);
                    self.structured_label.set_tooltip_text(Some(&tip));
                    self.structured_label.set_visible(true);
                }
                let detail = self.text.load(&l, &r, &cc);
                self.enc_label.set_text(&format!(
                    "左: {}  /  右: {}",
                    detail.left_encoding.unwrap_or(s::DASH),
                    detail.right_encoding.unwrap_or(s::DASH)
                ));
                match detail.result {
                    TextLoad::Loaded => self.stack.set_visible_child_name("text"),
                    TextLoad::Binary => {
                        self.hex.load(
                            Some(&l),
                            Some(&r),
                            diffloupe_core::hex_diff::DEFAULT_BYTE_CAP,
                        );
                        self.stack.set_visible_child_name("hex");
                    }
                    TextLoad::TooLarge => {
                        self.message.set_text(s::MSG_TOO_LARGE);
                        self.stack.set_visible_child_name("message");
                    }
                    TextLoad::Io(e) => {
                        self.message.set_text(&s::msg_load_error(&e));
                        self.stack.set_visible_child_name("message");
                    }
                }
            }
        }
    }
}
