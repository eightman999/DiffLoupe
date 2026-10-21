//! アプリの状態と比較オーケストレーション(Swift `AppModel` 相当)。
//! GTK スレッドから所有するため `Rc`/`RefCell` ベース。
//! 比較はワーカースレッドで実行し、進捗・完了を `glib::MainContext` 経由で返す。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use diffloupe_core::git_snapshot::GitRepository;
use diffloupe_core::watcher::DirectoryWatcher;
use diffloupe_core::{
    CancellationToken, CompareError, CompareOptions, ComparisonProgress, ComparisonResult,
    DiffStatus, FolderComparator,
};
use diffloupe_edition::DiffLoupeEdition;

use crate::settings::{normalize_patterns, Settings};

/// モデル → ウィンドウへの通知。`glib::MainContext::channel` で運ぶ。
pub enum ModelEvent {
    Progress(ComparisonProgress),
    Finished(Result<ComparisonResult, CompareError>),
    /// ディレクトリ監視が変更を検知した(ワーカースレッド発火)
    WatchChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
    Done,
}

pub struct AppModel {
    pub edition: Rc<dyn DiffLoupeEdition>,
    /// 表示名("DiffLoupe" / "DiffLoupe Pro")。`AppConfig::app_name`
    pub app_name: String,
    main_window: RefCell<Option<gtk4::glib::WeakRef<gtk4::ApplicationWindow>>>,
    pub settings: RefCell<Settings>,

    pub left_folder: RefCell<Option<PathBuf>>,
    pub right_folder: RefCell<Option<PathBuf>>,
    pub left_git: RefCell<Option<Rc<GitRepository>>>,
    pub right_git: RefCell<Option<Rc<GitRepository>>>,
    /// None = 作業ツリー。Some(ref) = git スナップショットで比較。
    pub left_ref: RefCell<Option<String>>,
    pub right_ref: RefCell<Option<String>>,

    pub result: RefCell<Option<Rc<ComparisonResult>>>,
    pub phase: Cell<Phase>,
    cancel: RefCell<Option<Arc<CancellationToken>>>,
    /// 比較結果が参照している git スナップショット(詳細表示が使うため保持)
    snapshots: RefCell<Vec<PathBuf>>,
    /// git ref 選択時の実比較ルート(スナップショット or 作業ツリー)
    pub effective_left: RefCell<Option<PathBuf>>,
    pub effective_right: RefCell<Option<PathBuf>>,

    watcher: RefCell<Option<Rc<DirectoryWatcher>>>,
    /// 連続自動再比較回数(上限 max_auto_refresh_runs で一時停止)
    pub auto_runs: Cell<usize>,
    pub watch_paused: Cell<bool>,
    /// 比較中に検知した変更(完了後に1回だけ再比較)
    pub watch_pending: Cell<bool>,

    events: RefCell<Option<mpsc::Sender<ModelEvent>>>,
}

pub const MAX_AUTO_REFRESH_RUNS: usize = 3;
pub const WATCH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

impl AppModel {
    pub fn new(
        edition: Rc<dyn DiffLoupeEdition>,
        events: mpsc::Sender<ModelEvent>,
        app_name: &str,
    ) -> Rc<Self> {
        Rc::new(Self {
            edition,
            app_name: app_name.to_string(),
            main_window: RefCell::new(None),
            settings: RefCell::new(Settings::load()),
            left_folder: RefCell::new(None),
            right_folder: RefCell::new(None),
            left_git: RefCell::new(None),
            right_git: RefCell::new(None),
            left_ref: RefCell::new(None),
            right_ref: RefCell::new(None),
            result: RefCell::new(None),
            phase: Cell::new(Phase::Idle),
            cancel: RefCell::new(None),
            snapshots: RefCell::new(Vec::new()),
            effective_left: RefCell::new(None),
            effective_right: RefCell::new(None),
            watcher: RefCell::new(None),
            auto_runs: Cell::new(0),
            watch_paused: Cell::new(false),
            watch_pending: Cell::new(false),
            events: RefCell::new(Some(events)),
        })
    }

    /// メインウィンドウ(拡張側がダイアログの親にする)。ウィンドウ構築後に有効。
    pub fn main_window(&self) -> Option<gtk4::ApplicationWindow> {
        self.main_window.borrow().as_ref().and_then(|w| w.upgrade())
    }

