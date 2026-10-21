//! 設定の永続化。Swift 版は UserDefaults、Linux では XDG 構成ディレクトリの
//! INI ファイル(GLib KeyFile)に保存する(OS ネイティブな場所)。
//!
//! 保存先: `$XDG_CONFIG_HOME/<config_dir_name>/settings.ini`(Free は diffloupe)
//! キー名は Swift 版 AppModel の UserDefaults キーと揃える。

use std::path::PathBuf;

use diffloupe_core::{ContentComparison, StructuredComparisonMode};

#[derive(Debug, Clone)]
pub struct Settings {
    pub recursive: bool,
    pub use_default_excludes: bool,
    pub user_exclude_patterns: String, // 改行区切り glob
    pub user_include_patterns: String,
    pub filter_presets: Vec<String>, // プリセット id
    pub content: ContentComparison,
    pub detect_renames: bool,
    pub auto_refresh: bool,
    // ウィンドウ位置・サイズ(GTK4 では位置取得は X11 のみ有効。Wayland はサイズのみ)
    pub window_width: i32,
    pub window_height: i32,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_maximized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recursive: true,
            use_default_excludes: true,
            user_exclude_patterns: String::new(),
            user_include_patterns: String::new(),
            filter_presets: Vec::new(),
            content: ContentComparison::default(),
            detect_renames: false,
            auto_refresh: false,
            window_width: 1200,
            window_height: 760,
            window_x: None,
            window_y: None,
            window_maximized: false,
        }
    }
}

/// 設定ディレクトリ名(`AppConfig::config_dir_name`)。未設定なら Free の "diffloupe"。
/// Pro は別名にして、Free と設定・履歴が混ざらないようにする。
static CONFIG_DIR_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// 設定ディレクトリ名を設定する(`run` が起動時に 1 回呼ぶ。2 回目以降は無視)。
pub fn set_config_dir_name(name: &str) {
    let _ = CONFIG_DIR_NAME.set(name.to_string());
}

/// 設定・履歴の保存先。Pro が追加の設定ファイルを置く場所にも使う。
pub fn config_dir() -> PathBuf {
    gtk4::glib::user_config_dir().join(
        CONFIG_DIR_NAME
            .get()
            .map(String::as_str)
            .unwrap_or("diffloupe"),
    )
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.ini")
}

/// 改行区切りパターンの正規化(Swift `normalizePatterns` と同規則):
/// 各行を trim、空行除去、順序保持で重複除去。
pub fn normalize_patterns(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.to_string()))
        .map(|l| l.to_string())
        .collect()
}

pub fn parse_structured_mode(raw: &str) -> StructuredComparisonMode {
    match raw {
        "automatic" => StructuredComparisonMode::Automatic,
        "json" => StructuredComparisonMode::Json,
        "xml" => StructuredComparisonMode::Xml,
        "csv" => StructuredComparisonMode::Csv,
        _ => StructuredComparisonMode::Disabled,
    }
}

pub fn structured_mode_raw(mode: StructuredComparisonMode) -> &'static str {
    mode.raw_value()
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let kf = gtk4::glib::KeyFile::new();
        if kf
            .load_from_file(settings_path(), gtk4::glib::KeyFileFlags::NONE)
            .is_err()
        {
            return s;
        }
        let get_bool = |key: &str, cur: bool| kf.boolean("diff", key).unwrap_or(cur);
        let get_string =
            |key: &str, cur: String| kf.string("diff", key).map(|v| v.to_string()).unwrap_or(cur);
        s.recursive = get_bool("recursiveComparison", s.recursive);
        s.use_default_excludes = get_bool("useDefaultExcludes", s.use_default_excludes);
        s.user_exclude_patterns = get_string("userExcludePatterns", s.user_exclude_patterns);
        s.user_include_patterns = get_string("userIncludePatterns", s.user_include_patterns);
        let presets = get_string("filterPresets", String::new());
        s.filter_presets = presets
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        s.content.ignore_line_endings =
            get_bool("contentIgnoreLineEndings", s.content.ignore_line_endings);
        s.content.ignore_trailing_whitespace = get_bool(
            "contentIgnoreTrailingWhitespace",
            s.content.ignore_trailing_whitespace,
        );
        s.content.normalize_whitespace =
            get_bool("contentNormalizeWhitespace", s.content.normalize_whitespace);
        s.content.ignore_case = get_bool("contentIgnoreCase", s.content.ignore_case);
        s.content.structured_mode =
            parse_structured_mode(&get_string("contentStructuredMode", "disabled".into()));
        s.detect_renames = get_bool("detectRenames", s.detect_renames);
        s.auto_refresh = get_bool("autoRefreshEnabled", s.auto_refresh);
        s.window_width = kf.integer("window", "width").unwrap_or(1200).max(400);
        s.window_height = kf.integer("window", "height").unwrap_or(760).max(300);
        s.window_x = kf.integer("window", "x").ok();
        s.window_y = kf.integer("window", "y").ok();
        s.window_maximized = kf.boolean("window", "maximized").unwrap_or(false);
        s
    }

    pub fn save(&self) {
        let kf = gtk4::glib::KeyFile::new();
        kf.set_boolean("diff", "recursiveComparison", self.recursive);
        kf.set_boolean("diff", "useDefaultExcludes", self.use_default_excludes);
        kf.set_string("diff", "userExcludePatterns", &self.user_exclude_patterns);
        kf.set_string("diff", "userIncludePatterns", &self.user_include_patterns);
        kf.set_string("diff", "filterPresets", &self.filter_presets.join(","));
        kf.set_boolean(
            "diff",
            "contentIgnoreLineEndings",
            self.content.ignore_line_endings,
        );
        kf.set_boolean(
            "diff",
            "contentIgnoreTrailingWhitespace",
            self.content.ignore_trailing_whitespace,
        );
        kf.set_boolean(
            "diff",
            "contentNormalizeWhitespace",
            self.content.normalize_whitespace,
        );
        kf.set_boolean("diff", "contentIgnoreCase", self.content.ignore_case);
        kf.set_string(
            "diff",
            "contentStructuredMode",
            structured_mode_raw(self.content.structured_mode),
        );
        kf.set_boolean("diff", "detectRenames", self.detect_renames);
        kf.set_boolean("diff", "autoRefreshEnabled", self.auto_refresh);
        kf.set_integer("window", "width", self.window_width);
        kf.set_integer("window", "height", self.window_height);
        if let (Some(x), Some(y)) = (self.window_x, self.window_y) {
            kf.set_integer("window", "x", x);
            kf.set_integer("window", "y", y);
        }
        kf.set_boolean("window", "maximized", self.window_maximized);
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = kf.save_to_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_patterns_matches_swift_rules() {
        assert_eq!(
            normalize_patterns("  *.tmp \n\n*.log\n*.tmp\n  "),
            vec!["*.tmp".to_string(), "*.log".to_string()]
        );
        assert!(normalize_patterns("   \n\t\n").is_empty());
    }

    #[test]
    fn structured_mode_roundtrip() {
        for raw in ["disabled", "automatic", "json", "xml", "csv"] {
            assert_eq!(structured_mode_raw(parse_structured_mode(raw)), raw);
        }
        assert_eq!(
            structured_mode_raw(parse_structured_mode("bogus")),
            "disabled"
        );
    }
}
