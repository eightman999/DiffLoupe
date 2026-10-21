//! Swift `DiffLoupeTests/EngineOptionsTests.swift` の 1:1 移植。
//! UserDefaults/AppModel 依存の8件は対象外(TEST_MAP.md 参照)。

use super::support::{compare, temp_pair, write};
use diffloupe_core::comparator::{CompareError, CompareOptions, FolderComparator};
use diffloupe_core::filter::{FilterSet, FILTER_PRESETS};
use diffloupe_core::glob::GlobPattern;
use diffloupe_core::line_diff::{self, DiffRowKind};
use diffloupe_core::models::{ComparisonProgress, DiffStatus};
use diffloupe_core::options::ContentComparison;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

fn paths(result: &diffloupe_core::models::ComparisonResult) -> HashSet<&str> {
    result
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect()
}

fn status(result: &diffloupe_core::models::ComparisonResult, p: &str) -> DiffStatus {
    result.entry_by_path(p).unwrap().status
}

fn opts() -> CompareOptions {
    CompareOptions::default()
}

fn pats(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// MARK: - ユーザー定義除外glob

#[test]
fn test_glob_pattern_matching() {
    assert!(GlobPattern::new("*.bak").matches("a.bak", "sub/a.bak"));
    assert!(!GlobPattern::new("*.bak").matches("a.txt", "sub/a.txt"));
    assert!(GlobPattern::new("build/").matches("build", "build"));
    assert!(!GlobPattern::new("build/").matches("build", "build/x.txt"));
    assert!(GlobPattern::new("build/*").matches("x.txt", "build/x.txt"));
    assert!(!GlobPattern::new("build/*").matches("x.txt", "build/sub/x.txt"));
    assert!(GlobPattern::new("**/tmp/*.log").matches("a.log", "src/tmp/a.log"));
    assert!(GlobPattern::new("**/tmp/*.log").matches("a.log", "tmp/a.log"));
    assert!(!GlobPattern::new("**/tmp/*.log").matches("a.log", "src/tmp/keep.txt"));
    assert!(GlobPattern::new("a?c").matches("abc", "abc"));
    assert!(!GlobPattern::new("a?c").matches("ac", "ac"));
    assert!(GlobPattern::new("ab*").matches("ab", "ab"));
}

#[test]
fn test_user_exclude_glob_basename() {
    let (base, l, r) = temp_pair("eo1");
    write(&l, "a.txt", b"a");
    write(&l, "b.bak", b"b");
    write(&l, "sub/c.bak", b"c");
    write(&l, "sub/keep.txt", b"keep");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["*.bak"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    let p = paths(&result);
    assert!(p.contains("a.txt"));
    assert!(p.contains("sub/keep.txt"));
    assert!(!p.contains("b.bak"));
    assert!(!p.contains("sub/c.bak"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_user_exclude_glob_relative_path_prunes_subtree() {
    let (base, l, r) = temp_pair("eo2");
    write(&l, "build/x.txt", b"x");
    write(&l, "build/sub/y.txt", b"y");
    write(&l, "src/z.txt", b"z");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["build/"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    let p = paths(&result);
    assert!(!p.contains("build"));
    assert!(!p.contains("build/x.txt"));
    assert!(!p.contains("build/sub/y.txt"));
    assert!(p.contains("src/z.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_user_exclude_glob_double_star() {
    let (base, l, r) = temp_pair("eo3");
    write(&l, "tmp/a.log", b"log1");
    write(&l, "src/tmp/b.log", b"log2");
    write(&l, "src/tmp/keep.txt", b"keep");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["**/tmp/*.log"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    let p = paths(&result);
    assert!(!p.contains("tmp/a.log"));
    assert!(!p.contains("src/tmp/b.log"));
    assert!(p.contains("src/tmp/keep.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_user_exclude_glob_directory_name_prunes_whole_tree() {
    let (base, l, r) = temp_pair("eo4");
    write(&l, "vendor/package/index.js", b"index");
    write(&l, "app.txt", b"app");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["vendor"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    let p = paths(&result);
    assert!(!p.contains("vendor"));
    assert!(!p.contains("vendor/package/index.js"));
    assert!(p.contains("app.txt"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_user_exclude_glob_keeps_default_excludes() {
    let (base, l, r) = temp_pair("eo5");
    write(&l, ".git/config", b"cfg");
    write(&l, "junk.tmp", b"junk");
    write(&l, "keep.txt", b"keep");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["*.tmp"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(paths(&result), ["keep.txt"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 内容比較モード

fn cc_opts(f: impl FnOnce(&mut ContentComparison)) -> CompareOptions {
    let mut cc = ContentComparison::default();
    f(&mut cc);
    CompareOptions {
        content_comparison: cc,
        ..opts()
    }
}

#[test]
fn test_strict_mode_is_default_for_line_endings() {
    let (base, l, r) = temp_pair("eo6");
    write(&l, "f.txt", b"a\nb\n");
    write(&r, "f.txt", b"a\r\nb\r\n");
    let result = compare(&l, &r, &opts());
    assert_eq!(status(&result, "f.txt"), DiffStatus::Different);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_ignore_line_endings() {
    let (base, l, r) = temp_pair("eo7");
    write(&l, "f.txt", b"a\nb\n");
    write(&r, "f.txt", b"a\r\nb\r\n");
    let result = compare(&l, &r, &cc_opts(|c| c.ignore_line_endings = true));
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_ignore_trailing_whitespace() {
    let (base, l, r) = temp_pair("eo8");
    write(&l, "f.txt", b"a \nb\t\n");
    write(&r, "f.txt", b"a\nb\n");
    let result = compare(&l, &r, &cc_opts(|c| c.ignore_trailing_whitespace = true));
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_normalize_whitespace() {
    let (base, l, r) = temp_pair("eo9");
    write(&l, "f.txt", b"a  b\n");
    write(&r, "f.txt", b"a b\n");
    let result = compare(&l, &r, &cc_opts(|c| c.normalize_whitespace = true));
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_ignore_case() {
    let (base, l, r) = temp_pair("eo10");
    write(&l, "f.txt", b"Hello World\n");
    write(&r, "f.txt", b"hello world\n");
    let result = compare(&l, &r, &cc_opts(|c| c.ignore_case = true));
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_combined_normalization() {
    let (base, l, r) = temp_pair("eo11");
    write(&l, "f.txt", b"HELLO  WORLD\r\n");
    write(&r, "f.txt", b"hello world\n");
    let result = compare(
        &l,
        &r,
        &cc_opts(|c| {
            c.ignore_line_endings = true;
            c.normalize_whitespace = true;
            c.ignore_case = true;
        }),
    );
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_normalization_does_not_miss_real_differences() {
    let (base, l, r) = temp_pair("eo12");
    write(&l, "f.txt", b"hello\n");
    write(&r, "f.txt", b"world\n");
    let result = compare(&l, &r, &cc_opts(|c| c.ignore_case = true));
    assert_eq!(status(&result, "f.txt"), DiffStatus::Different);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_binary_files_use_byte_fallback_under_normalization() {
    let (base, l, r) = temp_pair("eo13");
    write(&l, "bin.dat", &[0x00, 0x01, 0x41]);
    write(&r, "bin.dat", &[0x00, 0x01, 0x41]);
    let same = compare(&l, &r, &cc_opts(|c| c.ignore_case = true));
    assert_eq!(status(&same, "bin.dat"), DiffStatus::Identical);
    write(&r, "bin.dat", &[0x00, 0x01, 0x42, 0x43]);
    let different = compare(&l, &r, &cc_opts(|c| c.ignore_case = true));
    assert_eq!(status(&different, "bin.dat"), DiffStatus::Different);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_line_diff_uses_shared_normalization() {
    let cc = ContentComparison {
        ignore_case: true,
        ..Default::default()
    };
    let rows = line_diff::rows("Hello World\n", "hello world\n", &cc).unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    let strict_rows = line_diff::rows(
        "Hello World\n",
        "hello world\n",
        &ContentComparison::default(),
    )
    .unwrap();
    assert!(!strict_rows.iter().all(|r| r.kind == DiffRowKind::Context));
}

// MARK: - リネーム/移動候補の検出

#[test]
fn test_rename_detection_unique_move() {
    let (base, l, r) = temp_pair("eo14");
    write(&l, "old.txt", b"shared content");
    write(&r, "new.txt", b"shared content");
    let o = CompareOptions {
        detect_renames: true,
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(status(&result, "old.txt"), DiffStatus::LeftOnly);
    assert_eq!(status(&result, "new.txt"), DiffStatus::RightOnly);
    let lc = result
        .entry_by_path("old.txt")
        .unwrap()
        .rename_candidate
        .as_ref()
        .unwrap();
    assert_eq!(lc.counterpart_path, "new.txt");
    assert!(lc.is_unique_match);
    assert_eq!(lc.total_matches, 1);
    let rc = result
        .entry_by_path("new.txt")
        .unwrap()
        .rename_candidate
        .as_ref()
        .unwrap();
    assert_eq!(rc.counterpart_path, "old.txt");
    assert!(rc.is_unique_match);
    assert_eq!(rc.total_matches, 1);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_rename_detection_disabled_by_default() {
    let (base, l, r) = temp_pair("eo15");
    write(&l, "old.txt", b"shared content");
    write(&r, "new.txt", b"shared content");
    let result = compare(&l, &r, &opts());
    assert!(result
        .entry_by_path("old.txt")
        .unwrap()
        .rename_candidate
        .is_none());
    assert!(result
        .entry_by_path("new.txt")
        .unwrap()
        .rename_candidate
        .is_none());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_rename_detection_duplicate_handling() {
    let (base, l, r) = temp_pair("eo16");
    write(&l, "a1.txt", b"dup content");
    write(&l, "a2.txt", b"dup content");
    write(&r, "b1.txt", b"dup content");
    let o = CompareOptions {
        detect_renames: true,
        ..opts()
    };
    let result = compare(&l, &r, &o);
    for path in ["a1.txt", "a2.txt"] {
        let c = result
            .entry_by_path(path)
            .unwrap()
            .rename_candidate
            .as_ref()
            .unwrap();
        assert!(!c.is_unique_match);
        assert_eq!(c.counterpart_path, "b1.txt");
        assert_eq!(c.total_matches, 1);
    }
    let rc = result
        .entry_by_path("b1.txt")
        .unwrap()
        .rename_candidate
        .as_ref()
        .unwrap();
    assert!(!rc.is_unique_match);
    assert_eq!(rc.total_matches, 2);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_rename_detection_ignores_directories_and_different_content() {
    let (base, l, r) = temp_pair("eo17");
    std::fs::create_dir_all(l.join("dir_only")).unwrap();
    std::fs::create_dir_all(r.join("moved_dir")).unwrap();
    write(&l, "only_left.txt", b"AAA");
    write(&r, "only_right.txt", b"BBB");
    let o = CompareOptions {
        detect_renames: true,
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert!(result
        .entry_by_path("dir_only")
        .unwrap()
        .rename_candidate
        .is_none());
    assert!(result
        .entry_by_path("moved_dir")
        .unwrap()
        .rename_candidate
        .is_none());
    assert!(result
        .entry_by_path("only_left.txt")
        .unwrap()
        .rename_candidate
        .is_none());
    assert!(result
        .entry_by_path("only_right.txt")
        .unwrap()
        .rename_candidate
        .is_none());
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - リネーム検出の進捗とキャンセル

#[test]
fn test_rename_detection_reports_progress() {
    let (base, l, r) = temp_pair("eo18");
    for i in 0..30 {
        let c = format!("content {}", i);
        write(&l, &format!("old{}.txt", i), c.as_bytes());
        write(&r, &format!("new{}.txt", i), c.as_bytes());
    }
    let boxx = Arc::new(Mutex::new((0usize, 0usize))); // (total, max done)
    let bx = boxx.clone();
    let progress = move |p: ComparisonProgress| {
        if let ComparisonProgress::Renames { done, total } = p {
            let mut g = bx.lock().unwrap();
            g.0 = total;
            g.1 = g.1.max(done);
        }
    };
    let o = CompareOptions {
        detect_renames: true,
        ..opts()
    };
    let result = FolderComparator::compare(&l, &r, &o, Some(&progress), None).unwrap();
    let (total, done) = *boxx.lock().unwrap();
    assert_eq!(total, 60); // 左30 + 右30
    assert_eq!(done, 60);
    assert!(result
        .entry_by_path("old0.txt")
        .unwrap()
        .rename_candidate
        .is_some());
    assert!(result
        .entry_by_path("new0.txt")
        .unwrap()
        .rename_candidate
        .is_some());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_rename_detection_cancellation_propagates() {
    // リネーム検出フェーズに入った時点でキャンセルし、Cancelled が返ること
    let (base, l, r) = temp_pair("eo19");
    for i in 0..40 {
        let data = "x".repeat(64);
        write(&l, &format!("old{}.txt", i), data.as_bytes());
        write(&r, &format!("new{}.txt", i), data.as_bytes());
    }
    let token = Arc::new(diffloupe_core::cancel::CancellationToken::new());
    let t2 = token.clone();
    let progress = move |p: ComparisonProgress| {
        if let ComparisonProgress::Renames { .. } = p {
            t2.cancel();
        }
    };
    let o = CompareOptions {
        detect_renames: true,
        ..opts()
    };
    let result = FolderComparator::compare(&l, &r, &o, Some(&progress), Some(&*token));
    assert!(matches!(result, Err(CompareError::Cancelled)));
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 巨大ファイル保護(正規化比較の一括読込上限)

#[test]
fn test_huge_files_fall_back_to_byte_compare_without_oom() {
    let (base, l, r) = temp_pair("eo20");
    let small_limit: i64 = 16;
    let text = "a".repeat(32);

    // サイズ不一致: 正規化比較せずバイト厳密(サイズ)で相違
    write(&l, "f.txt", text.as_bytes());
    write(&r, "f.txt", (text.clone() + &text).as_bytes());
    let mut o = cc_opts(|c| c.ignore_line_endings = true);
    o.max_normalization_compare_bytes = small_limit;
    let diff = compare(&l, &r, &o);
    assert_eq!(status(&diff, "f.txt"), DiffStatus::Different);

    // サイズ一致・同一内容: ハッシュ経路で一致
    write(&r, "f.txt", text.as_bytes());
    let same = compare(&l, &r, &o);
    assert_eq!(status(&same, "f.txt"), DiffStatus::Identical);

    // 上限内のファイルは従来どおり正規化比較が働く(CRLF差でも一致)
    write(&l, "g.txt", b"a\r\nb\r\n");
    write(&r, "g.txt", b"a\nb\n");
    let in_limit = compare(&l, &r, &cc_opts(|c| c.ignore_line_endings = true));
    assert_eq!(status(&in_limit, "g.txt"), DiffStatus::Identical);
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - globの空パターン・Unicode

#[test]
fn test_empty_glob_pattern_matches_nothing() {
    assert!(!GlobPattern::new("").matches("a.txt", "a.txt"));
    assert!(!GlobPattern::new("").matches("a.txt", "sub/a.txt"));
    assert!(!GlobPattern::new("  ").matches("a.txt", "a.txt"));
}

#[test]
fn test_glob_pattern_with_unicode_and_nfc() {
    assert!(GlobPattern::new("*.txt").matches("ファイル.txt", "ディレクトリ/ファイル.txt"));
    assert!(GlobPattern::new("**/サブ/*.txt").matches("a.txt", "ディレクトリ/サブ/a.txt"));
    assert!(GlobPattern::new("写真?.jpg").matches("写真A.jpg", "写真A.jpg"));
    assert!(!GlobPattern::new("写真?.jpg").matches("写真AB.jpg", "写真AB.jpg"));
    // NFD の相対パスに対しても grapheme 比較で一致する
    let nfc = "が";
    assert!(GlobPattern::new("*.txt").matches("file.txt", &format!("{}/file.txt", nfc)));
}

// MARK: - 署名・元表示・メタデータ

#[test]
fn test_content_comparison_signature_reflects_settings() {
    let mut comparison = ContentComparison::default();
    assert_eq!(comparison.settings_signature(), "0000");
    comparison.ignore_line_endings = true;
    assert_eq!(comparison.settings_signature(), "1000");
    comparison.ignore_trailing_whitespace = true;
    assert_eq!(comparison.settings_signature(), "1100");
    comparison.normalize_whitespace = true;
    assert_eq!(comparison.settings_signature(), "1110");
    comparison.ignore_case = true;
    assert_eq!(comparison.settings_signature(), "1111");
}

#[test]
fn test_ignore_case_keeps_original_display_text() {
    let cc = ContentComparison {
        ignore_case: true,
        ..Default::default()
    };
    let rows = line_diff::rows(
        "Hello World\nSecond Line\n",
        "hello world\nsecond line\n",
        &cc,
    )
    .unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    let context: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Context)
        .collect();
    assert_eq!(context[0].left_text.as_deref(), Some("Hello World"));
    assert_eq!(context[0].right_text.as_deref(), Some("hello world"));
    assert_eq!(context[1].left_text.as_deref(), Some("Second Line"));
    assert_eq!(context[1].right_text.as_deref(), Some("second line"));
}

#[test]
fn test_normalize_whitespace_keeps_original_display_text() {
    let cc = ContentComparison {
        normalize_whitespace: true,
        ..Default::default()
    };
    let rows = line_diff::rows("a  b\n", "a b\n", &cc).unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    let row = rows
        .iter()
        .find(|r| r.kind == DiffRowKind::Context)
        .unwrap();
    assert_eq!(row.left_text.as_deref(), Some("a  b"));
    assert_eq!(row.right_text.as_deref(), Some("a b"));
}

#[test]
fn test_ignore_line_endings_keeps_original_cr_in_display() {
    let cc = ContentComparison {
        ignore_line_endings: true,
        ..Default::default()
    };
    let rows = line_diff::rows("a\r\nb\r\n", "a\nb\n", &cc).unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    assert_eq!(rows[0].left_text.as_deref(), Some("a\r"));
    assert_eq!(rows[0].right_text.as_deref(), Some("a"));
}

#[test]
fn test_metadata_is_exposed() {
    let (base, l, r) = temp_pair("eo21");
    write(&l, "f.txt", b"same");
    write(&r, "f.txt", b"same");
    let result = compare(&l, &r, &opts());
    let entry = result.entry_by_path("f.txt").unwrap();
    assert_eq!(entry.left_size, 4);
    assert_eq!(entry.right_size, 4);
    assert!(entry.left_modification_date.is_some());
    assert!(entry.right_modification_date.is_some());
    // POSIX パーミッションは Unix のみ。Windows では None を返す契約。
    let (lp, rp) = (
        entry.left_posix.and_then(|p| p.permissions),
        entry.right_posix.and_then(|p| p.permissions),
    );
    if cfg!(unix) {
        assert!(lp.is_some());
        assert!(rp.is_some());
    } else {
        assert!(lp.is_none());
        assert!(rp.is_none());
        assert!(entry.left_posix.is_none() && entry.right_posix.is_none());
    }
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - インクルードglob(フォルダ比較)

#[test]
fn test_user_include_glob_filters_files_by_basename() {
    let (base, l, r) = temp_pair("eo22");
    for t in ["a", "b"] {
        write(&l, "a.swift", t.as_bytes());
        write(&l, "b.txt", t.as_bytes());
        write(&r, "a.swift", t.as_bytes());
        write(&r, "b.txt", t.as_bytes());
    }
    let o = CompareOptions {
        user_include_patterns: pats(&["*.swift"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(paths(&result), ["a.swift"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_user_include_glob_reaches_nested_files() {
    let (base, l, r) = temp_pair("eo23");
    write(&l, "src/Engine/Foo.swift", b"AAA");
    write(&r, "src/Engine/Foo.swift", b"BBB");
    write(&l, "src/README.md", b"readme");
    write(&l, "top.txt", b"top");
    let o = CompareOptions {
        user_include_patterns: pats(&["*.swift"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    let p = paths(&result);
    assert!(p.contains("src"));
    assert!(p.contains("src/Engine"));
    assert!(p.contains("src/Engine/Foo.swift"));
    assert!(!p.contains("src/README.md"));
    assert!(!p.contains("top.txt"));
    assert_eq!(
        status(&result, "src/Engine/Foo.swift"),
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_empty_include_keeps_existing_exclude_behavior() {
    let (base, l, r) = temp_pair("eo24");
    write(&l, "a.txt", b"a");
    write(&l, "b.bak", b"b");
    write(&l, "keep.txt", b"keep");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["*.bak"]),
        user_include_patterns: vec![],
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(paths(&result), ["a.txt", "keep.txt"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_include_exclude_precedence() {
    let (base, l, r) = temp_pair("eo25");
    write(&l, "Foo.swift", b"a");
    write(&l, "Generated.swift", b"b");
    write(&l, "notes.txt", b"c");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["Generated.swift"]),
        user_include_patterns: pats(&["*.swift"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(paths(&result), ["Foo.swift"].into_iter().collect());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_excluded_directory_prunes_subtree_under_include() {
    let (base, l, r) = temp_pair("eo26");
    write(&l, "src/app.swift", b"a");
    write(&l, "build/tmp.swift", b"b");
    write(&l, "build/sub/deep.swift", b"c");
    let o = CompareOptions {
        user_exclude_patterns: pats(&["build"]),
        user_include_patterns: pats(&["**"]),
        ..opts()
    };
    let result = compare(&l, &r, &o);
    assert_eq!(
        paths(&result),
        ["src", "src/app.swift"].into_iter().collect()
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - FilterSet 単体(EngineOptions 内)

#[test]
fn test_filter_set_unit_semantics() {
    let filter = FilterSet::new(pats(&["*.swift"]), pats(&["**/Generated.swift"]));
    assert!(filter.allows_file("Foo.swift", "src/Foo.swift"));
    assert!(!filter.allows_file("Generated.swift", "src/Generated.swift"));
    assert!(!filter.allows_file("README.md", "README.md"));
    assert!(!filter.is_excluded("src", "src"));
    assert!(FilterSet::default().allows_file("anything", "anything"));
    let ids: Vec<_> = FILTER_PRESETS.iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        [
            "source-control",
            "build-artifacts",
            "web-nodejs",
            "macos-metadata"
        ]
    );
}

#[test]
fn test_filter_set_drops_blank_patterns_so_empty_filter_allows_everything() {
    let blank_includes = FilterSet::new(pats(&["", "  "]), vec![]);
    assert_eq!(blank_includes.include_patterns, Vec::<String>::new());
    assert!(blank_includes.allows_file("a.txt", "a.txt"));
    assert!(blank_includes.allows_file("a.txt", "sub/a.txt"));
    let blank_excludes = FilterSet::new(vec![], pats(&["", "  "]));
    assert_eq!(blank_excludes.exclude_patterns, Vec::<String>::new());
    assert!(blank_excludes.allows_file("a.txt", "a.txt"));
}

#[test]
fn test_filter_set_trims_and_deduplicates_patterns_preserving_order() {
    let filter = FilterSet::new(
        pats(&[" *.swift ", "*.swift", "src/**", "", "src/**"]),
        pats(&["build/", " build/ "]),
    );
    assert_eq!(filter.include_patterns, pats(&["*.swift", "src/**"]));
    assert_eq!(filter.exclude_patterns, pats(&["build/"]));
}

#[test]
fn test_filter_set_unicode_matching_is_normalization_insensitive() {
    let composed = "が.txt";
    let decomposed = "か\u{3099}.txt";
    let filter = FilterSet::new(pats(&["*.txt", composed]), vec![]);
    assert!(filter.allows_file(composed, composed));
    assert!(filter.allows_file(decomposed, decomposed));
    assert!(filter.allows_file(decomposed, &format!("sub/{}", decomposed)));
}

// MARK: - 後方互換(既定オプションは既存挙動と同一)

#[test]
fn test_default_options_preserve_strict_classification() {
    let (base, l, r) = temp_pair("eo27");
    write(&l, "f.txt", b"same");
    write(&r, "f.txt", b"same");
    write(&l, "g.txt", b"a");
    write(&r, "g.txt", b"b");
    write(&l, "only_left.txt", b"only");
    let result = compare(&l, &r, &opts());
    assert_eq!(status(&result, "f.txt"), DiffStatus::Identical);
    assert_eq!(status(&result, "g.txt"), DiffStatus::Different);
    assert_eq!(status(&result, "only_left.txt"), DiffStatus::LeftOnly);
    assert!(result
        .entry_by_path("only_left.txt")
        .unwrap()
        .rename_candidate
        .is_none());
    std::fs::remove_dir_all(&base).ok();
}
