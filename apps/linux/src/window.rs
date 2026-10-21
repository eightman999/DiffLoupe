//! メインウィンドウ。フォルダ選択・比較・結果ツリー・詳細・ステータスバー・
//! メニュー(レポート/コピー計画/3-way/設定)を束ねる。

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use diffloupe_core::report::{ComparisonReport, ReportFormat};
use diffloupe_core::sync_plan::SyncDirection;
use diffloupe_core::{CompareError, ComparisonProgress, FolderComparator};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::detail::DetailPane;
use crate::edition_ext::GtkEditionExtension;
use crate::model::{AppModel, ModelEvent, Phase, StatusFilter};
use crate::recent::{self, RecentComparison};
use crate::strings as s;
use crate::tree::ResultTree;

pub struct MainWindow {
    pub window: gtk4::ApplicationWindow,
    pub model: Rc<AppModel>,
    pub ext: Rc<dyn GtkEditionExtension>,
    pub tree: Rc<ResultTree>,
    pub detail: Rc<DetailPane>,

    /// 現在ツリーで選択中の相対パス(再比較での選択復元用)
    current_path: RefCell<Option<String>>,
    /// 比較完了後に選択し直すパス
    pending_select: RefCell<Option<String>>,

    left_path: gtk4::Label,
    right_path: gtk4::Label,
    left_ref_dd: gtk4::DropDown,
    right_ref_dd: gtk4::DropDown,
    left_refs: RefCell<Vec<String>>,
    right_refs: RefCell<Vec<String>>,
    compare_btn: gtk4::Button,
    cancel_btn: gtk4::Button,
    progress: gtk4::ProgressBar,
    progress_text: gtk4::Label,
    status: gtk4::Label,
    counts: gtk4::Label,
    watch_label: gtk4::Label,
    search: gtk4::SearchEntry,
    filter_dd: gtk4::DropDown,
    history_menu: gtk4::MenuButton,
    swap_btn: gtk4::Button,
    settings_btn: gtk4::Button,
    tw_btn: gtk4::Button,
}

impl MainWindow {
    pub fn new(
        app: &gtk4::Application,
        edition: Rc<dyn diffloupe_edition::Edition>,
        ext: Rc<dyn GtkEditionExtension>,
        app_name: &str,
    ) -> Rc<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<ModelEvent>();
        let model = AppModel::new(edition, tx, app_name);

        let settings = model.settings.borrow().clone();
        let window = gtk4::ApplicationWindow::new(app);
        model.set_main_window(&window);
        window.set_title(Some(app_name));
        window.set_default_size(settings.window_width, settings.window_height);
        window.set_size_request(900, 540);
        if settings.window_maximized {
            window.maximize();
        }

        // ── ヘッダバー ──
        let header = gtk4::HeaderBar::new();
        let history_menu = gtk4::MenuButton::new();
        history_menu.set_label(s::HISTORY);
        history_menu.set_tooltip_text(Some(s::HISTORY_TOOLTIP));
        let swap_btn = gtk4::Button::from_icon_name("media-flip-horizontal-symbolic");
        swap_btn.set_tooltip_text(Some(s::SWAP_TOOLTIP));
        let tw_btn = gtk4::Button::with_label(s::THREE_WAY);
        tw_btn.set_tooltip_text(Some(s::THREE_WAY_TOOLTIP));
        let settings_btn = gtk4::Button::from_icon_name("emblem-system-symbolic");
        settings_btn.set_tooltip_text(Some(s::SETTINGS));

        // メニュー(レポート / コピー計画)
        let menu = gio::Menu::new();
        let sec_report = gio::Menu::new();
        sec_report.append(Some(s::REPORT_HTML), Some("win.save-report::html"));
        sec_report.append(Some(s::REPORT_CSV), Some("win.save-report::csv"));
        sec_report.append(Some(s::REPORT_JSON), Some("win.save-report::json"));
        sec_report.append(Some(s::REPORT_MD), Some("win.save-report::md"));
        menu.append_section(Some(s::MENU_REPORT), &sec_report);
        let sec_plan = gio::Menu::new();
        sec_plan.append(Some(s::PLAN_LTR), Some("win.sync-plan::ltr"));
        sec_plan.append(Some(s::PLAN_RTL), Some("win.sync-plan::rtl"));
        menu.append_section(Some(s::MENU_PLAN), &sec_plan);
        let sec_tw = gio::Menu::new();
        sec_tw.append(Some(s::MENU_3WAY_ITEM), Some("win.three-way"));
        menu.append_section(Some(s::MENU_3WAY), &sec_tw);
        let menu_btn = gtk4::MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_menu_model(Some(&menu));

