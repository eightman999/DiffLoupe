//! 画像比較ビュー。共有キャンバス(ズーム/パン共通)上で
//! 左右・重ね合わせ・ブリンク・差分・スライスの5モードを描画する。
//! 画像デコードは gdk-pixbuf、差分計算は imageops(Swift ImageDiffEngine 移植)。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::glib;
use gtk4::prelude::*;

use crate::imageops::{self, ImageDiffResult, RgbaImage};
use crate::strings as s;

const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 4.0;
const ZOOM_STEP: f64 = 1.25;
const BLINK_MS: u64 = 650;
const THRESHOLD_DEBOUNCE_MS: u64 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    SideBySide = 0,
    Overlay = 1,
    Blink = 2,
    Difference = 3,
    Slice = 4,
}

impl Mode {
    fn from_index(i: u32) -> Self {
        match i {
            1 => Mode::Overlay,
            2 => Mode::Blink,
            3 => Mode::Difference,
            4 => Mode::Slice,
            _ => Mode::SideBySide,
        }
    }
    const NAMES: &'static [&'static str] = &[
        s::IMG_SIDE_BY_SIDE,
        s::IMG_OVERLAY,
        s::IMG_BLINK,
        s::IMG_DIFFERENCE,
        s::IMG_SLICE,
    ];
}

/// 表示用に加工済みの画像一式。
struct Surfaces {
    left: cairo::ImageSurface,
    right: cairo::ImageSurface,
    diff: cairo::ImageSurface,
    diff_w: f64,
    diff_h: f64,
}

pub struct ImageView {
    pub widget: gtk4::Box,
    area: gtk4::DrawingArea,
    mode: Cell<Mode>,
    opacity: Cell<f64>,
    slice_pos: Cell<f64>,
    zoom: Cell<f64>,
    pan: Cell<(f64, f64)>,
    blink_phase: Cell<bool>,
    blink_source: RefCell<Option<glib::SourceId>>,
    threshold_src: RefCell<Option<glib::SourceId>>,
    drag_start_pan: Cell<(f64, f64)>,
    pinch_base_zoom: Cell<f64>,
    /// 再計算用のデコード済み画像とメタ情報の経路
    decoded: RefCell<Option<(RgbaImage, RgbaImage, PathBuf, PathBuf)>>,
    result: RefCell<Option<Rc<ImageDiffResult>>>,
    surfaces: RefCell<Option<Rc<Surfaces>>>,
    threshold_scale: gtk4::Scale,
    zoom_label: gtk4::Label,
    opacity_label: gtk4::Label,
    slice_row: gtk4::Box,
    meta_grid: gtk4::Grid,
    warn_label: gtk4::Label,
    empty_label: gtk4::Label,
}

impl ImageView {
    pub fn new() -> Rc<Self> {
        let area = gtk4::DrawingArea::new();
        area.set_vexpand(true);
        area.set_hexpand(true);
        area.set_content_width(400);
        area.set_content_height(300);

        // ツールバー(狭いウィンドウ用に横スクロール可能)
        let toolbar_inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        toolbar_inner.add_css_class("dl-img-toolbar");
        let toolbar_scroll = gtk4::ScrolledWindow::new();
        toolbar_scroll.set_child(Some(&toolbar_inner));
        toolbar_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);

        let mode_dd = gtk4::DropDown::from_strings(Mode::NAMES);
        toolbar_inner.append(&labeled(s::IMG_MODE_LABEL, &mode_dd));

