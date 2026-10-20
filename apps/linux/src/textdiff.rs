//! テキスト行差分ビュー。
//! 2つの `TextView` を1つの ScrolledWindow 内の HBox に並べ、
//! 左右で同じ行数のバッファを構築する(片側のみの行は空行で埋める)。
//! 共通スクロール領域・行全体背景色は TextTag の paragraph-background、
//! 行内差分は background タグ、コンテキスト折りたたみはイベント付きタグで実装する。

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use diffloupe_core::diff_display as dd;
use diffloupe_core::inline_diff;
use diffloupe_core::line_diff::{self, DiffRow, DiffRowKind};
use diffloupe_core::unicode;
use diffloupe_core::ContentComparison;
use gtk4::prelude::*;

use crate::strings as s;

/// 行背景色(段落全体)。ライト/ダーク共用の半透明色。
const BG_ADD: &str = "rgba(46, 160, 67, 0.28)";
const BG_DEL: &str = "rgba(248, 81, 73, 0.28)";
const BG_MOD: &str = "rgba(210, 153, 34, 0.30)";
/// 行内(インライン)差分の強調色
const BG_INLINE_L: &str = "rgba(248, 81, 73, 0.55)";
const BG_INLINE_R: &str = "rgba(46, 160, 67, 0.55)";
const BG_SEARCH: &str = "rgba(191, 135, 240, 0.55)";

/// 読み込み結果(詳細ペイン側でメッセージ切替に使う)
#[derive(Debug)]
pub enum TextLoad {
    Loaded,
    Binary,
    TooLarge,
    Io(String),
}

/// `load` の戻り値。エンコード名はヘッダ表示用。
pub struct TextLoadDetail {
    pub result: TextLoad,
    pub left_encoding: Option<&'static str>,
    pub right_encoding: Option<&'static str>,
}

pub struct TextDiffView {
    pub widget: gtk4::Box,
    left_buf: gtk4::TextBuffer,
    right_buf: gtk4::TextBuffer,

    rows: RefCell<Vec<DiffRow>>,
    expanded: RefCell<HashSet<usize>>,
    show_all: Cell<bool>,
    search_query: RefCell<String>,
    search_case: Cell<bool>,
    search_hits: RefCell<Vec<usize>>,
    diff_ids: RefCell<Vec<usize>>,
    cur_diff_pos: Cell<Option<usize>>,
    cur_match_pos: Cell<Option<usize>>,
    /// バッファ行番号 → (DiffRow.id | 折りたたみグループ id)
    line_map: RefCell<Vec<LineTarget>>,
    left_tv: gtk4::TextView,
    counter: gtk4::Label,
    match_counter: gtk4::Label,
}

#[derive(Debug, Clone, Copy)]
enum LineTarget {
    Row(usize),
    Fold(usize),
}

fn make_tags(buf: &gtk4::TextBuffer) {
    use gtk4::TextTag;
    let table = buf.tag_table();
    let tag = |name: &str| {
        let t = TextTag::new(Some(name));
        table.add(&t);
        t
    };
    let add = tag("add");
    add.set_paragraph_background(Some(BG_ADD));
    let del = tag("del");
    del.set_paragraph_background(Some(BG_DEL));
    let md = tag("mod");
    md.set_paragraph_background(Some(BG_MOD));
    let il_l = tag("inline_l");
    il_l.set_background(Some(BG_INLINE_L));
    let il_r = tag("inline_r");
    il_r.set_background(Some(BG_INLINE_R));
    let hit = tag("hit");
    hit.set_background(Some(BG_SEARCH));
    let fold = tag("fold");
    fold.set_foreground(Some("#1f6feb"));
    fold.set_underline(gtk4::pango::Underline::Single);
    let lineno = tag("lineno");
    lineno.set_foreground(Some("rgba(127,127,127,0.8)"));
}

