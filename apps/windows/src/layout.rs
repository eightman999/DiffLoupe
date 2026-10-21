//! 画面レイアウト計算(プラットフォーム非依存)。
//!
//! Win32 層はコントロールの実測サイズ(テキスト幅・行高)をピクセルで渡し、
//! ここで全要素の矩形を決める。ウィンドウ幅に追従し、はみ出さないことを
//! ホスト OS 上のテストで保証する。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    /// 2 つの矩形が(面積を持って)重なるか
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.w > 0
            && self.h > 0
            && o.w > 0
            && o.h > 0
            && self.x < o.right()
            && o.x < self.right()
            && self.y < o.bottom()
            && o.y < self.bottom()
    }
}

/// 幅の合計が `avail` に収まるよう比例縮小する(各幅は `min_w` を下回らない)。
pub fn fit_widths(widths: &[i32], avail: i32, gap: i32, min_w: i32) -> Vec<i32> {
    let n = widths.len() as i32;
    if n == 0 {
        return Vec::new();
    }
    let total: i32 = widths.iter().sum::<i32>() + gap * (n - 1);
    if total <= avail || avail <= 0 {
        return widths.to_vec();
    }
    let usable = (avail - gap * (n - 1)).max(min_w * n);
    let sum: i32 = widths.iter().sum::<i32>().max(1);
    widths
        .iter()
        .map(|w| ((*w as i64 * usable as i64 / sum as i64) as i32).max(min_w))
        .collect()
}

