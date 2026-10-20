//! アプリ全体の状態管理(Swift `AppModel` 相当)。Win32 非依存。
//! UI 層は `plan_compare()` で比較計画を取り出し、ワーカースレッドで実行後
//! `finish_compare()` に結果を戻す。進捗・完了通知は UI 層のメッセージ経路。

use diffloupe_core::cancel::EntryLimitExceeded;
use diffloupe_core::comparator::{CompareOptions, FolderComparator, ProgressHandler};
use diffloupe_core::git_snapshot::{GitError, GitRepository};
use diffloupe_core::models::{ComparedEntry, ComparisonProgress, ComparisonResult};
use diffloupe_core::report::{ComparisonReport, ReportFormat};
use diffloupe_core::sync_plan::{SyncDirection, SyncPlan};
use diffloupe_core::watcher::DirectoryWatcher;
use diffloupe_core::CancellationToken;
use diffloupe_edition::Edition;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::recent::{self, RecentComparison};
use crate::settings::{self, AppSettings};
use crate::viewmodel::{self, ResultFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Scanning { found: usize },
    Comparing { done: usize, total: usize },
    Renaming { done: usize, total: usize },
}

impl Phase {
    pub fn is_running(&self) -> bool {
        *self != Phase::Idle
    }
}

/// 比較対象の実体(作業ツリー or git ref のスナップショット)
#[derive(Debug, Clone)]
pub enum CompareSource {
    Folder(PathBuf),
    GitRef {
        repo: GitRepository,
        git_ref: String,
    },
}

/// `plan_compare` が返す、ワーカー側で実行すべき入力一式。
pub struct ComparePlan {
    pub options: CompareOptions,
    pub left: CompareSource,
    pub right: CompareSource,
    /// 完了後に復元する選択パス(recompare(preservingSelection:) 相当)
    pub restoring_selection: Option<String>,
}

/// ワーカー側の比較実行結果。
pub struct CompareOutcome {
    pub result: Result<ComparisonResult, CompareFailure>,
    /// 結果が参照し続ける一時スナップショット(成功時に保持対象へ移す)
    pub snapshots: Vec<PathBuf>,
    /// restoring_selection そのまま返す
    pub restoring_selection: Option<String>,
}

#[derive(Debug)]
pub enum CompareFailure {
    Cancelled,
    EntryLimit(EntryLimitExceeded),
    Git(GitError),
    Io(String),
}

/// 自動再比較判定の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRefreshEffect {
    /// 何もしない
    None,
    /// 即時再比較
    Recompare,
    /// 比較中のため保留(完了後に一度だけ再比較)
    Pending,
}

/// エントリ上限アラートの内容
#[derive(Debug, Clone)]
pub struct EntryLimitAlert {
    pub message: String,
}

pub struct AppModel {
    pub edition: Arc<dyn Edition>,
    pub settings: AppSettings,
    /// 設定ファイルのパス(UI 層が解決して渡す)
    pub settings_path: PathBuf,

    pub left_folder: Option<PathBuf>,
    pub right_folder: Option<PathBuf>,
    pub phase: Phase,
    pub result: Option<ComparisonResult>,
    pub error_message: Option<String>,
    pub selection: Option<String>,
    pub result_filter: ResultFilter,
    pub result_search_text: String,
    pub sync_plan: Option<SyncPlan>,
    pub watcher_active: bool,
    pub auto_refresh_paused_reason: Option<String>,
    pub entry_limit_alert: Option<EntryLimitAlert>,

    pub left_git_repo: Option<GitRepository>,
    pub right_git_repo: Option<GitRepository>,
    pub left_git_ref: Option<String>,
    pub right_git_ref: Option<String>,

    /// 表示中の結果が参照している git スナップショット一時ディレクトリ
    pub active_snapshots: Vec<PathBuf>,

    // 自動再比較の内部状態
    pub auto_refresh_pending: bool,
    pub auto_refresh_deferred: bool,
    pub auto_refresh_run_count: usize,

    // 3-way ビューの選択ファイル
    pub three_way_base: Option<PathBuf>,
    pub three_way_left: Option<PathBuf>,
    pub three_way_right: Option<PathBuf>,
}