/// grapheme インデックス → バイトオフセット変換。
/// core の `unicode::graphemes` を使い、差分判定と同じ分割規則を保つ。
fn grapheme_to_byte_range(text: &str, g: &std::ops::Range<usize>) -> (usize, usize) {
    let graphemes = unicode::graphemes(text);
    let base = text.as_ptr() as usize;
    let start = graphemes
        .get(g.start)
        .map(|s| s.as_ptr() as usize - base)
        .unwrap_or(text.len());
    let end = graphemes
        .get(g.end)
        .map(|s| s.as_ptr() as usize - base)
        .unwrap_or(text.len());
    (start, end)
}

fn kind_tag(kind: DiffRowKind) -> &'static str {
    match kind {
        DiffRowKind::Context => "",
        DiffRowKind::Added => "add",
        DiffRowKind::Removed => "del",
        DiffRowKind::Modified => "mod",
    }
}

impl TextDiffView {
    pub fn new() -> Rc<Self> {
        let left_buf = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
        let right_buf = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
        make_tags(&left_buf);
        make_tags(&right_buf);

        let mk_tv = |buf: &gtk4::TextBuffer| {
            let tv = gtk4::TextView::with_buffer(buf);
            tv.set_editable(false);
            tv.set_monospace(true);
            tv.set_wrap_mode(gtk4::WrapMode::None);
            tv.set_hexpand(true);
            tv.set_cursor_visible(false);
            tv
        };
        let left_tv = mk_tv(&left_buf);
        let right_tv = mk_tv(&right_buf);

        let panes = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        panes.append(&left_tv);
        panes.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        panes.append(&right_tv);
        let scroller = gtk4::ScrolledWindow::new();
        scroller.set_child(Some(&panes));
        scroller.set_vexpand(true);
        scroller.set_hexpand(true);

        // ツールバー: 差分ナビ + 検索 + 折りたたみ解除
        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        toolbar.add_css_class("dl-diff-toolbar");
        let prev_btn = gtk4::Button::with_label(&format!("↑ {}", s::PREV_DIFF));
        prev_btn.set_tooltip_text(Some(&format!("{} (Ctrl+↑)", s::PREV_DIFF)));
        let next_btn = gtk4::Button::with_label(&format!("↓ {}", s::NEXT_DIFF));
        next_btn.set_tooltip_text(Some(&format!("{} (Ctrl+↓)", s::NEXT_DIFF)));
        let counter = gtk4::Label::new(Some(s::DASH));
        let sep = gtk4::Separator::new(gtk4::Orientation::Vertical);
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some(s::SEARCH));
        let prev_match = gtk4::Button::with_label("↑");
        prev_match.set_tooltip_text(Some(&format!("{} (Ctrl+Shift+G)", s::PREV_MATCH)));
        let next_match = gtk4::Button::with_label("↓");
        next_match.set_tooltip_text(Some(&format!("{} (Ctrl+G)", s::NEXT_MATCH)));
        let match_counter = gtk4::Label::new(Some(s::DASH));
        let case_toggle = gtk4::ToggleButton::with_label("Aa");
        case_toggle.set_tooltip_text(Some(s::CASE_TOGGLE_TOOLTIP));
        let show_all = gtk4::CheckButton::with_label(s::SHOW_ALL);
        show_all.set_tooltip_text(Some(s::SHOW_ALL_TOOLTIP));
        toolbar.append(&prev_btn);
        toolbar.append(&next_btn);
        toolbar.append(&counter);
        toolbar.append(&sep);
        toolbar.append(&search);
        toolbar.append(&prev_match);
        toolbar.append(&next_match);
        toolbar.append(&match_counter);
        toolbar.append(&case_toggle);
        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        toolbar.append(&spacer);
        toolbar.append(&show_all);

        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        widget.append(&toolbar);
        widget.append(&scroller);
        widget.set_vexpand(true);
        widget.set_hexpand(true);

