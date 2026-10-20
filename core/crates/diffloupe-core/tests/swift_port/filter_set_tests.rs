//! Swift `CoreTests/FilterSetTests.swift` の 1:1 移植(23件)

use super::support::{compare, temp_pair, write};
use diffloupe_core::comparator::CompareOptions;
use diffloupe_core::filter::{filter_preset, FilterKind, FilterRule, FilterSet, FILTER_PRESETS};
use diffloupe_core::models::DiffStatus;

fn inc(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// MARK: - FilterSet(純粋な型)

#[test]
fn test_empty_include_allows_everything() {
    let filter = FilterSet::default();
    assert!(filter.allows_file("a.txt", "a.txt"));
    assert!(filter.allows_file("a.txt", "sub/a.txt"));
    assert!(filter.allows_file("build", "build"));
    assert!(filter.allows_file("x", "src/x"));
}

#[test]
fn test_empty_include_still_honors_excludes() {
    let filter = FilterSet::new(vec![], inc(&["*.bak", "build/"]));
    assert!(!filter.allows_file("a.bak", "a.bak"));
    assert!(!filter.allows_file("build", "build"));
    assert!(filter.allows_file("a.txt", "a.txt"));
}

#[test]
fn test_non_empty_include_requires_match() {
    let filter = FilterSet::new(inc(&["*.swift"]), vec![]);
    assert!(filter.allows_file("Foo.swift", "src/Engine/Foo.swift"));
    assert!(filter.allows_file("Foo.swift", "build/Foo.swift"));
    assert!(!filter.allows_file("README.md", "README.md"));
    assert!(!filter.allows_file("notes.txt", "build/notes.txt"));
}

#[test]
fn test_include_matches_relative_path_with_separator() {
    let filter = FilterSet::new(inc(&["src/Engine/*"]), vec![]);
    assert!(filter.allows_file("Foo.swift", "src/Engine/Foo.swift"));
    assert!(!filter.allows_file("Foo.swift", "other/Foo.swift"));
    assert!(!filter.allows_file("Foo.swift", "src/Other/Foo.swift"));
}

#[test]
fn test_exclude_wins_over_include() {
    let filter = FilterSet::new(inc(&["*.swift"]), inc(&["**/Generated.swift"]));
    assert!(!filter.allows_file("Generated.swift", "src/Generated.swift"));
    assert!(filter.allows_file("Foo.swift", "src/Foo.swift"));
}

#[test]
fn test_include_does_not_prune_directories() {
    let filter = FilterSet::new(inc(&["*.swift"]), vec![]);
    assert!(!filter.is_excluded("src", "src"));
    assert!(!filter.is_excluded("Engine", "src/Engine"));
    assert!(!filter.is_excluded("build", "src/build"));
}

#[test]
fn test_excluded_directory_prunes_subtree() {
    let filter = FilterSet::new(inc(&["**"]), inc(&["build"]));
    assert!(filter.is_excluded("build", "build"));
    assert!(!filter.is_excluded("src", "src"));
}

#[test]
fn test_blank_and_empty_patterns_are_dropped_from_filter() {
    assert!(FilterSet::default().allows_file("a.txt", "a.txt"));
    let blank_excludes = FilterSet::new(vec![], inc(&["", "  "]));
    assert_eq!(blank_excludes.exclude_patterns, Vec::<String>::new());
    assert!(blank_excludes.allows_file("a.txt", "a.txt"));
    let only_blank_includes = FilterSet::new(inc(&["", "  "]), vec![]);
    assert_eq!(only_blank_includes.include_patterns, Vec::<String>::new());
    assert!(only_blank_includes.allows_file("a.txt", "a.txt"));
    assert!(only_blank_includes.allows_file("a.txt", "sub/a.txt"));
}

#[test]
fn test_patterns_are_trimmed_and_deduplicated_preserving_order() {
    let filter = FilterSet::new(
        inc(&[" *.swift ", "*.swift", "", "  ", "src/**"]),
        inc(&["build/", " build/ ", "node_modules"]),
    );
    assert_eq!(filter.include_patterns, inc(&["*.swift", "src/**"]));
    assert_eq!(filter.exclude_patterns, inc(&["build/", "node_modules"]));
}

#[test]
fn test_unicode_filter_matching_is_normalization_insensitive() {
    // FilterSet 自体は正規化せず呼び出し側が NFC へ揃える契約。
    // grapheme 単位比較のため NFD でも同一クラスタとして一致する。
    let composed = "が.txt"; // NFC
    let decomposed = "か\u{3099}.txt"; // NFD
    let filter = FilterSet::new(inc(&["*.txt"]), vec![]);
    assert!(filter.allows_file(composed, composed));
    assert!(filter.allows_file(decomposed, decomposed));
    assert!(filter.allows_file(decomposed, &format!("sub/{}", decomposed)));
}

#[test]
fn test_rules_flattening() {
    let filter = FilterSet::new(inc(&["*.swift"]), inc(&["build/"]));
    let rules = filter.rules();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0], FilterRule::new(FilterKind::Include, "*.swift"));
    assert_eq!(rules[1], FilterRule::new(FilterKind::Exclude, "build/"));
}

// MARK: - FilterRule

#[test]
fn test_filter_rule_matching() {
    let include = FilterRule::new(FilterKind::Include, "*.swift");
    assert_eq!(include.kind, FilterKind::Include);
    assert!(include.matches("Foo.swift", "src/Foo.swift"));
    assert!(!include.matches("Foo.txt", "src/Foo.txt"));
}