        let compare_btn = gtk4::Button::with_label(s::COMPARE);
        compare_btn.add_css_class("suggested-action");
        let cancel_btn = gtk4::Button::with_label(s::CANCEL);
        cancel_btn.set_visible(false);
        header.pack_start(&history_menu);
        header.pack_start(&swap_btn);
        for w in ext.header_widgets(&model) {
            header.pack_start(&w);
        }
        header.pack_end(&menu_btn);
        header.pack_end(&compare_btn);
        header.pack_end(&cancel_btn);
        header.pack_end(&settings_btn);
        header.pack_end(&tw_btn);
        window.set_titlebar(Some(&header));

        // ── フォルダバー ──
        let folder_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        folder_bar.set_margin_start(8);
        folder_bar.set_margin_end(8);
        folder_bar.set_margin_top(6);
        let (left_box, left_path, left_ref_dd) = folder_side(s::LEFT_FOLDER);
        let (right_box, right_path, right_ref_dd) = folder_side(s::RIGHT_FOLDER);
        folder_bar.append(&left_box);
        folder_bar.append(&right_box);

        // ── 進捗 ──
        let progress = gtk4::ProgressBar::new();
        progress.set_visible(false);
        let progress_text = gtk4::Label::new(None);
        progress_text.set_visible(false);