/// 詳細ペイン内ツールバー(差分ナビ・検索)の入力幅
#[derive(Debug, Clone, Copy)]
pub struct ToolbarInput {
    pub prev_diff_w: i32,
    pub next_diff_w: i32,
    pub pos_w: i32,
    pub hit_btn_w: i32,
    pub case_w: i32,
    pub all_w: i32,
    pub search_min_w: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct MainInput {
    pub w: i32,
    pub h: i32,
    pub pad: i32,
    pub gap: i32,
    /// コントロール 1 行の高さ
    pub row: i32,
    /// 下端のステータスバーの高さ
    pub status_h: i32,
    pub folder_btn_w: i32,
    pub ref_combo_w: i32,
    pub has_left_ref: bool,
    pub has_right_ref: bool,
    pub history_w: i32,
    pub swap_w: i32,
    pub threeway_w: i32,
    pub sync_w: i32,
    pub report_w: i32,
    pub settings_w: i32,
    pub compare_w: i32,
    pub filter_w: i32,
    pub split_ratio: f64,
    pub hdr1_h: i32,
    pub hdr2_h: i32,
    pub toolbar_visible: bool,
    pub toolbar: ToolbarInput,
    pub progress_w: i32,
    pub grip_w: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolbarLayout {
    pub prev_diff: Rect,
    pub next_diff: Rect,
    pub pos: Rect,
    pub search: Rect,
    pub prev_hit: Rect,
    pub next_hit: Rect,
    pub case_chk: Rect,
    pub show_all: Rect,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MainLayout {
    pub left_btn: Rect,
    pub left_path: Rect,
    pub left_ref: Option<Rect>,
    pub right_btn: Rect,
    pub right_path: Rect,
    pub right_ref: Option<Rect>,
    pub history: Rect,
    pub swap: Rect,
    pub threeway: Rect,
    pub sync: Rect,
    pub report: Rect,
    pub settings: Rect,
    pub compare: Rect,
    pub search: Rect,
    pub filter: Rect,
    pub tree: Rect,
    pub detail: Rect,
    pub hdr1: Rect,
    pub hdr2: Rect,
    pub toolbar: ToolbarLayout,
    /// 詳細コンテンツ(grid / 画像ペイン / メッセージ)
    pub content: Rect,
    pub status: Rect,
    pub progress: Rect,
    /// ステータスバー第 1 パートの右端(クライアント座標)
    pub status_part0_right: i32,
}

/// ツリーと詳細ペインの水平分割: (tree_x, tree_w, detail_x, detail_w)
pub fn split_panes(w: i32, pad: i32, gap: i32, ratio: f64) -> (i32, i32, i32, i32) {
    let inner = (w - pad * 2 - gap).max(0);
    let min_tree = 160.min(inner / 2);
    let min_detail = 240.min(inner / 2);
    let mut tree_w = (inner as f64 * ratio.clamp(0.1, 0.9)) as i32;
    tree_w = tree_w.min(inner - min_detail).max(min_tree);
    let detail_w = (inner - tree_w).max(0);
    (pad, tree_w, pad + tree_w + gap, detail_w)
}

pub fn compute_main(i: &MainInput) -> MainLayout {
    let mut l = MainLayout::default();
    let (w, pad, gap, row) = (i.w, i.pad, i.gap, i.row);
    let inner_w = (w - pad * 2).max(0);
    let mut y = pad;

    // 1・2 行目: フォルダ選択ボタン + パス(残り幅いっぱい) + (git ref コンボ)
    let folder_row = |btn_w: i32, has_ref: bool, y: i32| {
        let btn = Rect::new(pad, y, btn_w.min(inner_w), row);
        let mut right = pad + inner_w;
        let ref_r = if has_ref {
            let cw = i.ref_combo_w.min((inner_w - btn.w) / 2).max(0);
            right -= cw;
            let r = Rect::new(right, y, cw, row);
            right -= gap;
            Some(r)
        } else {
            None
        };
        let path_x = btn.right() + gap;
        let path = Rect::new(path_x, y, (right - path_x).max(0), row);
        (btn, path, ref_r)
    };
    let (b, p, r) = folder_row(i.folder_btn_w, i.has_left_ref, y);
    l.left_btn = b;
    l.left_path = p;
    l.left_ref = r;
    y += row + gap;
    let (b, p, r) = folder_row(i.folder_btn_w, i.has_right_ref, y);
    l.right_btn = b;
    l.right_path = p;
    l.right_ref = r;
    y += row + gap + gap / 2;

    // 3 行目: 履歴 / 交換 / 3-way / コピー計画 / レポート / 設定 ・・・ 比較(右端)
    let left_items = [
        i.history_w,
        i.swap_w,
        i.threeway_w,
        i.sync_w,
        i.report_w,
        i.settings_w,
    ];
    let avail = inner_w - i.compare_w - gap * 2;
    let ws = fit_widths(&left_items, avail, gap, row);
    let mut x = pad;
    let mut rects = [Rect::default(); 6];
    for (k, wv) in ws.iter().enumerate() {
        rects[k] = Rect::new(x, y, *wv, row);
        x += wv + gap;
    }
    l.history = rects[0];
    l.swap = rects[1];
    l.threeway = rects[2];
    l.sync = rects[3];
    l.report = rects[4];
    l.settings = rects[5];
    let cw = i.compare_w.min(inner_w);
    l.compare = Rect::new((pad + inner_w - cw).max(pad), y, cw, row);
    y += row + gap + gap / 2;

    // 4 行目以降: ツリー側(検索 + フィルタ) / 詳細ペイン
    let status_top = (i.h - i.status_h).max(y);
    let (tree_x, tree_w, detail_x, detail_w) = split_panes(w, pad, gap, i.split_ratio);
    let filter_w = i.filter_w.min(tree_w / 2);
    l.filter = Rect::new(tree_x + tree_w - filter_w, y, filter_w, row);
    l.search = Rect::new(tree_x, y, (tree_w - filter_w - gap).max(0), row);
    let body_top = y + row + gap;
    let body_bottom = (status_top - gap).max(body_top);
    l.tree = Rect::new(tree_x, body_top, tree_w, body_bottom - body_top);

    // 詳細ペイン
    l.detail = Rect::new(detail_x, y, detail_w, body_bottom - y);
    let mut dy = y;
    l.hdr1 = Rect::new(detail_x, dy, detail_w, i.hdr1_h);
    dy += i.hdr1_h;
    l.hdr2 = Rect::new(detail_x, dy, detail_w, i.hdr2_h);
    dy += i.hdr2_h + gap;
    if i.toolbar_visible {
        let t = &i.toolbar;
        let mut tl = ToolbarLayout::default();
        let right_items = [t.hit_btn_w, t.hit_btn_w, t.case_w, t.all_w];
        let right_total: i32 = right_items.iter().sum::<i32>() + gap * right_items.len() as i32;
        let left_total = t.prev_diff_w + t.next_diff_w + t.pos_w + gap * 3;
        let search_w = (detail_w - left_total - right_total).max(t.search_min_w.min(detail_w));
        let mut x = detail_x;
        tl.prev_diff = Rect::new(x, dy, t.prev_diff_w, row);
        x += t.prev_diff_w + gap;
        tl.next_diff = Rect::new(x, dy, t.next_diff_w, row);
        x += t.next_diff_w + gap;
        tl.pos = Rect::new(x, dy, t.pos_w, row);
        x += t.pos_w + gap;
        tl.search = Rect::new(x, dy, search_w, row);
        x += search_w + gap;
        tl.prev_hit = Rect::new(x, dy, t.hit_btn_w, row);
        x += t.hit_btn_w + gap;
        tl.next_hit = Rect::new(x, dy, t.hit_btn_w, row);
        x += t.hit_btn_w + gap;
        tl.case_chk = Rect::new(x, dy, t.case_w, row);
        x += t.case_w + gap;
        tl.show_all = Rect::new(x, dy, t.all_w, row);
        l.toolbar = tl;
        dy += row + gap;
    }
    l.content = Rect::new(detail_x, dy, detail_w, (body_bottom - dy).max(0));

    // ステータスバー(下端いっぱい) と進捗バー
    l.status = Rect::new(0, status_top, w, i.status_h);
    let ph = (i.status_h - gap).max(4);
    let px = (w - i.grip_w - i.progress_w - gap).max(0);
    l.progress = Rect::new(px, status_top + (i.status_h - ph) / 2, i.progress_w, ph);
    l.status_part0_right = (px - gap).max(1);
    l
}

/// 画像比較ペインの入力(ピクセル)
#[derive(Debug, Clone, Copy)]
pub struct ImageInput {
    pub w: i32,
    pub h: i32,
    pub pad: i32,
    pub gap: i32,
    pub row: i32,
    pub mode_w: i32,
    /// 不透明度/閾値/境界のスライダー群を出すか
    pub dyn_visible: bool,
    pub lbl1_w: i32,
    pub track_w: i32,
    pub lbl2_w: i32,
    pub zbtn_w: i32,
    pub ztrack_w: i32,
    pub lblz_w: i32,
    pub fit_w: i32,
    /// 下端の情報テーブルの高さ(0 なら無し)
    pub meta_h: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageLayout {
    pub mode: Rect,
    pub lbl1: Rect,
    pub track: Rect,
    pub lbl2: Rect,
    pub zout: Rect,
    pub ztrack: Rect,
    pub zin: Rect,
    pub lblz: Rect,
    pub zfit: Rect,
    pub canvas: Rect,
    pub meta: Rect,
    pub toolbar_h: i32,
    pub rows: i32,
}

/// 画像比較ペイン: ツールバー(1〜2 段) / キャンバス / 情報テーブル を縦に積む。
/// ズーム群が 1 段に収まらないときは 2 段目へ折り返す。
pub fn compute_image_pane(i: &ImageInput) -> ImageLayout {
    let mut l = ImageLayout::default();
    let (pad, gap, row) = (i.pad, i.gap, i.row);
    let mut x = pad;
    let y1 = pad;
    l.mode = Rect::new(x, y1, i.mode_w, row);
    x += i.mode_w + gap;
    if i.dyn_visible {
        l.lbl1 = Rect::new(x, y1, i.lbl1_w, row);
        x += i.lbl1_w + gap / 2;
        l.track = Rect::new(x, y1, i.track_w, row);
        x += i.track_w + gap / 2;
        l.lbl2 = Rect::new(x, y1, i.lbl2_w, row);
        x += i.lbl2_w + gap;
    }
    let zw = i.zbtn_w * 2 + i.ztrack_w + i.lblz_w + i.fit_w + gap * 4;
    let one_row = x + zw <= i.w - pad;
    let (zx, zy) = if one_row {
        ((i.w - pad - zw).max(x), y1)
    } else {
        (pad, y1 + row + gap)
    };
    let mut zx2 = zx;
    l.zout = Rect::new(zx2, zy, i.zbtn_w, row);
    zx2 += i.zbtn_w + gap;
    l.ztrack = Rect::new(zx2, zy, i.ztrack_w, row);
    zx2 += i.ztrack_w + gap;
    l.zin = Rect::new(zx2, zy, i.zbtn_w, row);
    zx2 += i.zbtn_w + gap;
    l.lblz = Rect::new(zx2, zy, i.lblz_w, row);
    zx2 += i.lblz_w + gap;
    l.zfit = Rect::new(zx2, zy, i.fit_w, row);
    l.rows = if one_row { 1 } else { 2 };
    l.toolbar_h = pad + l.rows * row + (l.rows - 1) * gap + gap;
    let meta_h = i.meta_h.max(0);
    let meta_y = (i.h - meta_h - pad).max(l.toolbar_h);
    if meta_h > 0 {
        l.meta = Rect::new(
            pad,
            meta_y,
            (i.w - pad * 2).max(0),
            (i.h - pad - meta_y).max(0),
        );
    }
    let canvas_bottom = if meta_h > 0 { meta_y - gap } else { i.h - pad };
    l.canvas = Rect::new(
        pad,
        l.toolbar_h,
        (i.w - pad * 2).max(0),
        (canvas_bottom - l.toolbar_h).max(0),
    );
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(w: i32, h: i32) -> MainInput {
        MainInput {
            w,
            h,
            pad: 12,
            gap: 8,
            row: 28,
            status_h: 22,
            folder_btn_w: 110,
            ref_combo_w: 150,
            has_left_ref: false,
            has_right_ref: false,
            history_w: 100,
            swap_w: 80,
            threeway_w: 110,
            sync_w: 110,
            report_w: 120,
            settings_w: 80,
            compare_w: 90,
            filter_w: 110,
            split_ratio: 0.38,
            hdr1_h: 22,
            hdr2_h: 40,
            toolbar_visible: true,
            toolbar: ToolbarInput {
                prev_diff_w: 90,
                next_diff_w: 90,
                pos_w: 50,
                hit_btn_w: 28,
                case_w: 50,
                all_w: 100,
                search_min_w: 80,
            },
            progress_w: 150,
            grip_w: 16,
        }
    }

    fn all_rects(l: &MainLayout) -> Vec<(&'static str, Rect)> {
        let mut v = vec![
            ("left_btn", l.left_btn),
            ("left_path", l.left_path),
            ("right_btn", l.right_btn),
            ("right_path", l.right_path),
            ("history", l.history),
            ("swap", l.swap),
            ("threeway", l.threeway),
            ("sync", l.sync),
            ("report", l.report),
            ("settings", l.settings),
            ("compare", l.compare),
            ("search", l.search),
            ("filter", l.filter),
            ("tree", l.tree),
            ("hdr1", l.hdr1),
            ("hdr2", l.hdr2),
            ("content", l.content),
        ];
        if let Some(r) = l.left_ref {
            v.push(("left_ref", r));
        }
        if let Some(r) = l.right_ref {
            v.push(("right_ref", r));
        }
        let t = &l.toolbar;
        v.extend([
            ("tb.prev_diff", t.prev_diff),
            ("tb.next_diff", t.next_diff),
            ("tb.pos", t.pos),
            ("tb.search", t.search),
            ("tb.prev_hit", t.prev_hit),
            ("tb.next_hit", t.next_hit),
            ("tb.case", t.case_chk),
            ("tb.all", t.show_all),
        ]);
        v
    }

    #[test]
    fn no_overlap_and_inside_for_many_sizes() {
        for (w, h) in [
            (1028, 700),
            (1000, 600),
            (1280, 800),
            (1920, 1080),
            (3000, 1500),
        ] {
            let mut inp = input(w, h);
            inp.has_left_ref = true;
            inp.has_right_ref = true;
            let l = compute_main(&inp);
            let rs = all_rects(&l);
            for (n, r) in &rs {
                assert!(r.x >= 0 && r.y >= 0, "{n} negative at {w}x{h}: {r:?}");
                assert!(r.right() <= w, "{n} exceeds width at {w}x{h}: {r:?}");
                assert!(r.bottom() <= h - inp.status_h, "{n} exceeds bottom: {r:?}");
            }
            for (i, (na, a)) in rs.iter().enumerate() {
                for (nb, b) in rs.iter().skip(i + 1) {
                    // hdr/toolbar は詳細ペイン内で縦に積むため重ならない。ペアで全検査する。
                    assert!(!a.overlaps(b), "{na} overlaps {nb} at {w}x{h}: {a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn path_edits_fill_remaining_width() {
        let l = compute_main(&input(1200, 800));
        assert_eq!(l.left_path.right(), 1200 - 12);
        assert_eq!(l.right_path.right(), 1200 - 12);
        let mut inp = input(1200, 800);
        inp.has_left_ref = true;
        let l = compute_main(&inp);
        let r = l.left_ref.unwrap();
        assert_eq!(r.right(), 1200 - 12);
        assert_eq!(l.left_path.right() + 8, r.x);
    }

    #[test]
    fn follows_window_size() {
        let a = compute_main(&input(1000, 700));
        let b = compute_main(&input(1400, 900));
        assert!(b.left_path.w > a.left_path.w);
        assert!(b.tree.h > a.tree.h);
        assert!(b.content.w > a.content.w);
        assert_eq!(b.compare.right(), 1400 - 12);
        assert_eq!(b.status.w, 1400);
        assert_eq!(b.status.bottom(), 900);
    }

    #[test]
    fn toolbar_hidden_gives_more_content_height() {
        let mut inp = input(1100, 700);
        let with = compute_main(&inp);
        inp.toolbar_visible = false;
        let without = compute_main(&inp);
        assert!(without.content.h > with.content.h);
        assert_eq!(without.toolbar, ToolbarLayout::default());
    }

    #[test]
    fn toolbar_fits_in_detail_pane() {
        let l = compute_main(&input(1028, 700));
        assert!(l.toolbar.show_all.right() <= l.detail.right());
        assert!(l.toolbar.search.w >= 80);
    }

    #[test]
    fn row3_shrinks_instead_of_overflowing() {
        let l = compute_main(&input(700, 600));
        assert!(l.settings.right() < l.compare.x);
        assert!(l.compare.right() <= 700);
    }

    #[test]
    fn fit_widths_behaviour() {
        assert_eq!(fit_widths(&[50, 50], 200, 6, 10), vec![50, 50]);
        let v = fit_widths(&[100, 100], 100, 10, 10);
        assert!(v.iter().sum::<i32>() + 10 <= 100);
        assert!(fit_widths(&[], 10, 1, 1).is_empty());
    }

    #[test]
    fn split_is_sane_for_tiny_widths() {
        let (tx, tw, dx, dw) = split_panes(100, 12, 8, 0.38);
        assert!(tx >= 0 && tw >= 0 && dw >= 0 && dx >= tx + tw);
        let (_, tw, _, dw) = split_panes(1000, 12, 8, 0.38);
        assert!(tw > 300 && dw > 500);
    }

    #[test]
    fn progress_sits_in_status_bar() {
        let l = compute_main(&input(1028, 700));
        assert!(l.progress.y >= l.status.y && l.progress.bottom() <= l.status.bottom());
        assert!(l.progress.right() <= l.status.right());
        assert!(l.status_part0_right < l.progress.x);
    }

    fn img_input(w: i32, h: i32, dyn_visible: bool) -> ImageInput {
        ImageInput {
            w,
            h,
            pad: 12,
            gap: 8,
            row: 28,
            mode_w: 140,
            dyn_visible,
            lbl1_w: 60,
            track_w: 140,
            lbl2_w: 44,
            zbtn_w: 32,
            ztrack_w: 130,
            lblz_w: 52,
            fit_w: 72,
            meta_h: 100,
        }
    }

    #[test]
    fn image_pane_never_overlaps_and_stays_inside() {
        for (w, h) in [(560, 480), (617, 560), (900, 600), (1400, 900)] {
            for dynv in [false, true] {
                let l = compute_image_pane(&img_input(w, h, dynv));
                let mut parts = vec![l.mode, l.zout, l.ztrack, l.zin, l.lblz, l.zfit];
                if dynv {
                    parts.extend([l.lbl1, l.track, l.lbl2]);
                }
                for r in &parts {
                    assert!(r.x >= 0 && r.right() <= w, "{r:?} outside {w}");
                    assert!(r.bottom() <= l.toolbar_h, "{r:?} below toolbar {l:?}");
                }
                for (i, a) in parts.iter().enumerate() {
                    for b in parts.iter().skip(i + 1) {
                        assert!(!a.overlaps(b), "{a:?} overlaps {b:?} at {w}x{h}");
                    }
                }
                // ツールバー・キャンバス・情報テーブルが縦に重ならない
                assert!(l.canvas.y >= l.toolbar_h);
                assert!(l.canvas.bottom() <= l.meta.y, "{l:?}");
                assert!(l.meta.bottom() <= h);
                assert!(l.canvas.right() <= w);
            }
        }
    }

    #[test]
    fn image_pane_wraps_zoom_when_narrow() {
        let wide = compute_image_pane(&img_input(1400, 800, true));
        let narrow = compute_image_pane(&img_input(560, 800, true));
        assert_eq!(wide.rows, 1);
        assert_eq!(narrow.rows, 2);
        assert!(narrow.canvas.y > wide.canvas.y);
    }
}