// MARK: - FilterPreset

#[test]
fn test_all_presets_have_non_empty_safe_excludes() {
    assert_eq!(FILTER_PRESETS.len(), 4);
    for preset in FILTER_PRESETS {
        assert!(!preset.id.is_empty(), "{} のIDが空", preset.display_name);
        assert!(!preset.display_name.is_empty());
        assert!(
            !preset.exclude_patterns.is_empty(),
            "{} の除外パターンが空",
            preset.display_name
        );
    }
}

#[test]
fn test_preset_ids_are_unique() {
    let ids: Vec<&str> = FILTER_PRESETS.iter().map(|p| p.id).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn test_preset_lookup_by_id() {
    assert_eq!(
        filter_preset("source-control").unwrap().id,
        "source-control"
    );
    assert_eq!(
        filter_preset("build-artifacts").unwrap().id,
        "build-artifacts"
    );
    assert_eq!(filter_preset("web-nodejs").unwrap().id, "web-nodejs");
    assert_eq!(
        filter_preset("macos-metadata").unwrap().id,
        "macos-metadata"
    );
    assert!(filter_preset("unknown").is_none());
    assert!(filter_preset("").is_none());
}

#[test]
fn test_preset_patterns_are_sane() {
    let p = |id: &str| filter_preset(id).unwrap().exclude_patterns;
    assert!(p("source-control").contains(&".git"));
    assert!(p("build-artifacts").contains(&"build"));
    assert!(p("web-nodejs").contains(&"node_modules"));
    assert!(p("macos-metadata").contains(&".DS_Store"));
}

// MARK: - FolderComparatorとの統合(一時ディレクトリ対)

#[test]
fn test_include_glob_filters_files_by_basename() {
    let (base, l, r) = temp_pair("fs1");
    for t in ["a", "b"] {
        write(&l, "a.swift", t.as_bytes());
        write(&l, "b.txt", t.as_bytes());
        write(&r, "a.swift", t.as_bytes());
        write(&r, "b.txt", t.as_bytes());
    }
    let options = CompareOptions {
        user_include_patterns: inc(&["*.swift"]),
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["a.swift"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_include_reaches_nested_files_through_non_matching_directories() {
    let (base, l, r) = temp_pair("fs2");
    write(&l, "src/Engine/Foo.swift", b"AAA");
    write(&r, "src/Engine/Foo.swift", b"BBB");
    write(&l, "src/README.md", b"readme");
    write(&l, "top.txt", b"top");
    let options = CompareOptions {
        user_include_patterns: inc(&["*.swift"]),
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert!(paths.contains("src"));
    assert!(paths.contains("src/Engine"));
    assert!(paths.contains("src/Engine/Foo.swift"));
    assert!(!paths.contains("src/README.md"));
    assert!(!paths.contains("top.txt"));
    assert_eq!(
        result.entry_by_path("src/Engine/Foo.swift").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_empty_include_keeps_existing_exclude_behavior() {
    let (base, l, r) = temp_pair("fs3");
    write(&l, "a.txt", b"a");
    write(&l, "b.bak", b"b");
    write(&l, "keep.txt", b"keep");
    let options = CompareOptions {
        user_exclude_patterns: inc(&["*.bak"]),
        user_include_patterns: vec![],
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["a.txt", "keep.txt"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_empty_include_with_no_excludes_keeps_all_files() {
    let (base, l, r) = temp_pair("fs4");
    write(&l, "a.txt", b"a");
    write(&l, "sub/b.txt", b"b");
    let options = CompareOptions {
        user_include_patterns: vec![],
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["a.txt", "sub", "sub/b.txt"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_include_exclude_precedence_in_comparator() {
    let (base, l, r) = temp_pair("fs5");
    write(&l, "Foo.swift", b"a");
    write(&l, "Generated.swift", b"b");
    write(&l, "notes.txt", b"c");
    let options = CompareOptions {
        user_exclude_patterns: inc(&["Generated.swift"]),
        user_include_patterns: inc(&["*.swift"]),
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["Foo.swift"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_excluded_directory_prunes_subtree_under_includes() {
    let (base, l, r) = temp_pair("fs6");
    write(&l, "src/app.swift", b"a");
    write(&l, "build/tmp.swift", b"b");
    write(&l, "build/sub/deep.swift", b"c");
    let options = CompareOptions {
        user_exclude_patterns: inc(&["build"]),
        user_include_patterns: inc(&["**"]),
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    let paths: std::collections::HashSet<_> = result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert_eq!(paths, ["src", "src/app.swift"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_nfc_normalized_result_keys_for_unicode_names() {
    // APFS/HFS+はNFDを返すため、走査は結果キーをNFCへ揃える。
    let (base, l, r) = temp_pair("fs7");
    let composed = "が.txt";
    let decomposed = "か\u{3099}.txt";
    write(&l, decomposed, b"a");
    write(&l, "other.txt", b"b");
    let result = compare(&l, &r, &CompareOptions::default());
    let key = result.entries.iter().find(|e| e.relative_path == composed);
    assert!(key.is_some());
    let scalars: Vec<u32> = key
        .unwrap()
        .relative_path
        .chars()
        .map(|c| c as u32)
        .collect();
    assert_eq!(
        scalars,
        vec![0x304C, 0x2E, 0x74, 0x78, 0x74],
        "結果キーはNFC(が.txt)であるべき"
    );
    std::fs::remove_dir_all(&base).ok();
}