        // ── ツリーペイン ──
        let filter_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some(s::SEARCH_PLACEHOLDER));
        search.set_hexpand(true);
        let filter_names: Vec<&str> = StatusFilter::ALL.iter().map(|f| f.display_name()).collect();
        let filter_dd = gtk4::DropDown::from_strings(filter_names.as_slice());
        filter_dd.set_tooltip_text(Some(s::FILTER_TOOLTIP));
        filter_bar.append(&search);
        filter_bar.append(&filter_dd);
        filter_bar.set_margin_start(6);
        filter_bar.set_margin_end(6);
        filter_bar.set_margin_top(4);

        let tree_holder = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        tree_holder.append(&filter_bar);
        tree_holder.set_vexpand(true);

        let detail = DetailPane::new(ext.clone());
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
        paned.set_vexpand(true);
        paned.set_hexpand(true);

        // ── ステータスバー ──
        let status = gtk4::Label::new(Some(s::STATUS_PROMPT));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        let counts = gtk4::Label::new(None);
        let watch_label = gtk4::Label::new(None);
        watch_label.set_visible(false);
        let statusbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        statusbar.add_css_class("dl-statusbar");
        statusbar.append(&watch_label);
        statusbar.append(&status);
        statusbar.append(&counts);

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.append(&folder_bar);
        root.append(&progress);
        root.append(&progress_text);
        root.append(&paned);
        root.append(&statusbar);
        window.set_child(Some(&root));

        let this = Rc::new(Self {
            window: window.clone(),
            model: model.clone(),
            ext,
            tree: ResultTree::new(),
            detail: detail.clone(),
            current_path: RefCell::new(None),
            pending_select: RefCell::new(None),
            left_path: left_path.clone(),
            right_path: right_path.clone(),
            left_ref_dd: left_ref_dd.clone(),
            right_ref_dd: right_ref_dd.clone(),
            left_refs: RefCell::new(Vec::new()),
            right_refs: RefCell::new(Vec::new()),
            compare_btn: compare_btn.clone(),
            cancel_btn: cancel_btn.clone(),
            progress: progress.clone(),
            progress_text: progress_text.clone(),
            status,
            counts,
            watch_label,
            search: search.clone(),
            filter_dd: filter_dd.clone(),
            history_menu: history_menu.clone(),
            swap_btn: swap_btn.clone(),
            settings_btn: settings_btn.clone(),
            tw_btn: tw_btn.clone(),
        });

        paned.set_start_child(Some(&tree_holder));
        paned.set_end_child(Some(&detail.widget));
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);
        tree_holder.append(&this.tree.root_widget);

        // ツリー選択/コンテキストの結線
        {
            let weak = Rc::downgrade(&this);
            this.tree.set_on_select(Rc::new(move |path| {
                if let Some(w) = weak.upgrade() {
                    w.on_tree_select(&path);
                }
            }));
        }
        {
            let weak = Rc::downgrade(&this);
            this.tree.set_on_context(Rc::new(move |path, x, y, w| {
                if let Some(this) = weak.upgrade() {
                    this.open_context_menu(&path, x, y, &w);
                }
            }));
        }

        this.wire(&model);
        this.wire_actions();
        this.ext.window_ready(&this);
        this.update_watch_label();

        // モデルイベントの受信(ワーカー→メインは mpsc + 周期ドレイン)
        {
            let weak = Rc::downgrade(&this);
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                let Some(this) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                while let Ok(ev) = rx.try_recv() {
                    this.handle_event(ev);
                }
                glib::ControlFlow::Continue
            });
        }

        // ウィンドウ位置・サイズ保存。閉じる前に未保存編集の確認フックを通す
        // (Free は常に true。false = キャンセルされたので閉じない)
        {
            let m = model.clone();
            window.connect_close_request(move |w| {
                if !m.edition.confirm_discard_unsaved_changes() {
                    return glib::Propagation::Stop;
                }
                let mut s = m.settings.borrow_mut();
                let (wd, ht) = w.default_size();
                if !w.is_maximized() {
                    s.window_width = wd;
                    s.window_height = ht;
                }
                s.window_maximized = w.is_maximized();
                s.save();
                glib::Propagation::Proceed
            });
        }

        this.refresh_history_menu();
        this
    }

    // ── イベント処理 ──

    fn handle_event(&self, ev: ModelEvent) {
        match ev {
            ModelEvent::Progress(p) => self.on_progress(p),
            ModelEvent::Finished(res) => self.on_finished(res),
            ModelEvent::WatchChanged => {
                AppModel::on_watch_changed(&self.model);
                self.update_watch_label();
            }
        }
    }

    fn on_progress(&self, p: ComparisonProgress) {
        match p {
            ComparisonProgress::Scanning { found } => {
                self.progress.pulse();
                self.progress_text.set_text(&s::status_scanning(found));
            }
            ComparisonProgress::Comparing { done, total } => {
                self.progress.set_fraction(if total > 0 {
                    done as f64 / total as f64
                } else {
                    0.0
                });
                self.progress_text
                    .set_text(&s::status_comparing(done, total));
            }
            ComparisonProgress::Renames { done, total } => {
                self.progress.set_fraction(if total > 0 {
                    done as f64 / total as f64
                } else {
                    0.0
                });
                self.progress_text.set_text(&s::status_renames(done, total));
            }
        }
    }

    fn on_finished(&self, res: Result<diffloupe_core::ComparisonResult, CompareError>) {
        self.progress.set_visible(false);
        self.progress_text.set_visible(false);
        self.compare_btn.set_visible(true);
        self.cancel_btn.set_visible(false);
        match res {
            Ok(result) => {
                let summary = result.summary;
                *self.model.result.borrow_mut() = Some(Rc::new(result));
                self.model.on_finished(true);
                self.rebuild_tree();
                self.update_counts(&summary);
                self.status.set_text("");
                self.record_history();
                // 再比較前の選択を復元(見つからなければ詳細は空のまま)
                if let Some(p) = self.pending_select.borrow_mut().take() {
                    self.tree.select_path(&p);
                }
            }
            Err(CompareError::Cancelled) => {
                self.model.on_finished(false);
                self.status.set_text(s::CANCEL);
            }
            Err(CompareError::EntryLimitExceeded(e)) => {
                self.model.on_finished(false);
                self.show_limit_alert(e.limit);
            }
            Err(CompareError::Io(msg)) => {
                self.model.on_finished(false);
                self.status.set_text(&s::status_failed(&msg));
            }
        }
        self.pending_select.borrow_mut().take();
        self.update_watch_label();
        self.ext.comparison_changed(&self.model);
    }

    fn update_counts(&self, sm: &diffloupe_core::ComparisonSummary) {
        self.counts.set_text(&format!(
            "{} {}   {} {}   {} {}   {} {}   {} {}    計 {} ファイル",
            s::FILTER_IDENTICAL,
            sm.identical_files,
            s::FILTER_DIFFERENT,
            sm.different_files,
            s::FILTER_LEFT_ONLY,
            sm.left_only_files,
            s::FILTER_RIGHT_ONLY,
            sm.right_only_files,
            s::FILTER_ERROR,
            sm.error_files,
            sm.total_files(),
        ));
    }

    fn show_limit_alert(&self, limit: usize) {
        let dlg = gtk4::AlertDialog::builder()
            .message(s::LIMIT_TITLE)
            .detail(format!(
                "Freeは{limit}ファイルまで比較できます。Proで制限を解除できます。"
            ))
            .buttons([s::OPEN_SETTINGS, s::CLOSE])
            .default_button(1)
            .cancel_button(1)
            .build();
        let parent: gtk4::Window = self.window.clone().upcast();
        let w = self.window.clone().upcast::<gtk4::Window>();
        let m = self.model.clone();
        let ext = self.ext.clone();
        dlg.choose(Some(&parent), None::<&gio::Cancellable>, move |choice| {
            if choice == Ok(0) {
                crate::settingsui::open(&w, &m, &ext);
            }
        });
        self.status
            .set_text(&format!("Freeは{limit}ファイルまで比較できます"));
    }

    fn rebuild_tree(&self) {
        let Some(result) = self.model.result.borrow().clone() else {
            self.tree.clear();
            return;
        };
        let filter = StatusFilter::ALL[self.filter_dd.selected() as usize];
        let query = self.search.text().to_string();
        let entries = AppModel::filtered_entries(&result, filter, &query);
        let root = FolderComparator::build_tree(&entries);
        let empty_msg = match filter {
            StatusFilter::AllDiffs => s::EMPTY_NO_DIFFS,
            _ if !query.trim().is_empty() => s::EMPTY_NO_MATCH,
            _ => s::EMPTY_NO_FILES,
        };
        self.tree.set_result(&root, empty_msg);
    }

    pub fn on_tree_select(&self, path: &str) {
        *self.current_path.borrow_mut() = Some(path.to_string());
        let Some(result) = self.model.result.borrow().clone() else {
            return;
        };
        if let Some(entry) = result.entry_by_path(path) {
            self.detail.show_entry(entry, &self.model);
        }
    }

    fn open_context_menu(&self, path: &str, x: f64, y: f64, anchor_row: &gtk4::Widget) {
        let Some(result) = self.model.result.borrow().clone() else {
            return;
        };
        let Some(entry) = result.entry_by_path(path).cloned() else {
            return;
        };
        let pop = gtk4::Popover::new();
        pop.set_has_arrow(true);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        vbox.set_margin_top(6);
        vbox.set_margin_bottom(6);
        vbox.set_margin_start(6);
        vbox.set_margin_end(6);
        for (side_label, path_opt) in [
            (s::SIDE_LEFT, entry.left_path.clone()),
            (s::SIDE_RIGHT, entry.right_path.clone()),
        ] {
            let head = gtk4::Label::new(Some(side_label));
            head.set_xalign(0.0);
            head.add_css_class("heading");
            vbox.append(&head);
            for (label, action) in [
                (s::OPEN, CtxAction::Open),
                (s::REVEAL, CtxAction::Reveal),
                (s::COPY_PATH, CtxAction::CopyPath),
            ] {
                let btn = gtk4::Button::with_label(label);
                btn.add_css_class("flat");
                btn.set_sensitive(path_opt.is_some());
                if let Some(p) = path_opt.clone() {
                    let pop = pop.clone();
                    btn.connect_clicked(move |_| {
                        let file = gio::File::for_path(&p);
                        match action {
                            CtxAction::Open => {
                                let l = gtk4::FileLauncher::new(Some(&file));
                                l.launch(None::<&gtk4::Window>, None::<&gio::Cancellable>, |_| {});
                            }
                            CtxAction::Reveal => {
                                let l = gtk4::FileLauncher::new(Some(&file));
                                l.open_containing_folder(
                                    None::<&gtk4::Window>,
                                    None::<&gio::Cancellable>,
                                    |_| {},
                                );
                            }
                            CtxAction::CopyPath => {
                                if let Some(display) = gdk::Display::default() {
                                    display.clipboard().set_text(&p.to_string_lossy());
                                }
                            }
                        }
                        pop.popdown();
                    });
                }
                vbox.append(&btn);
            }
        }
        pop.set_child(Some(&vbox));
        pop.set_parent(anchor_row);
        pop.popup();
    }

    fn record_history(&self) {
        let (Some(l), Some(r)) = (
            self.model.left_folder.borrow().clone(),
            self.model.right_folder.borrow().clone(),
        ) else {
            return;
        };
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        recent::record(RecentComparison {
            left_path: l.to_string_lossy().into_owned(),
            right_path: r.to_string_lossy().into_owned(),
            left_git_ref: self.model.left_ref.borrow().clone(),
            right_git_ref: self.model.right_ref.borrow().clone(),
            date: diffloupe_core::report::iso8601(secs),
        });
        self.refresh_history_menu();
    }

    fn refresh_history_menu(&self) {
        let menu = gio::Menu::new();
        let items = recent::load();
        if items.is_empty() {
            // 空履歴: 無効な項目を1つ表示
            let sec = gio::Menu::new();
            sec.append(Some(s::HISTORY_EMPTY), Some("win.noop"));
            menu.append_section(None, &sec);
        } else {
            for (i, item) in items.iter().enumerate() {
                // 詳細アクション名は文字列パラメータを引用符で渡す
                let label = item.display_label();
                let action = format!("win.restore-history('{i}')");
                menu.append(Some(label.as_str()), Some(action.as_str()));
            }
        }
        self.history_menu.set_menu_model(Some(&menu));
    }

    fn restore_history(&self, idx: usize) {
        let items = recent::load();
        let Some(item) = items.get(idx) else { return };
        if !item.is_restorable() {
            return;
        }
        if !self.model.edition.confirm_discard_unsaved_changes() {
            return;
        }
        self.model.set_folder(true, PathBuf::from(&item.left_path));
        self.model
            .set_folder(false, PathBuf::from(&item.right_path));
        *self.model.left_ref.borrow_mut() = item.left_git_ref.clone();
        *self.model.right_ref.borrow_mut() = item.right_git_ref.clone();
        self.refresh_folder_ui();
        self.start_compare(true);
    }

    fn update_watch_label(&self) {
        if !self.model.settings.borrow().auto_refresh || self.model.result.borrow().is_none() {
            self.watch_label.set_visible(false);
            return;
        }
        self.watch_label.set_visible(true);
        if self.model.watch_paused.get() {
            self.watch_label.set_text(s::WATCH_PAUSED);
            self.watch_label
                .set_tooltip_text(Some(&s::watch_pause_reason(
                    crate::model::MAX_AUTO_REFRESH_RUNS,
                )));
        } else {
            self.watch_label.set_text(s::WATCH_ACTIVE);
            self.watch_label
                .set_tooltip_text(Some(s::WATCH_ACTIVE_TOOLTIP));
        }
    }

    // ── 配線 ──

    fn wire(self: &Rc<Self>, model: &Rc<AppModel>) {
        // フォルダ選択ボタン + D&D
        self.wire_folder_side(true);
        self.wire_folder_side(false);

        // 比較/キャンセル
        {
            let this = self.clone();
            self.compare_btn
                .connect_clicked(move |_| this.start_compare(true));
        }
        {
            let m = model.clone();
            self.cancel_btn.connect_clicked(move |_| m.cancel_compare());
        }
        // 交換/設定/3-way ボタン
        {
            let this = self.clone();
            self.swap_btn.connect_clicked(move |_| this.do_swap());
        }
        {
            let this = self.clone();
            self.settings_btn.connect_clicked(move |_| {
                crate::settingsui::open(&this.window, &this.model, &this.ext);
            });
        }
        {
            let this = self.clone();
            self.tw_btn.connect_clicked(move |_| {
                gtk4::gio::prelude::ActionGroupExt::activate_action(
                    &this.window,
                    "three-way",
                    None,
                );
            });
        }
        // 検索/フィルタ
        {
            let this = self.clone();
            self.search
                .connect_search_changed(move |_| this.rebuild_tree());
        }
        {
            let this = self.clone();
            self.filter_dd
                .connect_selected_notify(move |_| this.rebuild_tree());
        }
        // ref ドロップダウン
        for is_left in [true, false] {
            let dd = if is_left {
                self.left_ref_dd.clone()
            } else {
                self.right_ref_dd.clone()
            };
            let this = self.clone();
            dd.connect_selected_notify(move |d| {
                this.on_ref_selected(is_left, d.selected());
            });
        }
    }

    fn wire_actions(self: &Rc<Self>) {
        // パラメータなしアクション + ショートカット
        let mk = |name: &str, f: Rc<dyn Fn()>, accels: &[&str]| {
            let a = gio::SimpleAction::new(name, None);
            let f = f.clone();
            a.connect_activate(move |_, _| f());
            self.window.add_action(&a);
            if !accels.is_empty() {
                if let Some(app) = self.window.application() {
                    app.set_accels_for_action(&format!("win.{name}"), accels);
                }
            }
        };
        mk(
            "compare",
            Rc::new({
                let this = self.clone();
                move || this.start_compare(true)
            }),
            &["<Ctrl>r"],
        );
        mk(
            "swap",
            Rc::new({
                let this = self.clone();
                move || this.do_swap()
            }),
            &["<Ctrl><Shift>s"],
        );
        mk(
            "next-diff",
            Rc::new({
                let this = self.clone();
                move || this.detail.text.goto_diff_pub(true)
            }),
            &["<Ctrl>Down"],
        );
        mk(
            "prev-diff",
            Rc::new({
                let this = self.clone();
                move || this.detail.text.goto_diff_pub(false)
            }),
            &["<Ctrl>Up"],
        );
        mk(
            "next-match",
            Rc::new({
                let this = self.clone();
                move || this.detail.text.goto_match_pub(true)
            }),
            &["<Ctrl>g"],
        );
        mk(
            "prev-match",
            Rc::new({
                let this = self.clone();
                move || this.detail.text.goto_match_pub(false)
            }),
            &["<Ctrl><Shift>g"],
        );
        mk("noop", Rc::new(|| {}), &[]);
        // 終了(Ctrl+Q): ウィンドウの close-request と同じ確認を通してから閉じる
        mk(
            "quit",
            Rc::new({
                let this = self.clone();
                move || this.window.close()
            }),
            &["<Ctrl>q"],
        );
        mk(
            "three-way",
            Rc::new({
                let this = self.clone();
                move || {
                    if this.model.edition.capabilities().three_way_view {
                        if let Some(w) = this.ext.three_way_view(&this.model) {
                            let dlg = gtk4::Window::new();
                            dlg.set_title(Some(s::TW_MERGE_TITLE));
                            dlg.set_transient_for(Some(&this.window));
                            dlg.set_modal(true);
                            dlg.set_destroy_with_parent(true);
                            dlg.set_default_size(1000, 680);
                            dlg.set_child(Some(&w));
                            dlg.present();
                        }
                    } else {
                        crate::threeway::open(&this.window, &this.model);
                    }
                }
            }),
            &[],
        );
        // パラメータ付きアクション
        let act = |name: &str| {
            let a = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            self.window.add_action(&a);
            a
        };
        let save_report = act("save-report");
        {
            let this = self.clone();
            save_report.connect_activate(move |_, v| {
                let fmt: String = v.and_then(|v| v.get()).unwrap_or_else(|| "html".into());
                this.save_report(fmt.as_str());
            });
        }
        let sync_plan = act("sync-plan");
        {
            let this = self.clone();
            sync_plan.connect_activate(move |_, v| {
                let dir = match v
                    .and_then(|v| v.get::<String>())
                    .as_deref()
                    .unwrap_or("ltr")
                {
                    "rtl" => SyncDirection::RightToLeft,
                    _ => SyncDirection::LeftToRight,
                };
                crate::syncplan::open(&this.window, &this.model, &this.ext, dir);
            });
        }
        let restore = act("restore-history");
        {
            let this = self.clone();
            restore.connect_activate(move |_, v| {
                if let Some(i) = v
                    .and_then(|v| v.get::<String>())
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    this.restore_history(i);
                }
            });
        }
    }

    /// 片側フォルダの選択ボタン・D&D・ref ドロップダウンの結線。
    fn wire_folder_side(self: &Rc<Self>, is_left: bool) {
        // 側ボックスは folder_side() が作った Frame; 子から辿る
        let (path_lbl, dd) = if is_left {
            (self.left_path.clone(), self.left_ref_dd.clone())
        } else {
            (self.right_path.clone(), self.right_ref_dd.clone())
        };
        let Some(vbox) = path_lbl
            .parent()
            .and_then(|w| w.parent())
            .and_then(|w| w.downcast::<gtk4::Box>().ok())
        else {
            return;
        };
        // 選択ボタン = vbox の最後の子
        if let Some(btn) = vbox
            .last_child()
            .and_then(|w| w.downcast::<gtk4::Button>().ok())
        {
            let this = self.clone();
            btn.connect_clicked(move |_| this.pick_folder(is_left));
        }
        // D&D
        // GTK4 のフォルダ D&D は FileList で届く(1件でも FileList)
        let drop = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        {
            let this = self.clone();
            drop.connect_drop(move |_t, value, _x, _y| {
                let file = value
                    .get::<gdk::FileList>()
                    .ok()
                    .and_then(|fl| fl.files().into_iter().next());
                let Some(f) = file else { return false };
                let Some(p) = f.path() else { return false };
                if !p.is_dir() {
                    return false;
                }
                // 実行中は拒否
                if this.model.phase.get() == Phase::Running {
                    return false;
                }
                this.apply_folder(is_left, p);
                true
            });
        }
        vbox.add_controller(drop);
        let _ = dd;
    }

    fn pick_folder(self: &Rc<Self>, is_left: bool) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title(if is_left {
            s::SELECT_LEFT_FOLDER
        } else {
            s::SELECT_RIGHT_FOLDER
        });
        dialog.set_accept_label(Some(s::SELECT));
        let this = self.clone();
        dialog.select_folder(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(p) = file.path() {
                    this.apply_folder(is_left, p);
                }
            }
        });
    }

    /// フォルダ適用(選択・D&D 共通)。シンボリックリンク解決・git 検出・ref 初期化。
    pub fn apply_folder(self: &Rc<Self>, is_left: bool, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        self.model.set_folder(is_left, path);
        self.refresh_folder_ui();
    }

    /// フォルダ表示と ref ドロップダウンを現在のモデル状態へ反映する。
    pub fn refresh_folder_ui(&self) {
        for is_left in [true, false] {
            let (folder, git) = if is_left {
                (
                    self.model.left_folder.borrow().clone(),
                    self.model.left_git.borrow().clone(),
                )
            } else {
                (
                    self.model.right_folder.borrow().clone(),
                    self.model.right_git.borrow().clone(),
                )
            };
            let (lbl, dd) = if is_left {
                (self.left_path.clone(), self.left_ref_dd.clone())
            } else {
                (self.right_path.clone(), self.right_ref_dd.clone())
            };
            match &folder {
                Some(p) => lbl.set_text(&p.to_string_lossy()),
                None => lbl.set_text(s::PATH_PLACEHOLDER),
            }
            // ref ドロップダウンは git リポジトリ検出時のみ表示
            if let Some(repo) = git {
                let mut items = vec![s::WORKING_TREE.to_string()];
                items.extend(repo.list_refs());
                items.push(s::ENTER_REF.to_string());
                *if is_left {
                    self.left_refs.borrow_mut()
                } else {
                    self.right_refs.borrow_mut()
                } = items.clone();
                dd.set_model(Some(&gtk4::StringList::new(
                    &items.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
                )));
                dd.set_visible(true);
                dd.set_selected(0);
            } else {
                dd.set_visible(false);
            }
        }
    }

    fn on_ref_selected(&self, is_left: bool, idx: u32) {
        let refs = if is_left {
            self.left_refs.borrow().clone()
        } else {
            self.right_refs.borrow().clone()
        };
        let Some(name) = refs.get(idx as usize) else {
            return;
        };
        let ref_slot = if is_left {
            &self.model.left_ref
        } else {
            &self.model.right_ref
        };
        if name == s::WORKING_TREE {
            *ref_slot.borrow_mut() = None;
        } else if name == s::ENTER_REF {
            self.ask_ref(is_left);
        } else {
            *ref_slot.borrow_mut() = Some(name.clone());
        }
    }

    /// 任意 ref 入力ダイアログ。
    fn ask_ref(&self, is_left: bool) {
        let dlg = gtk4::Window::new();
        dlg.set_title(Some(s::REF_NAME));
        dlg.set_transient_for(Some(&self.window));
        dlg.set_modal(true);
        dlg.set_destroy_with_parent(true);
        let b = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        b.set_margin_top(12);
        b.set_margin_bottom(12);
        b.set_margin_start(12);
        b.set_margin_end(12);
        let entry = gtk4::Entry::new();
        entry.set_placeholder_text(Some(s::REF_PLACEHOLDER));
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let cancel = gtk4::Button::with_label(s::CANCEL);
        let apply = gtk4::Button::with_label(s::APPLY);
        apply.add_css_class("suggested-action");
        row.append(&cancel);
        row.append(&apply);
        row.set_halign(gtk4::Align::End);
        b.append(&gtk4::Label::new(Some(s::REF_NAME)));
        b.append(&entry);
        b.append(&row);
        dlg.set_child(Some(&b));
        let m = self.model.clone();
        let d = dlg.clone();
        apply.connect_clicked(move |_| {
            let text = entry.text().to_string();
            let v = text.trim();
            if !v.is_empty() {
                *if is_left {
                    m.left_ref.borrow_mut()
                } else {
                    m.right_ref.borrow_mut()
                } = Some(v.to_string());
            }
            d.close();
        });
        let d2 = dlg.clone();
        cancel.connect_clicked(move |_| d2.close());
        dlg.present();
    }

    fn do_swap(&self) {
        if self.model.swap_sides() {
            self.refresh_folder_ui();
            self.detail.clear();
            self.tree.clear();
            self.status.set_text(s::STATUS_PROMPT);
            self.counts.set_text("");
            *self.current_path.borrow_mut() = None;
            self.ext.comparison_changed(&self.model);
        }
    }

    /// 比較開始(UI 側の状態遷移込み)。
    pub fn start_compare(&self, user_initiated: bool) {
        if user_initiated {
            self.model.reset_auto_runs();
        }
        if self.model.begin_compare(user_initiated) {
            self.compare_btn.set_visible(false);
            self.cancel_btn.set_visible(true);
            self.progress.set_visible(true);
            self.progress.set_fraction(0.0);
            self.progress_text.set_visible(true);
            self.detail.clear();
            *self.current_path.borrow_mut() = None;
            self.ext.comparison_changed(&self.model);
        }
    }

    /// 再比較する(同期後の「再比較」など)。`preserve_selection` なら完了後に
    /// 直前の選択パスを復元する。未保存編集の確認フックは通常の比較と同じく通る。
    pub fn recompare(&self, preserve_selection: bool) {
        let path = if preserve_selection {
            self.current_path.borrow().clone()
        } else {
            None
        };
        *self.pending_select.borrow_mut() = path;
        self.start_compare(true);
        if self.model.phase.get() != Phase::Running {
            // 開始できなかった(キャンセル等)ときは保留を捨てる
            self.pending_select.borrow_mut().take();
        }
    }

    fn save_report(&self, fmt: &str) {
        let Some(result) = self.model.result.borrow().clone() else {
            return;
        };
        let format = match fmt {
            "csv" => ReportFormat::Csv,
            "json" => ReportFormat::Json,
            "md" => ReportFormat::Markdown,
            _ => ReportFormat::Html,
        };
        let mut report = ComparisonReport::new(
            &result,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        );
        report.left_git_ref = self.model.left_ref.borrow().clone();
        report.right_git_ref = self.model.right_ref.borrow().clone();
        let text = report.render(format);
        let dialog = gtk4::FileDialog::new();
        dialog.set_title(s::SAVE_REPORT);
        dialog.set_initial_name(Some(&s::report_default_name(format.file_extension())));
        dialog.save(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(p) = file.path() {
                    let _ = std::fs::write(p, text);
                }
            }
        });
    }
}

