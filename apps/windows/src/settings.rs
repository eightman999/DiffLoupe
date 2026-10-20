//! アプリ設定の永続化(Swift `AppModel` の UserDefaults 設定キー相当)。
//! Windows では `%APPDATA%\DiffLoupe\settings.json` に JSON で保存する。
//! キー名は SwiftUI 版の UserDefaults キーと合わせる。

use diffloupe_core::comparator::CompareOptions;
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured::StructuredComparisonMode;
use diffloupe_core::{filter_preset, FILTER_PRESETS};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::recent::RecentComparison;

/// 自動再比較の連続実行上限(Swift `AppModel.maxAutoRefreshRuns` 相当)
pub const MAX_AUTO_REFRESH_RUNS: usize = 3;

/// 上限超過で一時停止すべきか(Swift `AppModel.shouldPauseAutoRefresh` 相当)
pub fn should_pause_auto_refresh(after_consecutive_runs: usize) -> bool {
    after_consecutive_runs > MAX_AUTO_REFRESH_RUNS
}

/// ウォッチャーのポーリング間隔(Swift `DirectoryWatcher` 既定の 1.0 秒)
pub const WATCH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// ウィンドウの保存位置(論理座標。DPI は起動時のモニタ基準で再解釈する)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowFrame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    #[serde(rename = "recursiveComparison")]
    pub recursive_comparison: bool,
    #[serde(rename = "useDefaultExcludes")]
    pub use_default_excludes: bool,
    #[serde(rename = "userExcludePatterns")]
    pub user_exclude_patterns: String,
    #[serde(rename = "userIncludePatterns")]
    pub user_include_patterns: String,
    /// 選択済みプリセットID(カンマ区切り)
    #[serde(rename = "filterPresets")]
    pub filter_presets: String,
    #[serde(rename = "contentIgnoreLineEndings")]
    pub content_ignore_line_endings: bool,
    #[serde(rename = "contentIgnoreTrailingWhitespace")]
    pub content_ignore_trailing_whitespace: bool,
    #[serde(rename = "contentNormalizeWhitespace")]
    pub content_normalize_whitespace: bool,
    #[serde(rename = "contentIgnoreCase")]
    pub content_ignore_case: bool,
    #[serde(rename = "contentStructuredMode")]
    pub content_structured_mode: String,
    #[serde(rename = "detectRenames")]
    pub detect_renames: bool,
    #[serde(rename = "autoRefreshEnabled")]
    pub auto_refresh_enabled: bool,
    /// ウィンドウ位置の復元
    #[serde(rename = "windowFrame")]
    pub window_frame: Option<WindowFrame>,
    /// 比較履歴(Swift `recentComparisons` 相当)
    #[serde(rename = "recentComparisons")]
    pub recent_comparisons: Vec<RecentComparison>,
}

impl Default for AppSettings {
    fn default() -> Self {
        // Swift `AppModel.defaultSettings` 相当
        Self {
            recursive_comparison: true,
            use_default_excludes: true,
            user_exclude_patterns: String::new(),
            user_include_patterns: String::new(),
            filter_presets: String::new(),
            content_ignore_line_endings: false,
            content_ignore_trailing_whitespace: false,
            content_normalize_whitespace: false,
            content_ignore_case: false,
            content_structured_mode: StructuredComparisonMode::Disabled.raw_value().to_string(),
            detect_renames: false,
            auto_refresh_enabled: false,
            window_frame: None,
            recent_comparisons: Vec::new(),
        }
    }
}

/// パターン入力(1行1パターン)を正規化する。前後空白除去・空行除去・重複除去・出現順保持。
/// (Swift `AppModel.normalizePatterns` 相当)
pub fn normalize_patterns(raw: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for line in raw.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if seen.insert(line.to_string()) {
            result.push(line.to_string());
        }
    }
    result
}

pub fn normalize_exclude_patterns(raw: &str) -> Vec<String> {
    normalize_patterns(raw)
}

pub fn normalize_include_patterns(raw: &str) -> Vec<String> {
    normalize_patterns(raw)
}

impl AppSettings {
    /// 選択済みプリセットID(カンマ区切り。順序は FILTER_PRESETS 準拠)
    pub fn selected_preset_ids(&self) -> Vec<String> {
        let ids: std::collections::HashSet<String> = self
            .filter_presets
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        FILTER_PRESETS
            .iter()
            .map(|p| p.id.to_string())
            .filter(|id| ids.contains(id))
            .collect()
    }

    /// 選択済みプリセットの除外パターンをユーザー除外へ展開して追加する(opt-in)。
    pub fn effective_exclude_patterns(&self) -> Vec<String> {
        let mut pats = normalize_exclude_patterns(&self.user_exclude_patterns);
        for id in self.selected_preset_ids() {
            if let Some(p) = filter_preset(&id) {
                pats.extend(p.exclude_patterns.iter().map(|s| s.to_string()));
            }
        }
        pats
    }

    pub fn structured_mode(&self) -> StructuredComparisonMode {
        match self.content_structured_mode.as_str() {
            "automatic" => StructuredComparisonMode::Automatic,
            "json" => StructuredComparisonMode::Json,
            "xml" => StructuredComparisonMode::Xml,
            "csv" => StructuredComparisonMode::Csv,
            _ => StructuredComparisonMode::Disabled,
        }
    }