    pub(crate) fn set_main_window(&self, w: &gtk4::ApplicationWindow) {
        *self.main_window.borrow_mut() = Some(gtk4::prelude::ObjectExt::downgrade(w));
    }

    fn emit(&self, ev: ModelEvent) {
        if let Some(tx) = self.events.borrow().as_ref() {
            let _ = tx.send(ev);
        }
    }

    /// 設定 + エディション上限から比較オプションを組み立てる。
    /// プリセットの除外パターンはユーザー除外の後ろに畳み込む(Swift `makeOptions` と同じ)。
    pub fn make_options(&self) -> CompareOptions {
        let s = self.settings.borrow();
        let mut excludes = normalize_patterns(&s.user_exclude_patterns);
        for preset_id in &s.filter_presets {
            if let Some(p) = diffloupe_core::filter_preset(preset_id) {
                excludes.extend(p.exclude_patterns.iter().map(|p| p.to_string()));
            }
        }
        CompareOptions {
            recursive: s.recursive,
            use_default_excludes: s.use_default_excludes,
            max_entries: self.edition.max_folder_entries(),
            user_exclude_patterns: excludes,
            user_include_patterns: normalize_patterns(&s.user_include_patterns),
            content_comparison: s.content,
            detect_renames: s.detect_renames,
            ..CompareOptions::default()
        }
    }

    /// フォルダ選択/ドロップ時の適用。git リポジトリ検出と ref 初期化を行う。
    /// `confirm_discard_unsaved_changes` が false なら拒否。
    pub fn set_folder(self: &Rc<Self>, left: bool, path: PathBuf) {
        if !self.edition.confirm_discard_unsaved_changes() {
            return;
        }
        let git = GitRepository::detect(&path).map(Rc::new);
        if left {
            *self.left_folder.borrow_mut() = Some(path);
            *self.left_git.borrow_mut() = git;
            *self.left_ref.borrow_mut() = None;
        } else {
            *self.right_folder.borrow_mut() = Some(path);
            *self.right_git.borrow_mut() = git;
            *self.right_ref.borrow_mut() = None;
        }
    }

    /// 左右交換(⌘⇧S 相当)。比較実行中は no-op。
    /// 結果・選択・スナップショットを破棄する。
    pub fn swap_sides(self: &Rc<Self>) -> bool {
        if self.phase.get() == Phase::Running {
            return false;
        }
        if !self.edition.confirm_discard_unsaved_changes() {
            return false;
        }
        std::mem::swap(
            &mut *self.left_folder.borrow_mut(),
            &mut *self.right_folder.borrow_mut(),
        );
        std::mem::swap(
            &mut *self.left_git.borrow_mut(),
            &mut *self.right_git.borrow_mut(),
        );
        std::mem::swap(
            &mut *self.left_ref.borrow_mut(),
            &mut *self.right_ref.borrow_mut(),
        );
        self.discard_result();
        true
    }

    /// 結果と保持中の git スナップショットを破棄する。
    pub fn discard_result(&self) {
        *self.result.borrow_mut() = None;
        for p in self.snapshots.borrow_mut().drain(..) {
            GitRepository::cleanup(&p);
        }
        *self.effective_left.borrow_mut() = None;
        *self.effective_right.borrow_mut() = None;
    }