        let this = Rc::new(Self {
            widget,
            left_buf: left_buf.clone(),
            right_buf: right_buf.clone(),
            rows: RefCell::new(Vec::new()),
            expanded: RefCell::new(HashSet::new()),
            show_all: Cell::new(false),
            search_query: RefCell::new(String::new()),
            search_case: Cell::new(false),
            search_hits: RefCell::new(Vec::new()),
            diff_ids: RefCell::new(Vec::new()),
            cur_diff_pos: Cell::new(None),
            cur_match_pos: Cell::new(None),
            line_map: RefCell::new(Vec::new()),
            left_tv: left_tv.clone(),
            counter,
            match_counter,
        });

        // 折りたたみ行のクリックで展開(クリック位置 → 行 → グループ)
        for tv in [&left_tv, &right_tv] {
            let click = gtk4::GestureClick::new();
            let weak = Rc::downgrade(&this);
            let tvw = tv.clone();
            click.connect_released(move |_g, _n, x, y| {
                let Some(this) = weak.upgrade() else { return };
                let (bx, by) =
                    tvw.window_to_buffer_coords(gtk4::TextWindowType::Widget, x as i32, y as i32);
                let Some(iter) = tvw.iter_at_location(bx, by) else {
                    return;
                };
                let line = iter.line() as usize;
                let group = this.line_map.borrow().get(line).and_then(|t| match t {
                    LineTarget::Fold(id) => Some(*id),
                    _ => None,
                });
                if let Some(id) = group {
                    this.expanded.borrow_mut().insert(id);
                    this.render();
                }
            });
            tv.add_controller(click);
        }