    pub fn content_comparison(&self) -> ContentComparison {
        ContentComparison {
            ignore_line_endings: self.content_ignore_line_endings,
            ignore_trailing_whitespace: self.content_ignore_trailing_whitespace,
            normalize_whitespace: self.content_normalize_whitespace,
            ignore_case: self.content_ignore_case,
            structured_mode: self.structured_mode(),
        }
    }

    /// 設定を CompareOptions へ一括伝播(Swift `AppModel.makeOptions` 相当)
    pub fn make_options(&self, max_entries: Option<usize>) -> CompareOptions {
        CompareOptions {
            recursive: self.recursive_comparison,
            use_default_excludes: self.use_default_excludes,
            max_entries,
            user_exclude_patterns: self.effective_exclude_patterns(),
            user_include_patterns: normalize_include_patterns(&self.user_include_patterns),
            content_comparison: self.content_comparison(),
            detect_renames: self.detect_renames,
            ..CompareOptions::default()
        }
    }

    /// 構造化比較モードの raw_value 一覧(設定コンボボックス用)
    pub fn structured_modes() -> [(StructuredComparisonMode, &'static str); 5] {
        [
            (StructuredComparisonMode::Disabled, "オフ"),
            (StructuredComparisonMode::Automatic, "自動(拡張子で判定)"),
            (StructuredComparisonMode::Json, "JSON"),
            (StructuredComparisonMode::Xml, "XML"),
            (StructuredComparisonMode::Csv, "CSV"),
        ]
    }
}

/// 設定ファイルのフルパスを返す。`base_dir` には `%APPDATA%\DiffLoupe` 相当を渡す。
pub fn settings_path(base_dir: &Path) -> PathBuf {
    base_dir.join("settings.json")
}

/// 設定を読み込む。ファイル不存在・壊れている場合は既定値を返す(安全に無視)。
pub fn load(path: &Path) -> AppSettings {
    let Ok(data) = std::fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// 設定を保存する。親ディレクトリは必要に応じて作成する。
pub fn save(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_swift() {
        let s = AppSettings::default();
        assert!(s.recursive_comparison);
        assert!(s.use_default_excludes);
        assert!(s.user_exclude_patterns.is_empty());
        assert_eq!(s.content_structured_mode, "disabled");
        assert!(!s.detect_renames);
        assert!(!s.auto_refresh_enabled);
    }

    #[test]
    fn normalize_patterns_trims_and_dedupes() {
        let raw = "  *.log \n\n*.tmp\n*.log\n  \n*.bak\r\n*.log  ";
        // \r 付きの行は trim で除去される
        let pats = normalize_patterns(raw);
        assert_eq!(pats, vec!["*.log", "*.tmp", "*.bak"]);
    }

    #[test]
    fn structured_mode_unknown_falls_back_to_disabled() {
        let mut s = AppSettings {
            content_structured_mode: "bogus".into(),
            ..Default::default()
        };
        assert_eq!(s.structured_mode(), StructuredComparisonMode::Disabled);
        s.content_structured_mode = "json".into();
        assert_eq!(s.structured_mode(), StructuredComparisonMode::Json);
    }

    #[test]
    fn presets_expand_to_exclude_patterns() {
        let s = AppSettings {
            filter_presets: "source-control".into(),
            ..Default::default()
        };
        let pats = s.effective_exclude_patterns();
        assert!(pats.contains(&".git".to_string()));
        assert!(pats.contains(&".svn".to_string()));
    }

    #[test]
    fn preset_ids_follow_builtin_order() {
        let s = AppSettings {
            filter_presets: "web-nodejs, source-control".into(),
            ..Default::default()
        };
        assert_eq!(
            s.selected_preset_ids(),
            vec!["source-control", "web-nodejs"]
        );
    }

    #[test]
    fn make_options_combines_user_and_preset_patterns() {
        let s = AppSettings {
            user_exclude_patterns: "*.log".into(),
            filter_presets: "source-control".into(),
            detect_renames: true,
            ..Default::default()
        };
        let opt = s.make_options(Some(2000));
        assert_eq!(opt.max_entries, Some(2000));
        assert!(opt.detect_renames);
        assert!(opt.user_exclude_patterns.contains(&"*.log".to_string()));
        assert!(opt.user_exclude_patterns.contains(&".git".to_string()));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "diffloupe-settings-test-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = settings_path(&dir);
        let s = AppSettings {
            user_exclude_patterns: "a\nb".into(),
            window_frame: Some(WindowFrame {
                x: 10,
                y: 20,
                width: 1200,
                height: 800,
                maximized: false,
            }),
            ..Default::default()
        };
        save(&path, &s).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_broken_file_returns_default() {
        let dir = std::env::temp_dir().join(format!(
            "diffloupe-settings-test-{}-{}",
            std::process::id(),
            "b"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = settings_path(&dir);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path), AppSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pause_after_three_consecutive_runs() {
        assert!(!should_pause_auto_refresh(3));
        assert!(should_pause_auto_refresh(4));
    }
}