impl AppModel {
    pub fn new(edition: Arc<dyn Edition>, settings_path: PathBuf) -> Self {
        Self {
            edition,
            settings: settings::load(&settings_path),
            settings_path,
            left_folder: None,
            right_folder: None,
            phase: Phase::Idle,
            result: None,
            error_message: None,
            selection: None,
            result_filter: ResultFilter::All,
            result_search_text: String::new(),
            sync_plan: None,
            watcher_active: false,
            auto_refresh_paused_reason: None,
            entry_limit_alert: None,
            left_git_repo: None,
            right_git_repo: None,
            left_git_ref: None,
            right_git_ref: None,
            active_snapshots: Vec::new(),
            auto_refresh_pending: false,
            auto_refresh_deferred: false,
            auto_refresh_run_count: 0,
            three_way_base: None,
            three_way_left: None,
            three_way_right: None,
        }
    }

    pub fn can_compare(&self) -> bool {
        self.left_folder.is_some() && self.right_folder.is_some() && !self.phase.is_running()
    }

    pub fn save_settings(&self) {
        let _ = settings::save(&self.settings_path, &self.settings);
    }

    pub fn folder(&self, side: Side) -> Option<&PathBuf> {
        match side {
            Side::Left => self.left_folder.as_ref(),
            Side::Right => self.right_folder.as_ref(),
        }
    }

    pub fn git_repo(&self, side: Side) -> Option<&GitRepository> {
        match side {
            Side::Left => self.left_git_repo.as_ref(),
            Side::Right => self.right_git_repo.as_ref(),
        }
    }

    pub fn git_ref(&self, side: Side) -> Option<&String> {
        match side {
            Side::Left => self.left_git_ref.as_ref(),
            Side::Right => self.right_git_ref.as_ref(),
        }
    }

    /// フォルダ適用(NSOpenPanel/ドロップで共用。git 検出と ref 初期化を行う)
    /// 戻り値: 適用できたか(未保存編集の確認で中止された場合 false)
    pub fn apply_folder(&mut self, path: PathBuf, side: Side) -> bool {
        if !self.edition.confirm_discard_unsaved_changes() {
            return false;
        }
        let detected = GitRepository::detect(&path);
        match side {
            Side::Left => {
                self.left_folder = Some(path);
                self.left_git_repo = detected;
                self.left_git_ref = None;
            }
            Side::Right => {
                self.right_folder = Some(path);
                self.right_git_repo = detected;
                self.right_git_ref = None;
            }
        }
        true
    }

    /// ドロップ受付。フォルダ以外・実行中は拒否(シンボリックリンクは解決)。
    pub fn accept_dropped_folder(&mut self, path: &Path, side: Side) -> bool {
        if self.phase.is_running() {
            return false;
        }
        if !path.is_dir() {
            return false;
        }
        let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.apply_folder(resolved, side)
    }

    /// 左右交換。実行中は no-op。結果・選択・エラーを破棄しスナップショットを掃除する。
    pub fn swap_sides(&mut self) -> bool {
        if self.phase.is_running() {
            return false;
        }
        if !self.edition.confirm_discard_unsaved_changes() {
            return false;
        }
        std::mem::swap(&mut self.left_folder, &mut self.right_folder);
        std::mem::swap(&mut self.left_git_repo, &mut self.right_git_repo);
        std::mem::swap(&mut self.left_git_ref, &mut self.right_git_ref);
        self.clear_result();
        true
    }

    fn clear_result(&mut self) {
        for p in self.active_snapshots.drain(..) {
            GitRepository::cleanup(&p);
        }
        self.result = None;
        self.selection = None;
        self.error_message = None;
        self.sync_plan = None;
    }

