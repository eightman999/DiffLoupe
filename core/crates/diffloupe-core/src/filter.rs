//! フィルタ集合・プリセット(Swift `FilterSet.swift` 相当)

use crate::glob::GlobPattern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKind {
    Include,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FilterRule {
    pub kind: FilterKind,
    pub pattern: String,
}

impl FilterRule {
    pub fn new(kind: FilterKind, pattern: &str) -> Self {
        Self {
            kind,
            pattern: pattern.to_string(),
        }
    }
    pub fn matches(&self, name: &str, relative_path: &str) -> bool {
        GlobPattern::new(&self.pattern).matches(name, relative_path)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilterSet {
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    include_globs: Vec<GlobPattern>,
    exclude_globs: Vec<GlobPattern>,
}

fn sanitized(patterns: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in patterns {
        let t = raw.trim();
        if !t.is_empty() && seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }
    out
}

impl FilterSet {
    pub fn new(include_patterns: Vec<String>, exclude_patterns: Vec<String>) -> Self {
        let inc = sanitized(&include_patterns);
        let exc = sanitized(&exclude_patterns);
        Self {
            include_globs: inc.iter().map(|p| GlobPattern::new(p)).collect(),
            exclude_globs: exc.iter().map(|p| GlobPattern::new(p)).collect(),
            include_patterns: inc,
            exclude_patterns: exc,
        }
    }

    /// ファイルが比較対象に含まれるか(除外 > インクルード)
    pub fn allows_file(&self, name: &str, relative_path: &str) -> bool {
        if self
            .exclude_globs
            .iter()
            .any(|g| g.matches(name, relative_path))
        {
            return false;
        }
        if self.include_patterns.is_empty() {
            return true;
        }
        self.include_globs
            .iter()
            .any(|g| g.matches(name, relative_path))
    }

    /// エントリ共通の除外判定(ディレクトリは枝刈り)
    pub fn is_excluded(&self, name: &str, relative_path: &str) -> bool {
        self.exclude_globs
            .iter()
            .any(|g| g.matches(name, relative_path))
    }

    pub fn rules(&self) -> Vec<FilterRule> {
        self.include_patterns
            .iter()
            .map(|p| FilterRule::new(FilterKind::Include, p))
            .chain(
                self.exclude_patterns
                    .iter()
                    .map(|p| FilterRule::new(FilterKind::Exclude, p)),
            )
            .collect()
    }
}

pub struct FilterPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub exclude_patterns: &'static [&'static str],
}

pub const PRESET_SOURCE_CONTROL: FilterPreset = FilterPreset {
    id: "source-control",
    display_name: "ソース管理 (Source Control)",
    exclude_patterns: &[".git", ".svn", ".hg", ".bzr", "_darcs", "CVS"],
};
pub const PRESET_BUILD_ARTIFACTS: FilterPreset = FilterPreset {
    id: "build-artifacts",
    display_name: "ビルド成果物 (Build Artifacts)",
    exclude_patterns: &[
        "build",
        "dist",
        ".build",
        ".gradle",
        "DerivedData",
        "Pods",
        "*.o",
        "*.obj",
        "*.class",
        "*.pyc",
        "__pycache__",
        "*.dylib",
        "*.so",
        "*.dll",
        "*.exe",
    ],
};
pub const PRESET_WEB_NODEJS: FilterPreset = FilterPreset {
    id: "web-nodejs",
    display_name: "Web / Node.js",
    exclude_patterns: &[
        "node_modules",
        "bower_components",
        ".cache",
        ".parcel-cache",
        "*.min.js",
        "*.min.css",
        "coverage",
    ],
};
pub const PRESET_MAC_METADATA: FilterPreset = FilterPreset {
    id: "macos-metadata",
    display_name: "macOS メタデータ",
    exclude_patterns: &[
        ".DS_Store",
        "._*",
        ".Spotlight-V100",
        ".Trashes",
        ".fseventsd",
        ".DocumentRevisions-V100",
        ".AppleDouble",
    ],
};

pub static FILTER_PRESETS: &[FilterPreset] = &[
    PRESET_SOURCE_CONTROL,
    PRESET_BUILD_ARTIFACTS,
    PRESET_WEB_NODEJS,
    PRESET_MAC_METADATA,
];

pub fn filter_preset(id: &str) -> Option<&'static FilterPreset> {
    FILTER_PRESETS.iter().find(|p| p.id == id)
}