    /// 比較開始。`user_initiated` 時のみ未保存編集の確認フックを通す
    /// (Swift `beginCompare` と同じ分岐)。
    /// 成功時はバックグラウンドスレッドが起きる。false = 開始できなかった。
    pub fn begin_compare(self: &Rc<Self>, user_initiated: bool) -> bool {
        if self.phase.get() == Phase::Running {
            return false;
        }
        if user_initiated && !self.edition.confirm_discard_unsaved_changes() {
            return false;
        }
        let (Some(left), Some(right)) = (
            self.left_folder.borrow().clone(),
            self.right_folder.borrow().clone(),
        ) else {
            return false;
        };

        // git ref が選ばれている側はスナップショットを実体化して比較ルートにする
        self.discard_result();
        let mut eff_left = left;
        let mut eff_right = right;
        for (is_left, path) in [(true, eff_left.clone()), (false, eff_right.clone())] {
            let (git, git_ref) = if is_left {
                (
                    self.left_git.borrow().clone(),
                    self.left_ref.borrow().clone(),
                )
            } else {
                (
                    self.right_git.borrow().clone(),
                    self.right_ref.borrow().clone(),
                )
            };
            if let (Some(repo), Some(r)) = (git, git_ref) {
                match repo.materialize_snapshot(&r) {
                    Ok(snapshot) => {
                        if is_left {
                            eff_left = snapshot.clone();
                        } else {
                            eff_right = snapshot.clone();
                        }
                        self.snapshots.borrow_mut().push(snapshot);
                    }
                    Err(e) => {
                        self.emit(ModelEvent::Finished(Err(CompareError::Io(e.to_string()))));
                        for p in self.snapshots.borrow_mut().drain(..) {
                            GitRepository::cleanup(&p);
                        }
                        return false;
                    }
                }
            }
            let _ = path;
        }
        *self.effective_left.borrow_mut() = Some(eff_left.clone());
        *self.effective_right.borrow_mut() = Some(eff_right.clone());

        let token = Arc::new(CancellationToken::new());
        *self.cancel.borrow_mut() = Some(token.clone());
        self.phase.set(Phase::Running);
        self.watch_pending.set(false);

        let options = self.make_options();
        let tx = self.events.borrow().clone().expect("events sender");
        let progress_tx = tx.clone();
        let token_t = token.clone();
        std::thread::spawn(move || {
            let progress: Box<dyn Fn(ComparisonProgress) + Send + Sync> = Box::new(move |p| {
                let _ = progress_tx.send(ModelEvent::Progress(p));
            });
            let res = FolderComparator::compare(
                &eff_left,
                &eff_right,
                &options,
                Some(&*progress),
                Some(&token_t),
            );
            let _ = tx.send(ModelEvent::Finished(res));
        });
        true
    }

    /// 完了通知をモデル側へ反映(ウィンドウが UI 更新後に呼ぶ)。
    /// 失敗・キャンセル時はスナップショットを即時削除する。
    pub fn on_finished(self: &Rc<Self>, ok: bool) {
        self.phase.set(Phase::Done);
        *self.cancel.borrow_mut() = None;
        if !ok {
            for p in self.snapshots.borrow_mut().drain(..) {
                GitRepository::cleanup(&p);
            }
            *self.effective_left.borrow_mut() = None;
            *self.effective_right.borrow_mut() = None;
        }
        // 比較中に検知した変更をまとめて1回だけ再比較
        if ok && self.watch_pending.replace(false) {
            self.try_auto_recompare();
        }
        self.configure_watcher();
    }

    pub fn cancel_compare(&self) {
        if let Some(t) = self.cancel.borrow().as_ref() {
            t.cancel();
        }
    }

    /// 自動再比較(監視スレッド発火 → メインスレッドで呼ぶ)。
    pub fn on_watch_changed(self: &Rc<Self>) {
        if self.phase.get() == Phase::Running {
            self.watch_pending.set(true);
            return;
        }
        self.try_auto_recompare();
    }

    fn try_auto_recompare(self: &Rc<Self>) {
        if self.watch_paused.get() {
            return;
        }
        // Pro の編集保持: 未保存があれば延期する(Free は常に false)
        if self.edition.has_unsaved_changes() {
            self.watch_pending.set(true);
            return;
        }
        if self.auto_runs.get() >= MAX_AUTO_REFRESH_RUNS {
            self.watch_paused.set(true);
            return;
        }
        self.auto_runs.set(self.auto_runs.get() + 1);
        self.begin_compare(false);
    }

    /// 手動比較や設定変更で自動再比較カウンタを復帰する。
    pub fn reset_auto_runs(&self) {
        self.auto_runs.set(0);
        self.watch_paused.set(false);
        self.watch_pending.set(false);
    }