    /// 「フォルダ名 @ ref」形式の表示ラベル
    pub fn display_label(&self, side: Side) -> Option<String> {
        let url = self.folder(side)?;
        let name = url
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| url.to_string_lossy().into_owned());
        match self.git_ref(side) {
            Some(r) => Some(format!("{} @ {}", name, r)),
            None => Some(name),
        }
    }

    /// ref 選択。None で「作業ツリー」へ戻す。
    pub fn select_git_ref(&mut self, side: Side, git_ref: Option<String>) {
        match side {
            Side::Left => self.left_git_ref = git_ref,
            Side::Right => self.right_git_ref = git_ref,
        }
    }

    pub fn selected_entry(&self) -> Option<&ComparedEntry> {
        let sel = self.selection.as_deref()?;
        self.result.as_ref()?.entry_by_path(sel)
    }

    /// 履歴復元。存在しないディレクトリ・実行中は何もしない。
    pub fn restore_comparison(&mut self, item: &RecentComparison) -> bool {
        if self.phase.is_running() {
            return false;
        }
        let l = Path::new(&item.left_path);
        let r = Path::new(&item.right_path);
        if !l.is_dir() || !r.is_dir() {
            return false;
        }
        if !self.edition.confirm_discard_unsaved_changes() {
            return false;
        }
        self.clear_result();
        self.left_folder = Some(l.to_path_buf());
        self.right_folder = Some(r.to_path_buf());
        self.left_git_repo = GitRepository::detect(l);
        self.right_git_repo = GitRepository::detect(r);
        self.left_git_ref = item.left_git_ref.clone();
        self.right_git_ref = item.right_git_ref.clone();
        true
    }

    /// 履歴項目が復元可能か(両側が実在するディレクトリ)
    pub fn history_item_exists(item: &RecentComparison) -> bool {
        Path::new(&item.left_path).is_dir() && Path::new(&item.right_path).is_dir()
    }

    pub fn display_nodes(&self) -> Vec<diffloupe_core::models::DiffNode> {
        viewmodel::display_nodes(
            self.result.as_ref(),
            self.result_filter,
            &self.result_search_text,
        )
    }

    pub fn prepare_sync_plan(&mut self, direction: SyncDirection) {
        if let Some(result) = &self.result {
            self.sync_plan = Some(SyncPlan::make(result, direction));
        }
    }

    /// レポート本文を生成(保存ダイアログと上書き確認は UI 層)
    pub fn render_report(&self, format: ReportFormat, generated_at: i64) -> Option<String> {
        let result = self.result.as_ref()?;
        let mut report = ComparisonReport::new(result, generated_at);
        report.left_git_ref = self.left_git_ref.clone();
        report.right_git_ref = self.right_git_ref.clone();
        Some(report.render(format))
    }

    /// 既定のレポートファイル名(UTC・ロケール非依存)
    /// 例: DiffLoupe-report-20260812-013000.html
    pub fn default_report_file_name(format: ReportFormat) -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let iso = diffloupe_core::report::iso8601(secs); // YYYY-MM-DDTHH:MM:SSZ
        let stamp: String = iso
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(14)
            .collect();
        let stamp = format!("{}-{}", &stamp[..8], &stamp[8..]);
        format!("DiffLoupe-report-{}.{}", stamp, format.file_extension())
    }

    /// 比較開始(手動)。履歴記録・スナップショット掃除・options 生成まで行い、
    /// ワーカーが実行すべき計画を返す。開始できない場合は None。
    pub fn plan_compare(&mut self, user_initiated: bool) -> Option<ComparePlan> {
        self.plan_compare_impl(user_initiated, None)
    }

    /// 選択復元付きの再比較(「recompare(preservingSelection:)」相当)
    pub fn plan_recompare_preserving_selection(&mut self) -> Option<ComparePlan> {
        self.plan_compare_impl(true, self.selection.clone())
    }

    fn plan_compare_impl(
        &mut self,
        user_initiated: bool,
        restoring_selection: Option<String>,
    ) -> Option<ComparePlan> {
        let (Some(left), Some(right)) = (self.left_folder.clone(), self.right_folder.clone())
        else {
            return None;
        };
        if self.phase.is_running() {
            return None;
        }
        if user_initiated && !self.edition.confirm_discard_unsaved_changes() {
            return None;
        }
        if user_initiated {
            self.auto_refresh_run_count = 0;
            self.auto_refresh_paused_reason = None;
        }
        let options = self
            .settings
            .make_options(self.edition.max_folder_entries());
        self.phase = Phase::Scanning { found: 0 };
        self.clear_result();
        self.entry_limit_alert = None;

        // 比較開始時に履歴へ記録(同一ペアは先頭へ移動)
        self.settings.recent_comparisons = recent::record(
            &self.settings.recent_comparisons,
            RecentComparison {
                left_path: left.to_string_lossy().into_owned(),
                right_path: right.to_string_lossy().into_owned(),
                left_git_ref: self.left_git_ref.clone(),
                right_git_ref: self.right_git_ref.clone(),
                date: recent::now_iso8601(),
            },
        );
        self.save_settings();

        let left_src = match (&self.left_git_repo, &self.left_git_ref) {
            (Some(repo), Some(r)) => CompareSource::GitRef {
                repo: repo.clone(),
                git_ref: r.clone(),
            },
            _ => CompareSource::Folder(left),
        };
        let right_src = match (&self.right_git_repo, &self.right_git_ref) {
            (Some(repo), Some(r)) => CompareSource::GitRef {
                repo: repo.clone(),
                git_ref: r.clone(),
            },
            _ => CompareSource::Folder(right),
        };
        Some(ComparePlan {
            options,
            left: left_src,
            right: right_src,
            restoring_selection,
        })
    }

    /// 比較完了の後処理(Swift `finish` 相当)。
    /// 戻り値: 比較中に変更検知があり、直ちに自動再比較すべきなら true。
    pub fn finish_compare(&mut self, outcome: CompareOutcome) -> bool {
        match outcome.result {
            Ok(result) => {
                self.active_snapshots = outcome.snapshots;
                self.result = Some(result);
                if let Some(path) = &outcome.restoring_selection {
                    if self
                        .result
                        .as_ref()
                        .map(|r| r.entry_by_path(path).is_some())
                        .unwrap_or(false)
                    {
                        self.selection = Some(path.clone());
                    }
                }
                self.error_message = None;
            }
            Err(CompareFailure::EntryLimit(e)) => {
                for p in &outcome.snapshots {
                    GitRepository::cleanup(p);
                }
                self.error_message = Some(e.to_string());
                self.entry_limit_alert = Some(EntryLimitAlert {
                    message: e.to_string(),
                });
            }
            Err(CompareFailure::Git(e)) => {
                for p in &outcome.snapshots {
                    GitRepository::cleanup(p);
                }
                self.error_message = Some(e.to_string());
            }
            Err(CompareFailure::Io(e)) => {
                for p in &outcome.snapshots {
                    GitRepository::cleanup(p);
                }
                self.error_message = Some(format!("比較に失敗しました: {}", e));
            }
            Err(CompareFailure::Cancelled) => {
                for p in &outcome.snapshots {
                    GitRepository::cleanup(p);
                }
            }
        }
        self.phase = Phase::Idle;
        // 比較中に検知した変更は成功時のみ一度だけ再比較
        let pending = std::mem::take(&mut self.auto_refresh_pending);
        pending && self.result.is_some()
    }

    /// 自動再比較の連続上限超過による一時停止。
    /// 戻り値は表示用の理由文字列。
    pub fn pause_auto_refresh_reason(&self) -> Option<String> {
        if settings::should_pause_auto_refresh(self.auto_refresh_run_count) {
            Some(format!(
                "自動再比較が{}回連続したため一時停止しました。設定のトグルを再度オンにするか、手動で比較してください。",
                settings::MAX_AUTO_REFRESH_RUNS
            ))
        } else {
            None
        }
    }

    /// 監視対象のルート(git ref 選択側はスナップショット比較なので監視しない)
    pub fn watcher_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if self.left_git_ref.is_none() {
            if let Some(l) = &self.left_folder {
                roots.push(l.clone());
            }
        }
        if self.right_git_ref.is_none() {
            if let Some(r) = &self.right_folder {
                roots.push(r.clone());
            }
        }
        roots
    }

    /// watcher を構成すべきか(Swift `reconfigureWatcher` の判定部分)
    pub fn should_watch(&self) -> bool {
        self.settings.auto_refresh_enabled
            && self.auto_refresh_paused_reason.is_none()
            && !self.phase.is_running()
            && !self.watcher_roots().is_empty()
    }

    /// watcher の既定除外名(Swift は useDefaultExcludes の設定値を使う)
    pub fn watcher_excluded_names(&self) -> Vec<String> {
        if self.settings.use_default_excludes {
            vec![".git".into(), ".DS_Store".into(), "node_modules".into()]
        } else {
            Vec::new()
        }
    }

    /// watcher からの変更通知。比較中は保留、待機中は再比較判定。
    pub fn handle_watcher_change(&mut self, is_running: bool) -> AutoRefreshEffect {
        if !self.settings.auto_refresh_enabled {
            return AutoRefreshEffect::None;
        }
        if is_running {
            self.auto_refresh_pending = true;
            return AutoRefreshEffect::Pending;
        }
        self.auto_recompare()
    }

    /// watcher 由来の再比較判定(Swift `autoRecompare` 相当)。
    /// Recompare を返したら UI 層は plan_compare(false) を呼ぶ。
    pub fn auto_recompare(&mut self) -> AutoRefreshEffect {
        if !self.settings.auto_refresh_enabled
            || self.auto_refresh_paused_reason.is_some()
            || self.left_folder.is_none()
            || self.right_folder.is_none()
            || self.phase.is_running()
        {
            return AutoRefreshEffect::None;
        }
        if self.edition.has_unsaved_changes() {
            self.auto_refresh_deferred = true;
            return AutoRefreshEffect::None;
        }
        self.auto_refresh_deferred = false;
        self.auto_refresh_pending = false;
        self.auto_refresh_run_count += 1;
        if let Some(reason) = self.pause_auto_refresh_reason() {
            self.auto_refresh_paused_reason = Some(reason);
            return AutoRefreshEffect::None;
        }
        AutoRefreshEffect::Recompare
    }

    /// 未保存編集で保留した自動再比較の再開(保存/破棄後にエディション側から呼ぶ想定)
    pub fn resume_deferred_auto_refresh(&mut self) -> AutoRefreshEffect {
        if !self.auto_refresh_deferred {
            return AutoRefreshEffect::None;
        }
        self.auto_recompare()
    }

    /// 比較実行(ワーカースレッドから呼ぶ)。plan の解決 → compare。
    /// `cancel` は外部からの中断用。進捗は `progress` へ逐次報告される。
    pub fn run_compare(
        plan: &ComparePlan,
        progress: &ProgressHandler,
        cancel: Option<&CancellationToken>,
    ) -> CompareOutcome {
        let mut snapshots = Vec::new();
        let resolve = |src: &CompareSource, snapshots: &mut Vec<PathBuf>| match src {
            CompareSource::Folder(p) => Ok(p.clone()),
            CompareSource::GitRef { repo, git_ref } => match repo.materialize_snapshot(git_ref) {
                Ok(dir) => {
                    snapshots.push(dir.clone());
                    Ok(dir)
                }
                Err(e) => Err(e),
            },
        };
        let left = match resolve(&plan.left, &mut snapshots) {
            Ok(p) => p,
            Err(e) => {
                return CompareOutcome {
                    result: Err(CompareFailure::Git(e)),
                    snapshots,
                    restoring_selection: plan.restoring_selection.clone(),
                }
            }
        };
        let right = match resolve(&plan.right, &mut snapshots) {
            Ok(p) => p,
            Err(e) => {
                return CompareOutcome {
                    result: Err(CompareFailure::Git(e)),
                    snapshots,
                    restoring_selection: plan.restoring_selection.clone(),
                }
            }
        };
        match FolderComparator::compare(&left, &right, &plan.options, Some(progress), cancel) {
            Ok(result) => CompareOutcome {
                result: Ok(result),
                snapshots,
                restoring_selection: plan.restoring_selection.clone(),
            },
            Err(e) => {
                let f = match e {
                    diffloupe_core::comparator::CompareError::Cancelled => {
                        CompareFailure::Cancelled
                    }
                    diffloupe_core::comparator::CompareError::EntryLimitExceeded(l) => {
                        CompareFailure::EntryLimit(l)
                    }
                    diffloupe_core::comparator::CompareError::Io(s) => CompareFailure::Io(s),
                };
                CompareOutcome {
                    result: Err(f),
                    snapshots,
                    restoring_selection: plan.restoring_selection.clone(),
                }
            }
        }
    }
}