enum CtxAction {
    Open,
    Reveal,
    CopyPath,
}

/// フォルダ選択1側分のウィジェット(ラベル + パス + 選択ボタン + ref ドロップダウン)。
fn folder_side(title: &str) -> (gtk4::Box, gtk4::Label, gtk4::DropDown) {
    let frame = gtk4::Frame::new(None);
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    vbox.set_margin_start(8);
    vbox.set_margin_end(8);
    vbox.set_margin_top(6);
    vbox.set_margin_bottom(6);
    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let lab = gtk4::Label::new(Some(title));
    lab.add_css_class("heading");
    let path = gtk4::Label::new(Some(s::PATH_PLACEHOLDER));
    path.set_xalign(0.0);
    path.set_hexpand(true);
    path.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    head.append(&lab);
    head.append(&path);
    let ref_dd = gtk4::DropDown::from_strings(&[s::WORKING_TREE]);
    ref_dd.set_visible(false);
    head.append(&ref_dd);
    let btn = gtk4::Button::with_label(s::SELECT);
    vbox.append(&head);
    vbox.append(&btn);
    frame.set_child(Some(&vbox));
    frame.set_hexpand(true);
    // 戻り値は (frame相当の外側Box, path, dd)。frame を Box で包む
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    outer.append(&frame);
    outer.set_hexpand(true);
    (outer, path, ref_dd)
}