    /// 監視対象を再構成する。git ref 選択側(スナップショット比較)は監視しない。
    /// 両側 ref なら監視しない。設定で有効かつ比較結果がある場合のみ監視する。
    pub fn configure_watcher(self: &Rc<Self>) {
        *self.watcher.borrow_mut() = None; // 旧監視を stop
        if !self.settings.borrow().auto_refresh || self.result.borrow().is_none() {
            return;
        }
        let mut roots = Vec::new();
        if self.left_ref.borrow().is_none() {
            if let Some(p) = self.left_folder.borrow().clone() {
                roots.push(p);
            }
        }
        if self.right_ref.borrow().is_none() {
            if let Some(p) = self.right_folder.borrow().clone() {
                roots.push(p);
            }
        }
        if roots.is_empty() {
            return;
        }
        let excluded = if self.settings.borrow().use_default_excludes {
            vec![
                ".git".to_string(),
                ".DS_Store".to_string(),
                "node_modules".to_string(),
            ]
        } else {
            Vec::new()
        };
        let tx = self.events.borrow().clone().expect("events sender");
        let w = Rc::new(DirectoryWatcher::new(
            roots,
            WATCH_INTERVAL,
            excluded,
            move || {
                let _ = tx.send(ModelEvent::WatchChanged);
            },
        ));
        w.start();
        *self.watcher.borrow_mut() = Some(w);
    }

    /// 状態フィルタ + 相対パス検索で表示エントリを絞る。
    /// `ComparisonResult` 自体は変更しない(レポート等の対象件数に影響しない)。
    /// 一致する子孫を表示するため祖先ディレクトリは残す。
    pub fn filtered_entries(
        result: &ComparisonResult,
        filter: StatusFilter,
        query: &str,
    ) -> Vec<diffloupe_core::ComparedEntry> {
        let q = query.trim().to_lowercase();
        let matches = |e: &diffloupe_core::ComparedEntry| -> bool {
            let status_ok = match filter {
                StatusFilter::All => true,
                StatusFilter::AllDiffs => e.status != DiffStatus::Identical,
                StatusFilter::Different => e.status == DiffStatus::Different,
                StatusFilter::LeftOnly => e.status == DiffStatus::LeftOnly,
                StatusFilter::RightOnly => e.status == DiffStatus::RightOnly,
                StatusFilter::Error => e.status == DiffStatus::Error,
                StatusFilter::Identical => e.status == DiffStatus::Identical,
            };
            status_ok && (q.is_empty() || e.relative_path.to_lowercase().contains(&q))
        };
        // 第一パス: 直接一致した相対パス集合
        let hit: std::collections::HashSet<&str> = result
            .entries
            .iter()
            .filter(|e| matches(e))
            .map(|e| e.relative_path.as_str())
            .collect();
        result
            .entries
            .iter()
            .filter(|e| {
                if hit.contains(e.relative_path.as_str()) {
                    return true;
                }
                // ディレクトリは一致する子孫を表示する祖先として残す
                e.is_directory
                    && hit
                        .iter()
                        .any(|p| p.starts_with(&format!("{}/", e.relative_path)))
            })
            .cloned()
            .collect()
    }
}

/// 状態フィルタ(Swift `ResultFilter`)。display_name は strings 側。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    AllDiffs,
    Different,
    LeftOnly,
    RightOnly,
    Error,
    Identical,
}

impl StatusFilter {
    pub const ALL: &'static [StatusFilter] = &[
        StatusFilter::All,
        StatusFilter::AllDiffs,
        StatusFilter::Different,
        StatusFilter::LeftOnly,
        StatusFilter::RightOnly,
        StatusFilter::Error,
        StatusFilter::Identical,
    ];

    pub fn display_name(&self) -> &'static str {
        use crate::strings as s;
        match self {
            StatusFilter::All => s::FILTER_ALL,
            StatusFilter::AllDiffs => s::FILTER_ALL_DIFFS,
            StatusFilter::Different => s::FILTER_DIFFERENT,
            StatusFilter::LeftOnly => s::FILTER_LEFT_ONLY,
            StatusFilter::RightOnly => s::FILTER_RIGHT_ONLY,
            StatusFilter::Error => s::FILTER_ERROR,
            StatusFilter::Identical => s::FILTER_IDENTICAL,
        }
    }
}

/// ステータスのバッジ表示名。ディレクトリの different は「差分あり」。
pub fn status_badge(status: DiffStatus, is_directory: bool) -> &'static str {
    use crate::strings as s;
    match status {
        DiffStatus::Identical => s::FILTER_IDENTICAL,
        DiffStatus::Different if is_directory => s::STATUS_HAS_DIFFS,
        DiffStatus::Different => s::FILTER_DIFFERENT,
        DiffStatus::LeftOnly => s::FILTER_LEFT_ONLY,
        DiffStatus::RightOnly => s::FILTER_RIGHT_ONLY,
        DiffStatus::Error => s::FILTER_ERROR,
    }
}