/// 比較進捗 → Phase への変換
pub fn phase_from_progress(p: ComparisonProgress) -> Phase {
    match p {
        ComparisonProgress::Scanning { found } => Phase::Scanning { found },
        ComparisonProgress::Comparing { done, total } => Phase::Comparing { done, total },
        ComparisonProgress::Renames { done, total } => Phase::Renaming { done, total },
    }
}

/// ウォッチャー構築(on_change は UI 層の通知用クロージャ)
pub fn new_watcher(
    roots: Vec<PathBuf>,
    excluded: Vec<String>,
    on_change: impl Fn() + Send + Sync + 'static,
) -> DirectoryWatcher {
    DirectoryWatcher::new(roots, settings::WATCH_INTERVAL, excluded, on_change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffloupe_edition::FreeEdition;

    fn model() -> AppModel {
        let dir = std::env::temp_dir().join(format!("diffloupe-model-{}", std::process::id()));
        AppModel::new(Arc::new(FreeEdition), dir.join("settings.json"))
    }

    #[test]
    fn apply_folder_detects_and_resets_ref() {
        let mut m = model();
        let dir = std::env::temp_dir();
        assert!(m.apply_folder(dir.clone(), Side::Left));
        assert_eq!(m.left_folder.as_deref(), Some(dir.as_path()));
        assert!(m.left_git_ref.is_none());
    }

    #[test]
    fn swap_sides_swaps_everything_and_clears_result() {
        let mut m = model();
        m.left_folder = Some(PathBuf::from("/l"));
        m.right_folder = Some(PathBuf::from("/r"));
        m.left_git_ref = Some("main".into());
        m.result_search_text = "x".into();
        assert!(m.swap_sides());
        assert_eq!(m.left_folder.as_deref(), Some(Path::new("/r")));
        assert_eq!(m.right_git_ref.as_deref(), Some("main"));
    }

    #[test]
    fn dropped_file_rejected() {
        let mut m = model();
        let f = std::env::temp_dir().join("diffloupe-drop-file.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(!m.accept_dropped_folder(&f, Side::Left));
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn plan_compare_requires_both_folders() {
        let mut m = model();
        assert!(m.plan_compare(true).is_none());
        m.left_folder = Some(std::env::temp_dir());
        assert!(m.plan_compare(true).is_none());
        m.right_folder = Some(std::env::temp_dir());
        assert!(m.plan_compare(true).is_some());
        assert!(m.phase.is_running());
    }

    #[test]
    fn plan_compare_records_history() {
        let mut m = model();
        m.left_folder = Some(std::env::temp_dir());
        m.right_folder = Some(std::env::temp_dir());
        m.plan_compare(true);
        assert_eq!(m.settings.recent_comparisons.len(), 1);
    }

    #[test]
    fn watcher_roots_skip_ref_sides() {
        let mut m = model();
        m.left_folder = Some(PathBuf::from("/l"));
        m.right_folder = Some(PathBuf::from("/r"));
        assert_eq!(m.watcher_roots().len(), 2);
        m.left_git_ref = Some("HEAD".into());
        let roots = m.watcher_roots();
        assert_eq!(roots, vec![PathBuf::from("/r")]);
    }

    #[test]
    fn auto_refresh_pause_after_limit() {
        let mut m = model();
        m.settings.auto_refresh_enabled = true;
        m.left_folder = Some(PathBuf::from("/l"));
        m.right_folder = Some(PathBuf::from("/r"));
        for _ in 0..3 {
            assert_eq!(m.auto_recompare(), AutoRefreshEffect::Recompare);
            m.phase = Phase::Idle; // 即座に完了したと仮定
        }
        // 4回目で一時停止
        assert_eq!(m.auto_recompare(), AutoRefreshEffect::None);
        assert!(m.auto_refresh_paused_reason.is_some());
    }

    #[test]
    fn default_report_file_name_format() {
        let name = AppModel::default_report_file_name(ReportFormat::Json);
        assert!(name.starts_with("DiffLoupe-report-"));
        assert!(name.ends_with(".json"));
    }

    #[test]
    fn restore_rejects_missing_dirs() {
        let mut m = model();
        let item = RecentComparison {
            left_path: "/nonexistent-dl-a".into(),
            right_path: "/nonexistent-dl-b".into(),
            left_git_ref: None,
            right_git_ref: None,
            date: "x".into(),
        };
        assert!(!m.restore_comparison(&item));
        assert!(m.left_folder.is_none());
    }
}