        let opacity_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.05);
        opacity_scale.set_value(0.5);
        opacity_scale.set_size_request(110, -1);
        let opacity_label = gtk4::Label::new(Some("50%"));
        let op_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        op_box.append(&gtk4::Label::new(Some(s::IMG_OPACITY)));
        op_box.append(&opacity_scale);
        op_box.append(&opacity_label);
        toolbar_inner.append(&op_box);

        let threshold_scale =
            gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 255.0, 1.0);
        threshold_scale.set_value(imageops::DEFAULT_THRESHOLD as f64);
        threshold_scale.set_size_request(110, -1);
        threshold_scale.set_digits(0);
        let th_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        th_box.append(&gtk4::Label::new(Some(s::IMG_THRESHOLD)));
        th_box.append(&threshold_scale);
        let th_val = gtk4::Label::new(Some(&imageops::DEFAULT_THRESHOLD.to_string()));
        th_box.append(&th_val);
        toolbar_inner.append(&th_box);

        let slice_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        let slice_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.01);
        slice_scale.set_value(0.5);
        slice_scale.set_size_request(110, -1);
        let slice_val = gtk4::Label::new(Some("50%"));
        slice_row.append(&gtk4::Label::new(Some(s::IMG_BOUNDARY)));
        slice_row.append(&slice_scale);
        slice_row.append(&slice_val);
        slice_row.set_visible(false);
        toolbar_inner.append(&slice_row);

        let zoom_out = gtk4::Button::from_icon_name("zoom-out-symbolic");
        zoom_out.set_tooltip_text(Some(s::IMG_ZOOM_OUT));
        let zoom_in = gtk4::Button::from_icon_name("zoom-in-symbolic");
        zoom_in.set_tooltip_text(Some(s::IMG_ZOOM_IN));
        let zoom_label = gtk4::Label::new(Some("100%"));
        let fit = gtk4::Button::from_icon_name("zoom-fit-best-symbolic");
        fit.set_tooltip_text(Some(s::IMG_FIT));
        let zoom_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        zoom_box.append(&gtk4::Label::new(Some(s::IMG_ZOOM)));
        zoom_box.append(&zoom_out);
        zoom_box.append(&zoom_label);
        zoom_box.append(&zoom_in);
        zoom_box.append(&fit);
        toolbar_inner.append(&zoom_box);

        let empty_label = gtk4::Label::new(Some(s::MSG_NO_TARGET));
        empty_label.add_css_class("dl-empty");
        empty_label.set_vexpand(true);
        empty_label.set_visible(false);

        let meta_grid = gtk4::Grid::new();
        meta_grid.set_row_spacing(2);
        meta_grid.set_column_spacing(12);
        meta_grid.set_margin_start(8);
        meta_grid.set_margin_end(8);
        meta_grid.set_margin_top(4);
        meta_grid.set_margin_bottom(4);

        let warn_label = gtk4::Label::new(None);
        warn_label.set_xalign(0.0);
        warn_label.set_margin_start(8);
        warn_label.add_css_class("dl-empty");
        warn_label.set_visible(false);

        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.append(&toolbar_scroll);
        widget.append(&area);
        widget.append(&empty_label);
        widget.append(&meta_grid);
        widget.append(&warn_label);
        widget.set_vexpand(true);
        widget.set_hexpand(true);

        let this = Rc::new(Self {
            widget,
            area: area.clone(),
            mode: Cell::new(Mode::SideBySide),
            opacity: Cell::new(0.5),
            slice_pos: Cell::new(0.5),
            zoom: Cell::new(1.0),
            pan: Cell::new((0.0, 0.0)),
            blink_phase: Cell::new(false),
            blink_source: RefCell::new(None),
            threshold_src: RefCell::new(None),
            drag_start_pan: Cell::new((0.0, 0.0)),
            pinch_base_zoom: Cell::new(1.0),
            decoded: RefCell::new(None),
            result: RefCell::new(None),
            surfaces: RefCell::new(None),
            threshold_scale: threshold_scale.clone(),
            zoom_label: zoom_label.clone(),
            opacity_label: opacity_label.clone(),
            slice_row: slice_row.clone(),
            meta_grid: meta_grid.clone(),
            warn_label: warn_label.clone(),
            empty_label: empty_label.clone(),
        });

        // ドラッグでパン
        let drag = gtk4::GestureDrag::new();
        {
            let weak = Rc::downgrade(&this);
            drag.connect_drag_update(move |_g, dx, dy| {
                let Some(this) = weak.upgrade() else { return };
                let (ox, oy) = this.drag_start_pan.get();
                // 画像が完全に画面外へ出ないよう緩くクランプ
                this.pan.set((ox + dx, oy + dy));
                this.area.queue_draw();
            });
        }
        {
            let weak = Rc::downgrade(&this);
            drag.connect_drag_begin(move |g, _x, _y| {
                if let Some(this) = weak.upgrade() {
                    this.drag_start_pan.set(this.pan.get());
                }
                g.set_state(gtk4::EventSequenceState::Claimed);
            });
        }
        area.add_controller(drag);

        // ピンチズーム(開始時の倍率を基準にする)
        let pinch = gtk4::GestureZoom::new();
        {
            let weak = Rc::downgrade(&this);
            pinch.connect_begin(move |g, _seq| {
                if let Some(this) = weak.upgrade() {
                    this.pinch_base_zoom.set(this.zoom.get());
                }
                g.set_state(gtk4::EventSequenceState::Claimed);
            });
        }
        {
            let weak = Rc::downgrade(&this);
            pinch.connect_scale_changed(move |_g, scale| {
                let Some(this) = weak.upgrade() else { return };
                this.set_zoom(this.pinch_base_zoom.get() * scale);
            });
        }
        area.add_controller(pinch);

        {
            let weak = Rc::downgrade(&this);
            area.set_draw_func(move |_a, cr, _w, _h| {
                if let Some(this) = weak.upgrade() {
                    this.draw(cr);
                }
            });
        }

        {
            let weak = Rc::downgrade(&this);
            mode_dd.connect_selected_notify(move |dd| {
                let Some(this) = weak.upgrade() else { return };
                this.mode.set(Mode::from_index(dd.selected()));
                // モード切替時はパン位置をリセット(別レイアウトへの持ち越し防止)
                this.pan.set((0.0, 0.0));
                this.slice_row
                    .set_visible(dd.selected() == Mode::Slice as u32);
                this.update_blink();
                this.area.queue_draw();
            });
        }
        {
            let weak = Rc::downgrade(&this);
            opacity_scale.connect_value_changed(move |sc| {
                let Some(this) = weak.upgrade() else { return };
                this.opacity.set(sc.value());
                this.opacity_label
                    .set_text(&format!("{}%", (sc.value() * 100.0) as i32));
                this.area.queue_draw();
            });
        }
        {
            let weak = Rc::downgrade(&this);
            threshold_scale.connect_value_changed(move |sc| {
                let Some(this) = weak.upgrade() else { return };
                let v = sc.value() as i32;
                th_val.set_text(&v.to_string());
                // 400ms デバウンス後に差分を再計算
                if let Some(src) = this.threshold_src.borrow_mut().take() {
                    src.remove();
                }
                let weak2 = Rc::downgrade(&this);
                *this.threshold_src.borrow_mut() = Some(glib::timeout_add_local(
                    std::time::Duration::from_millis(THRESHOLD_DEBOUNCE_MS),
                    move || {
                        if let Some(t) = weak2.upgrade() {
                            t.recompute_diff(v);
                        }
                        glib::ControlFlow::Break
                    },
                ));
            });
        }
        {
            let weak = Rc::downgrade(&this);
            slice_scale.connect_value_changed(move |sc| {
                let Some(this) = weak.upgrade() else { return };
                this.slice_pos.set(sc.value());
                slice_val.set_text(&format!("{}%", (sc.value() * 100.0) as i32));
                this.area.queue_draw();
            });
        }
        {
            let weak = Rc::downgrade(&this);
            zoom_in.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.set_zoom(t.zoom.get() * ZOOM_STEP)
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            zoom_out.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.set_zoom(t.zoom.get() / ZOOM_STEP)
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            fit.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.fit_zoom()
                }
            });
        }
        this
    }

    fn set_zoom(&self, z: f64) {
        self.zoom.set(z.clamp(ZOOM_MIN, ZOOM_MAX));
        self.zoom_label
            .set_text(&format!("{}%", (self.zoom.get() * 100.0) as i32));
        self.area.queue_draw();
    }

    fn fit_zoom(&self) {
        let Some(res) = self.result.borrow().clone() else {
            return;
        };
        let aw = self.area.width().max(1) as f64;
        let ah = self.area.height().max(1) as f64;
        let (cw, ch) = self.canvas_size(&res);
        if cw <= 0.0 || ch <= 0.0 {
            return;
        }
        let z = (aw / cw).min(ah / ch).clamp(ZOOM_MIN, ZOOM_MAX);
        self.zoom.set(z);
        self.pan.set((0.0, 0.0));
        self.zoom_label
            .set_text(&format!("{}%", (z * 100.0) as i32));
        self.area.queue_draw();
    }

    /// モード別のコンテンツ論理サイズ(ピクセル)。
    fn canvas_size(&self, res: &ImageDiffResult) -> (f64, f64) {
        match self.mode.get() {
            Mode::SideBySide => (
                res.left.width as f64 + res.right.width as f64 + 8.0,
                res.left.height.max(res.right.height) as f64,
            ),
            Mode::Difference => (res.diff.width as f64, res.diff.height as f64),
            _ => (
                res.left.width.max(res.right.width) as f64,
                res.left.height.max(res.right.height) as f64,
            ),
        }
    }

    /// 左右の画像ファイルを読み込み、差分計算と描画を行う。
    pub fn load(&self, left: &std::path::Path, right: &std::path::Path) {
        self.empty_label.set_visible(false);
        match (
            imageops::RgbaImage::load(left),
            imageops::RgbaImage::load(right),
        ) {
            (Ok(l), Ok(r)) => {
                let threshold = self.threshold_scale.value() as i32;
                let res = imageops::compute_diff(l.clone(), r.clone(), threshold, left, right);
                *self.decoded.borrow_mut() = Some((l, r, left.to_path_buf(), right.to_path_buf()));
                self.apply_result(res);
            }
            (Err(e), _) | (_, Err(e)) => {
                self.show_message(&e);
            }
        }
    }

    fn recompute_diff(&self, threshold: i32) {
        let Some((l, r, lp, rp)) = self.decoded.borrow().clone() else {
            return;
        };
        let res = imageops::compute_diff(l, r, threshold, &lp, &rp);
        self.apply_result(res);
    }

    fn apply_result(&self, res: ImageDiffResult) {
        let surfaces = Surfaces {
            left: imageops::to_cairo_surface(&res.left),
            right: imageops::to_cairo_surface(&res.right),
            diff: imageops::to_cairo_surface(&res.diff),
            diff_w: res.diff.width as f64,
            diff_h: res.diff.height as f64,
        };
        *self.surfaces.borrow_mut() = Some(Rc::new(surfaces));
        self.update_meta(&res);
        let mut warns = Vec::new();
        if res.stats.used_downsampled_comparison {
            warns.push(s::IMG_WARN_DOWNSAMPLED);
        }
        if res.stats.size_mismatch {
            warns.push(s::IMG_WARN_SIZE_MISMATCH);
        }
        if warns.is_empty() {
            self.warn_label.set_visible(false);
        } else {
            self.warn_label.set_text(&warns.join("  /  "));
            self.warn_label.set_visible(true);
        }
        *self.result.borrow_mut() = Some(Rc::new(res));
        self.pan.set((0.0, 0.0));
        self.area.queue_draw();
    }

    fn update_meta(&self, res: &ImageDiffResult) {
        // 既存行をクリア
        while let Some(child) = self.meta_grid.first_child() {
            self.meta_grid.remove(&child);
        }
        let cell = |txt: &str, x: i32, y: i32, head: bool| {
            let l = gtk4::Label::new(Some(txt));
            l.set_xalign(0.0);
            if head {
                l.add_css_class("heading");
            }
            self.meta_grid.attach(&l, x, y, 1, 1);
        };
        cell(s::IMG_DIM, 1, 0, true);
        cell(s::IMG_FILESIZE, 2, 0, true);
        cell(s::IMG_FORMAT, 3, 0, true);
        cell(s::IMG_LEFT, 0, 1, true);
        cell(
            &format!("{} x {}", res.left_meta.width, res.left_meta.height),
            1,
            1,
            false,
        );
        cell(&imageops::human_size(res.left_meta.file_size), 2, 1, false);
        cell(&res.left_meta.format, 3, 1, false);
        cell(s::IMG_RIGHT, 0, 2, true);
        cell(
            &format!("{} x {}", res.right_meta.width, res.right_meta.height),
            1,
            2,
            false,
        );
        cell(&imageops::human_size(res.right_meta.file_size), 2, 2, false);
        cell(&res.right_meta.format, 3, 2, false);
        cell(s::IMG_DIFF_ROW, 0, 3, true);
        cell(
            &format!(
                "{} x {} px",
                res.stats.compared_width, res.stats.compared_height
            ),
            1,
            3,
            false,
        );
        cell(&format!("{} px", res.stats.diff_pixel_count), 2, 3, false);
        cell(&format!("{:.2}%", res.stats.diff_percentage), 3, 3, false);
        self.meta_grid.set_visible(true);
    }

    pub fn show_message(&self, msg: &str) {
        *self.result.borrow_mut() = None;
        *self.surfaces.borrow_mut() = None;
        self.empty_label.set_text(msg);
        self.empty_label.set_visible(true);
        self.area.queue_draw();
    }

    fn update_blink(self: &Rc<Self>) {
        if let Some(src) = self.blink_source.borrow_mut().take() {
            src.remove();
        }
        if self.mode.get() != Mode::Blink {
            return;
        }
        let weak = Rc::downgrade(self);
        *self.blink_source.borrow_mut() = Some(glib::timeout_add_local(
            std::time::Duration::from_millis(BLINK_MS),
            move || {
                let Some(this) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if this.mode.get() != Mode::Blink {
                    return glib::ControlFlow::Break;
                }
                this.blink_phase.set(!this.blink_phase.get());
                this.area.queue_draw();
                glib::ControlFlow::Continue
            },
        ));
    }

    fn draw(&self, cr: &cairo::Context) {
        let Some(surf) = self.surfaces.borrow().clone() else {
            return;
        };
        let (px, py) = self.pan.get();
        let z = self.zoom.get();
        let _ = cr.save();
        cr.translate(px, py);
        cr.scale(z, z);
        match self.mode.get() {
            Mode::SideBySide => {
                let _ = cr.set_source_surface(&surf.left, 0.0, 0.0);
                let _ = cr.paint();
                let x = surf.left.width() as f64 + 8.0;
                let _ = cr.set_source_surface(&surf.right, x, 0.0);
                let _ = cr.paint();
            }
            Mode::Overlay => {
                let _ = cr.set_source_surface(&surf.left, 0.0, 0.0);
                let _ = cr.paint();
                let _ = cr.set_source_surface(&surf.right, 0.0, 0.0);
                let _ = cr.paint_with_alpha(self.opacity.get());
            }
            Mode::Blink => {
                let img = if self.blink_phase.get() {
                    &surf.right
                } else {
                    &surf.left
                };
                let _ = cr.set_source_surface(img, 0.0, 0.0);
                let _ = cr.paint();
            }
            Mode::Difference => {
                let _ = cr.set_source_surface(&surf.diff, 0.0, 0.0);
                let _ = cr.paint();
            }
            Mode::Slice => {
                let w = surf
                    .diff_w
                    .max((surf.left.width().max(surf.right.width())) as f64);
                let _h = surf.diff_h;
                let cut = self.slice_pos.get() * w;
                cr.rectangle(0.0, 0.0, cut, f64::MAX / 4.0);
                cr.clip();
                let _ = cr.set_source_surface(&surf.left, 0.0, 0.0);
                let _ = cr.paint();
                cr.reset_clip();
                cr.rectangle(cut, 0.0, f64::MAX / 4.0, f64::MAX / 4.0);
                cr.clip();
                let _ = cr.set_source_surface(&surf.right, 0.0, 0.0);
                let _ = cr.paint();
                cr.reset_clip();
                // 白い境界線
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.set_line_width(1.5 / z.max(0.01));
                cr.move_to(cut, 0.0);
                cr.line_to(
                    cut,
                    surf.diff_h
                        .max(surf.left.height().max(surf.right.height()) as f64),
                );
                let _ = cr.stroke();
            }
        }
        let _ = cr.restore();
    }
}

fn labeled(label: &str, w: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    b.append(&gtk4::Label::new(Some(label)));
    b.append(w);
    b
}