        {
            let weak = Rc::downgrade(&this);
            prev_btn.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.goto_diff(false)
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            next_btn.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.goto_diff(true)
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            search.connect_search_changed(move |e| {
                if let Some(t) = weak.upgrade() {
                    *t.search_query.borrow_mut() = e.text().to_string();
                    t.refresh_search();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            case_toggle.connect_toggled(move |b| {
                if let Some(t) = weak.upgrade() {
                    t.search_case.set(b.is_active());
                    t.refresh_search();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            show_all.connect_toggled(move |b| {
                if let Some(t) = weak.upgrade() {
                    t.show_all.set(b.is_active());
                    t.render();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            prev_match.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.goto_match(false)
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            next_match.connect_clicked(move |_| {
                if let Some(t) = weak.upgrade() {
                    t.goto_match(true)
                }
            });
        }
        this
    }

    /// 公開: ツールバーの差分ナビ/検索をショートカットからも呼べるようにする
    pub fn goto_diff_pub(&self, next: bool) {
        self.goto_diff(next)
    }
    pub fn goto_match_pub(&self, next: bool) {
        self.goto_match(next)
    }

    /// 左右ファイルを読み込んで差分を構築する。
    /// どちらかがバイナリ/巨大/IO 失敗でも、読めた側のエンコード名は返す。
    pub fn load(&self, left: &Path, right: &Path, cc: &ContentComparison) -> TextLoadDetail {
        use diffloupe_core::text_loader::{self, TextLoadResult};
        let read = |p: &Path| -> Result<(String, &'static str), TextLoad> {
            match text_loader::load(p) {
                Ok(TextLoadResult::Text(t, enc)) => Ok((t, enc)),
                Ok(TextLoadResult::Binary) => Err(TextLoad::Binary),
                Ok(TextLoadResult::TooLarge) => Err(TextLoad::TooLarge),
                Err(e) => Err(TextLoad::Io(e.to_string())),
            }
        };
        let l = read(left);
        let r = read(right);
        let detail = TextLoadDetail {
            result: match (&l, &r) {
                (Ok(_), Ok(_)) => TextLoad::Loaded,
                (Err(e), _) | (_, Err(e)) => match e {
                    TextLoad::Binary => TextLoad::Binary,
                    TextLoad::TooLarge => TextLoad::TooLarge,
                    TextLoad::Io(m) => TextLoad::Io(m.clone()),
                    TextLoad::Loaded => TextLoad::Loaded,
                },
            },
            left_encoding: l.as_ref().ok().map(|(_, e)| *e),
            right_encoding: r.as_ref().ok().map(|(_, e)| *e),
        };
        if let (Ok((lt, _)), Ok((rt, _))) = (l, r) {
            self.load_texts(&lt, &rt, cc);
        }
        detail
    }

    /// デコード済みテキストから差分を構築(構造化正規化後の比較やスモークからも使う)。
    /// 戻り値: 行数上限超過なら false。
    pub fn load_texts(&self, left_text: &str, right_text: &str, cc: &ContentComparison) -> bool {
        let Some(rows) = line_diff::rows(left_text, right_text, cc) else {
            self.rows.borrow_mut().clear();
            self.render();
            return false;
        };
        *self.diff_ids.borrow_mut() = dd::difference_row_ids(&rows);
        *self.rows.borrow_mut() = rows;
        self.expanded.borrow_mut().clear();
        self.cur_diff_pos.set(None);
        self.cur_match_pos.set(None);
        self.render();
        self.refresh_search();
        true
    }

    fn update_counter(&self) {
        let n = self.diff_ids.borrow().len();
        match self.cur_diff_pos.get() {
            Some(i) if n > 0 => self.counter.set_text(&format!("{}/{}", i + 1, n)),
            _ if n > 0 => self.counter.set_text(&format!("{}/{}", s::DASH, n)),
            _ => self.counter.set_text(s::DASH),
        }
        let m = self.search_hits.borrow().len();
        match self.cur_match_pos.get() {
            Some(i) if m > 0 => self.match_counter.set_text(&format!("{}/{}", i + 1, m)),
            _ if m > 0 => self.match_counter.set_text(&format!("{}/{}", s::DASH, m)),
            _ => self.match_counter.set_text(s::DASH),
        }
    }

    /// バッファを現在の rows/expanded/search 状態で再構築する。
    /// `TextIter` は文字オフセット基準のため、行内バイト範囲は文字数へ変換してから使う。
    fn render(&self) {
        struct LineRender {
            target: LineTarget,
            text: String,
            lineno: Option<usize>,
            tag: &'static str,
            /// 行内バイト範囲(インライン強調 or 検索ヒット)
            byte_ranges: Vec<(usize, usize)>,
            inline_tag: &'static str,
        }

        let rows = self.rows.borrow();
        let expanded = self.expanded.borrow();
        let items = dd::display_items(
            &rows,
            &expanded,
            self.show_all.get(),
            dd::COLLAPSE_THRESHOLD,
        );
        let hits: HashSet<usize> = self.search_hits.borrow().iter().copied().collect();
        let inline = inline_diff::highlights_for_rows(&rows, false);
        let query = self.search_query.borrow().clone();
        let case = self.search_case.get();

        let mut line_map = Vec::with_capacity(items.len());
        let mut left_lines: Vec<LineRender> = Vec::with_capacity(items.len());
        let mut right_lines: Vec<LineRender> = Vec::with_capacity(items.len());

        for item in items.iter() {
            match item {
                dd::DisplayItem::Row(row) => {
                    line_map.push(LineTarget::Row(row.id));
                    let lt = row.left_text.clone().unwrap_or_default();
                    let rt = row.right_text.clone().unwrap_or_default();
                    let tag = kind_tag(row.kind);
                    let (mut lr, mut rr) = (Vec::new(), Vec::new());
                    if let Some(hl) = inline.get(&row.id) {
                        lr = hl
                            .left_ranges
                            .iter()
                            .map(|r| grapheme_to_byte_range(&lt, r))
                            .collect();
                        rr = hl
                            .right_ranges
                            .iter()
                            .map(|r| grapheme_to_byte_range(&rt, r))
                            .collect();
                    }
                    left_lines.push(LineRender {
                        target: LineTarget::Row(row.id),
                        text: lt,
                        lineno: row.left_number,
                        tag,
                        byte_ranges: lr,
                        inline_tag: "inline_l",
                    });
                    right_lines.push(LineRender {
                        target: LineTarget::Row(row.id),
                        text: rt,
                        lineno: row.right_number,
                        tag,
                        byte_ranges: rr,
                        inline_tag: "inline_r",
                    });
                }
                dd::DisplayItem::Collapsed(group) => {
                    line_map.push(LineTarget::Fold(group.id));
                    for lines in [&mut left_lines, &mut right_lines] {
                        lines.push(LineRender {
                            target: LineTarget::Fold(group.id),
                            text: s::fold_label(group.count),
                            lineno: None,
                            tag: "fold",
                            byte_ranges: Vec::new(),
                            inline_tag: "",
                        });
                    }
                }
            }
        }

        for (buf, lines) in [
            (&self.left_buf, &left_lines),
            (&self.right_buf, &right_lines),
        ] {
            let mut text = String::new();
            for line in lines.iter() {
                let num = line
                    .lineno
                    .map(|n| format!("{n:>5} "))
                    .unwrap_or_else(|| "      ".to_string());
                text.push_str(&num);
                text.push_str(&line.text);
                text.push('\n');
            }
            buf.set_text(&text);

            let tag_of = |name: &str| buf.tag_table().lookup(name);
            let lineno_tag = tag_of("lineno");
            let hit_tag = tag_of("hit");
            for (i, line) in lines.iter().enumerate() {
                let i = i as i32;
                if let Some(t) = &lineno_tag {
                    let (a, b) = (buf.iter_at_line_offset(i, 0), buf.iter_at_line_offset(i, 6));
                    if let (Some(a), Some(b)) = (a, b) {
                        buf.apply_tag(t, &a, &b);
                    }
                }
                match line.target {
                    LineTarget::Row(_) => {
                        if !line.tag.is_empty() {
                            if let Some(t) = tag_of(line.tag) {
                                // 行末の改行まで含めて段落全体に背景を適用
                                let a = buf.iter_at_line_offset(i, 0);
                                let b = buf.iter_at_line(i + 1).or_else(|| Some(buf.end_iter()));
                                if let (Some(a), Some(b)) = (a, b) {
                                    buf.apply_tag(&t, &a, &b);
                                }
                            }
                        }
                        if !line.inline_tag.is_empty() {
                            if let Some(t) = tag_of(line.inline_tag) {
                                for (a, b) in &line.byte_ranges {
                                    let ca = line.text[..*a].chars().count() as i32;
                                    let cb = line.text[..*b].chars().count() as i32;
                                    let (ia, ib) = (
                                        buf.iter_at_line_offset(i, 6 + ca),
                                        buf.iter_at_line_offset(i, 6 + cb),
                                    );
                                    if let (Some(ia), Some(ib)) = (ia, ib) {
                                        buf.apply_tag(&t, &ia, &ib);
                                    }
                                }
                            }
                        }
                        if !query.is_empty() && !line.text.is_empty() {
                            if let Some(t) = &hit_tag {
                                let is_hit = match line.target {
                                    LineTarget::Row(id) => hits.contains(&id),
                                    _ => false,
                                };
                                if is_hit {
                                    for (a, b) in find_ranges(&line.text, &query, case) {
                                        let ca = line.text[..a].chars().count() as i32;
                                        let cb = line.text[..b].chars().count() as i32;
                                        let (ia, ib) = (
                                            buf.iter_at_line_offset(i, 6 + ca),
                                            buf.iter_at_line_offset(i, 6 + cb),
                                        );
                                        if let (Some(ia), Some(ib)) = (ia, ib) {
                                            buf.apply_tag(t, &ia, &ib);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    LineTarget::Fold(_) => {
                        if let Some(t) = tag_of("fold") {
                            let a = buf.iter_at_line_offset(i, 6);
                            let b = buf.iter_at_line(i + 1).or_else(|| Some(buf.end_iter()));
                            if let (Some(a), Some(b)) = (a, b) {
                                buf.apply_tag(&t, &a, &b);
                            }
                        }
                    }
                }
            }
        }

        *self.line_map.borrow_mut() = line_map;
        self.update_counter();
    }

    /// 検索条件変更: ヒット行を再計算して再描画。
    fn refresh_search(&self) {
        let q = self.search_query.borrow().clone();
        if q.trim().is_empty() {
            self.search_hits.borrow_mut().clear();
        } else {
            *self.search_hits.borrow_mut() =
                dd::search_hit_row_ids(&self.rows.borrow(), &q, self.search_case.get());
        }
        self.cur_match_pos.set(None);
        self.render();
    }

    /// 対象行を含む折りたたみグループを自動展開する。
    /// 展開が発生したら true(再描画済み)。
    fn expand_for_row(&self, row_id: usize) -> bool {
        let rows = self.rows.borrow();
        let need = dd::expanded_group_ids(&[row_id], &rows, dd::COLLAPSE_THRESHOLD);
        let mut expanded = self.expanded.borrow_mut();
        let mut changed = false;
        for g in need {
            changed |= expanded.insert(g);
        }
        changed
    }

    fn row_to_buffer_line(&self, row_id: usize) -> Option<usize> {
        self.line_map.borrow().iter().position(|t| match t {
            LineTarget::Row(id) => *id == row_id,
            _ => false,
        })
    }

    fn scroll_to_row(&self, row_id: usize) {
        if self.expand_for_row(row_id) {
            self.render();
        }
        let Some(line) = self.row_to_buffer_line(row_id) else {
            return;
        };
        // 共通 ScrolledWindow 内の TextView へ scroll すれば両ペインが連動する
        {
            let tv = &self.left_tv;
            let iter = self
                .left_buf
                .iter_at_line(line as i32)
                .unwrap_or_else(|| self.left_buf.start_iter());
            let mark = self.left_buf.create_mark(None, &iter, true);
            tv.scroll_to_mark(&mark, 0.15, true, 0.0, 0.2);
        }
    }

    fn goto_diff(&self, next: bool) {
        let ids = self.diff_ids.borrow().clone();
        if ids.is_empty() {
            return;
        }
        let pos = match self.cur_diff_pos.get() {
            Some(p) => {
                if next {
                    (p + 1).min(ids.len() - 1)
                } else {
                    p.saturating_sub(1)
                }
            }
            // 未選択では「次」は先頭、「前」は末尾へ
            None => {
                if next {
                    0
                } else {
                    ids.len() - 1
                }
            }
        };
        self.cur_diff_pos.set(Some(pos));
        self.scroll_to_row(ids[pos]);
        self.update_counter();
    }

    fn goto_match(&self, next: bool) {
        let ids = self.search_hits.borrow().clone();
        if ids.is_empty() {
            return;
        }
        let pos = match self.cur_match_pos.get() {
            Some(p) => {
                if next {
                    (p + 1).min(ids.len() - 1)
                } else {
                    p.saturating_sub(1)
                }
            }
            None => {
                if next {
                    0
                } else {
                    ids.len() - 1
                }
            }
        };
        self.cur_match_pos.set(Some(pos));
        self.scroll_to_row(ids[pos]);
        self.update_counter();
    }

    pub fn clear(&self) {
        self.rows.borrow_mut().clear();
        self.left_buf.set_text("");
        self.right_buf.set_text("");
        self.update_counter();
    }
}

/// 行内のクエリ出現レンジ(バイト)。`case`=false なら大小無視。
fn find_ranges(line: &str, query: &str, case: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let (hay, needle) = if case {
        (line.to_string(), query.to_string())
    } else {
        (line.to_lowercase(), query.to_lowercase())
    };
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = hay[start..].find(&needle) {
        let a = start + pos;
        let b = a + needle.len();
        // バイト境界は小文字化でずれる可能性がある(トルコ語 i 等)。
        // lowercase 化は char 数を変えないケースがほぼすべてで、
        // 境界を UTF-8 char 境界に丸めて安全側に倒す。
        if line.is_char_boundary(a) && line.is_char_boundary(b) {
            out.push((a, b));
        }
        start = b.max(a + 1);
    }
    out
}
