//! Swift `DiffLoupeTests`/`CoreTests` の移植テスト。
//! 詳細な対応表は `core/tests/parity/TEST_MAP.md` を参照。

use diffloupe_core::cancel::CancellationToken;
use diffloupe_core::comparator::{CompareOptions, FolderComparator};
use diffloupe_core::diff_display as dd;
use diffloupe_core::filter::{filter_preset, FilterKind, FilterSet, FILTER_PRESETS};
use diffloupe_core::glob::GlobPattern;
use diffloupe_core::hex_diff;
use diffloupe_core::inline_diff;
use diffloupe_core::line_diff::{self, DiffRow, DiffRowKind};
use diffloupe_core::models::DiffStatus;
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured::{self, StructuredComparisonMode as SM};
use diffloupe_core::sync_plan::{SyncAction, SyncDirection, SyncPlan};
use diffloupe_core::text_loader::{self, TextLoadResult};
use diffloupe_core::three_way::{self, ThreeWayDiff, ThreeWayDiffStatus as TWS};
use std::path::{Path, PathBuf};

// ---------- helpers ----------
fn temp_pair(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "diffloupe-test-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let l = base.join("left");
    let r = base.join("right");
    std::fs::create_dir_all(&l).unwrap();
    std::fs::create_dir_all(&r).unwrap();
    (base, l, r)
}
fn write(dir: &Path, rel: &str, data: &[u8]) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, data).unwrap();
}
fn compare(l: &Path, r: &Path, opts: &CompareOptions) -> diffloupe_core::models::ComparisonResult {
    FolderComparator::compare(l, r, opts, None, None).unwrap()
}

// ---------- GlobPattern ----------
#[test]
fn glob_basics() {
    assert!(GlobPattern::new("*.bak").matches("x.bak", "dir/x.bak"));
    assert!(!GlobPattern::new("*.bak").matches("x.txt", "dir/x.txt"));
    assert!(GlobPattern::new("build/").matches("build", "build"));
    assert!(!GlobPattern::new("build/").matches("build", "build/x.txt"));
    assert!(GlobPattern::new("build/*").matches("x", "build/x"));
    assert!(!GlobPattern::new("build/*").matches("x", "build/sub/x"));
    assert!(GlobPattern::new("**/tmp/*.log").matches("a.log", "x/tmp/a.log"));
    assert!(!GlobPattern::new("**/tmp/*.log").matches("a.log", "tmp2/a.log"));
    assert!(GlobPattern::new("a?c").matches("abc", "abc"));
    assert!(!GlobPattern::new("a?c").matches("abcd", "abcd"));
    assert!(GlobPattern::new("ab*").matches("ab", "ab"));
    assert!(GlobPattern::new("ab*").matches("abcdef", "abcdef"));
    assert!(!GlobPattern::new("*.TXT").matches("x.txt", "x.txt"));
    assert!(!GlobPattern::new("").matches("a.txt", "a.txt"));
    assert!(!GlobPattern::new("  ").matches("a.txt", "a.txt"));
    assert!(GlobPattern::new("*.txt").matches("file.txt", "dir/sub/file.txt"));
}

// ---------- FilterSet ----------
#[test]
fn filter_semantics() {
    let f = FilterSet::new(vec!["*.swift".into()], vec!["**/Generated.swift".into()]);
    assert!(f.allows_file("Foo.swift", "src/Foo.swift"));
    assert!(!f.allows_file("Generated.swift", "src/Generated.swift"));
    assert!(!f.allows_file("README.md", "README.md"));
    assert!(!f.is_excluded("src", "src"));
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
    let blank = FilterSet::new(vec!["".into(), "  ".into()], vec![]);
    assert!(blank.include_patterns.is_empty());
    assert!(blank.allows_file("a.txt", "sub/a.txt"));
    let f2 = FilterSet::new(
        vec![
            " *.swift ".into(),
            "*.swift".into(),
            "src/**".into(),
            "".into(),
            "src/**".into(),
        ],
        vec!["build/".into(), " build/ ".into()],
    );
    assert_eq!(f2.include_patterns, ["*.swift", "src/**"]);
    assert_eq!(f2.exclude_patterns, ["build/"]);
    assert!(filter_preset("web-nodejs").is_some());
    assert!(filter_preset("bogus").is_none());
    let rules = f.rules();
    assert!(rules.iter().any(|r| r.kind == FilterKind::Include));
}

// ---------- ContentComparison ----------
#[test]
fn content_comparison_signature() {
    let mut cc = ContentComparison::default();
    assert!(cc.is_strict());
    assert_eq!(cc.settings_signature(), "0000");
    cc.ignore_line_endings = true;
    assert_eq!(cc.settings_signature(), "1000");
    cc.ignore_trailing_whitespace = true;
    assert_eq!(cc.settings_signature(), "1100");
    cc.normalize_whitespace = true;
    assert_eq!(cc.settings_signature(), "1110");
    cc.ignore_case = true;
    assert_eq!(cc.settings_signature(), "1111");
    cc.structured_mode = SM::Json;
    assert_eq!(cc.settings_signature(), "1111|json");
    assert!(!cc.is_strict());
}

#[test]
fn normalization_lines() {
    let cc = ContentComparison {
        normalize_whitespace: true,
        ..Default::default()
    };
    assert_eq!(cc.normalized_line("a  b   c"), "a b c");
    let cc = ContentComparison {
        ignore_trailing_whitespace: true,
        ..Default::default()
    };
    assert_eq!(cc.normalized_line("x   "), "x");
    // .whitespaces = スペース/タブのみで \r は残る
    assert_eq!(cc.normalized_line("x\r"), "x\r");
    let cc = ContentComparison {
        ignore_case: true,
        ..Default::default()
    };
    assert_eq!(cc.normalized_line("Hello WORLD"), "hello world");
    let cc = ContentComparison {
        ignore_line_endings: true,
        ..Default::default()
    };
    assert_eq!(cc.normalized_text("a\r\nb\rc"), "a\nb\nc");
    // UTF-8 BOM は normalized_data 経路で除去される
    let cc = ContentComparison {
        ignore_case: true,
        ..Default::default()
    };
    let a = cc.normalized_data(b"\xef\xbb\xbfHello\n").unwrap();
    let b = cc.normalized_data(b"hello\n").unwrap();
    assert_eq!(a, b);
}

// ---------- LineDiff ----------
#[test]
fn line_diff_basics() {
    let cc = ContentComparison::default();
    let rows = line_diff::rows("a\nb\nc\n", "a\nb\nc\n", &cc).unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    assert_eq!(rows.len(), 4);
    let rows = line_diff::rows("a\nb\n", "a\nx\nb\n", &cc).unwrap();
    let added: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Added)
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].right_text.as_deref(), Some("x"));
    assert_eq!(added[0].left_number, None);
    assert_eq!(added[0].right_number, Some(2));
    let rows = line_diff::rows("a\nx\nb\n", "a\nb\n", &cc).unwrap();
    let removed: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Removed)
        .collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].left_text.as_deref(), Some("x"));
    let rows = line_diff::rows("a\nb\nc\n", "a\nX\nc\n", &cc).unwrap();
    let modified: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Modified)
        .collect();
    assert_eq!(modified.len(), 1);
    assert_eq!(modified[0].left_text.as_deref(), Some("b"));
    assert_eq!(modified[0].right_text.as_deref(), Some("X"));
    let rows = line_diff::rows("1\n2\n3\n4\n", "1\nA\nB\nC\n4\n", &cc).unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r.kind == DiffRowKind::Modified)
            .count(),
        2
    );
    assert_eq!(
        rows.iter().filter(|r| r.kind == DiffRowKind::Added).count(),
        1
    );
    let rows = line_diff::rows("a\r\nb\r\n", "a\nb\n", &cc).unwrap();
    assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    let rows = line_diff::rows("a\nb\nc\n", "a\nX\nY\nc\n", &cc).unwrap();
    let lnums: Vec<usize> = rows.iter().filter_map(|r| r.left_number).collect();
    let rnums: Vec<usize> = rows.iter().filter_map(|r| r.right_number).collect();
    assert_eq!(lnums, vec![1, 2, 3, 4]);
    assert_eq!(rnums, vec![1, 2, 3, 4, 5]);
    let big = "line\n".repeat(line_diff::MAX_TOTAL_LINES / 2 + 1);
    assert!(line_diff::rows(&big, &big, &cc).is_none());
    // 正規化しても元表示は保持される
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
    assert_eq!(rows[0].left_text.as_deref(), Some("Hello World"));
    assert_eq!(rows[0].right_text.as_deref(), Some("hello world"));
    let cc = ContentComparison {
        ignore_line_endings: true,
        ..Default::default()
    };
    let rows = line_diff::rows("a\r\nb\r\n", "a\nb\n", &cc).unwrap();
    assert_eq!(rows[0].left_text.as_deref(), Some("a\r"));
    assert_eq!(rows[0].right_text.as_deref(), Some("a"));
}

// ---------- InlineDiff ----------
#[test]
fn inline_diff_basics() {
    assert!(inline_diff::highlight(Some("abc"), Some("abc"), false).is_none());
    assert!(inline_diff::highlight(Some(""), Some(""), false).is_none());
    assert!(inline_diff::highlight(None, Some("abc"), false).is_none());
    let h = inline_diff::highlight(Some("abc"), Some("axc"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    assert_eq!(h.right_ranges, vec![1..2]);
    let h = inline_diff::highlight(Some("ab"), Some("aXb"), false).unwrap();
    assert_eq!(h.left_ranges, Vec::<std::ops::Range<usize>>::new());
    assert_eq!(h.right_ranges, vec![1..2]);
    let h = inline_diff::highlight(Some("aXb"), Some("ab"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    let h = inline_diff::highlight(Some(""), Some("abc"), false).unwrap();
    assert_eq!(h.right_ranges, vec![0..3]);
    let h = inline_diff::highlight(Some("abc"), Some("XXabcYY"), false).unwrap();
    assert_eq!(h.right_ranges, vec![0..2, 5..7]);
    let h = inline_diff::highlight(Some("XXabcYY"), Some("abc"), false).unwrap();
    assert_eq!(h.left_ranges, vec![0..2, 5..7]);
    assert!(inline_diff::highlight(Some("Hello World"), Some("hello world"), true).is_none());
    let h = inline_diff::highlight(Some("Hello World"), Some("hello world"), false).unwrap();
    assert_eq!(h.left_ranges, vec![0..1, 6..7]);
    let h = inline_diff::highlight(Some("Hello"), Some("hello world"), true).unwrap();
    assert_eq!(h.right_ranges, vec![5..11]);
    let h = inline_diff::highlight(Some("こんにちは"), Some("こんばんは"), false).unwrap();
    assert_eq!(h.left_ranges, vec![2..4]);
    let h = inline_diff::highlight(Some("a🎉b"), Some("a😀b"), false).unwrap();
    assert_eq!(h.left_ranges, vec![1..2]);
    let over = "x".repeat(inline_diff::MAX_CHARACTERS / 2 + 1);
    assert!(inline_diff::highlight(Some(&over), Some(&over), false).is_none());
    let half = "x".repeat(inline_diff::MAX_CHARACTERS / 2 - 1);
    assert!(inline_diff::highlight(Some(&half), Some(&(half.clone() + "y")), false).is_some());
}

#[test]
fn inline_diff_for_rows() {
    let rows = vec![
        DiffRow {
            id: 0,
            kind: DiffRowKind::Context,
            left_number: Some(1),
            left_text: Some("same".into()),
            right_number: Some(1),
            right_text: Some("same".into()),
        },
        DiffRow {
            id: 1,
            kind: DiffRowKind::Modified,
            left_number: Some(2),
            left_text: Some("abc".into()),
            right_number: Some(2),
            right_text: Some("axc".into()),
        },
        DiffRow {
            id: 2,
            kind: DiffRowKind::Added,
            left_number: None,
            left_text: None,
            right_number: Some(3),
            right_text: Some("new".into()),
        },
        DiffRow {
            id: 3,
            kind: DiffRowKind::Removed,
            left_number: Some(4),
            left_text: Some("old".into()),
            right_number: None,
            right_text: None,
        },
        DiffRow {
            id: 4,
            kind: DiffRowKind::Modified,
            left_number: Some(5),
            left_text: Some("Hello".into()),
            right_number: Some(5),
            right_text: Some("Hello World".into()),
        },
    ];
    let h = inline_diff::highlights_for_rows(&rows, false);
    assert_eq!(h.len(), 2);
    assert!(h.contains_key(&1) && h.contains_key(&4));
    let rows2 = vec![
        DiffRow {
            id: 0,
            kind: DiffRowKind::Modified,
            left_number: Some(1),
            left_text: Some("Hello World".into()),
            right_number: Some(1),
            right_text: Some("hello world".into()),
        },
        DiffRow {
            id: 1,
            kind: DiffRowKind::Modified,
            left_number: Some(2),
            left_text: Some("abc".into()),
            right_number: Some(2),
            right_text: Some("axc".into()),
        },
    ];
    let ins = inline_diff::highlights_for_rows(&rows2, true);
    assert!(!ins.contains_key(&0) && ins.contains_key(&1));
}

// ---------- DiffDisplay ----------
fn make_rows() -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut id = 0usize;
    let add = |kind, l: Option<&str>, r: Option<&str>, rows: &mut Vec<DiffRow>, id: &mut usize| {
        rows.push(DiffRow {
            id: *id,
            kind,
            left_number: Some(*id + 1),
            left_text: l.map(String::from),
            right_number: Some(*id + 1),
            right_text: r.map(String::from),
        });
        *id += 1;
    };
    add(
        DiffRowKind::Context,
        Some("c1"),
        Some("c1"),
        &mut rows,
        &mut id,
    );
    add(
        DiffRowKind::Context,
        Some("c2"),
        Some("c2"),
        &mut rows,
        &mut id,
    );
    add(
        DiffRowKind::Modified,
        Some("m1"),
        Some("m1'"),
        &mut rows,
        &mut id,
    );
    for i in 0..7 {
        let s = format!("g{}", i);
        add(
            DiffRowKind::Context,
            Some(&s.clone()),
            Some(&s),
            &mut rows,
            &mut id,
        );
    }
    add(DiffRowKind::Added, None, Some("new"), &mut rows, &mut id);
    for i in 1..=3 {
        let s = format!("x{}", i);
        add(
            DiffRowKind::Context,
            Some(&s.clone()),
            Some(&s),
            &mut rows,
            &mut id,
        );
    }
    rows
}

#[test]
fn diff_display_search() {
    let rows = make_rows();
    assert_eq!(
        dd::search_hit_row_ids(&rows, "g", true),
        (3..=9).collect::<Vec<_>>()
    );
    assert_eq!(dd::search_hit_row_ids(&rows, "g0", true), vec![3]);
    assert_eq!(dd::search_hit_row_ids(&rows, "new", true), vec![10]);
    assert_eq!(dd::search_hit_row_ids(&rows, "x", true), vec![11, 12, 13]);
    assert_eq!(
        dd::search_hit_row_ids(&rows, "z", true),
        Vec::<usize>::new()
    );
    assert_eq!(
        dd::search_hit_row_ids(&rows, "", false),
        Vec::<usize>::new()
    );
    let rows = vec![
        DiffRow {
            id: 0,
            kind: DiffRowKind::Context,
            left_number: Some(1),
            left_text: Some("Hello".into()),
            right_number: Some(1),
            right_text: Some("Hello".into()),
        },
        DiffRow {
            id: 1,
            kind: DiffRowKind::Context,
            left_number: Some(2),
            left_text: Some("world".into()),
            right_number: Some(2),
            right_text: Some("WORLD".into()),
        },
    ];
    assert_eq!(dd::search_hit_row_ids(&rows, "hello", false), vec![0]);
    assert_eq!(
        dd::search_hit_row_ids(&rows, "hello", true),
        Vec::<usize>::new()
    );
}

#[test]
fn diff_display_folding() {
    let rows = make_rows();
    let groups = dd::fold_groups(&rows, dd::COLLAPSE_THRESHOLD);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].count, 7);
    assert_eq!(groups[0].row_ids, (3..=9).collect::<Vec<_>>());
    assert_eq!(dd::fold_groups(&rows, 4).len(), 1);
    let small = dd::fold_groups(&rows, 2);
    assert_eq!(small.len(), 3);
    assert_eq!(
        small.iter().map(|g| g.count).collect::<Vec<_>>(),
        vec![2, 7, 3]
    );
    assert_eq!(dd::fold_groups(&rows, 8).len(), 0);
    let items = dd::display_items(&rows, &Default::default(), false, dd::COLLAPSE_THRESHOLD);
    assert_eq!(items.len(), 8);
    assert_eq!(
        items
            .iter()
            .filter(|i| matches!(i, dd::DisplayItem::Collapsed(_)))
            .count(),
        1
    );
    let expanded = dd::expanded_group_ids(&[5], &rows, dd::COLLAPSE_THRESHOLD);
    assert_eq!(expanded, [3].into_iter().collect());
    let items = dd::display_items(&rows, &expanded, false, dd::COLLAPSE_THRESHOLD);
    assert_eq!(items.len(), 14);
    let items = dd::display_items(&rows, &Default::default(), true, dd::COLLAPSE_THRESHOLD);
    assert_eq!(items.len(), 14);
    assert_eq!(dd::difference_row_ids(&rows), vec![2, 10]);
}

// ---------- TextFileLoader ----------
fn write_temp(data: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "diffloupe-tl-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&p, data).unwrap();
    p
}

#[test]
fn text_loader_encodings() {
    match text_loader::load(&write_temp("こんにちは\nworld\n".as_bytes())).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert!(t.contains("こんにちは"));
            assert_eq!(e, "UTF-8");
        }
        _ => panic!("text expected"),
    }
    match text_loader::load(&write_temp(&[0x41, 0x00, 0x42])).unwrap() {
        TextLoadResult::Binary => {}
        _ => panic!("binary expected"),
    }
    match text_loader::load(&write_temp(&[])).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(t, "");
            assert_eq!(e, "UTF-8");
        }
        _ => panic!("text expected"),
    }
    let sjis = encoding_rs::SHIFT_JIS
        .encode("こんにちは、世界\nシフトJISのテスト")
        .0;
    match text_loader::load(&write_temp(&sjis)).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(t, "こんにちは、世界\nシフトJISのテスト");
            assert_eq!(e, "Shift_JIS");
        }
        _ => panic!("sjis expected"),
    }
    let euc = encoding_rs::EUC_JP
        .encode("これはEUC-JPのテストです\n日本語文字列")
        .0;
    match text_loader::load(&write_temp(&euc)).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(e, "EUC-JP");
            assert!(t.contains("EUC-JPのテスト"));
        }
        _ => panic!("euc expected"),
    }
    let iso = encoding_rs::ISO_2022_JP
        .encode("日本語のISO-2022-JPテスト")
        .0;
    match text_loader::load(&write_temp(&iso)).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(e, "ISO-2022-JP");
            assert!(t.contains("ISO-2022-JPテスト"));
        }
        _ => panic!("iso2022jp expected"),
    }
    let mut u16le = vec![0xFF, 0xFE];
    for u in "UTF-16LEのBOM付きテスト\n日本語".encode_utf16() {
        u16le.extend_from_slice(&u.to_le_bytes());
    }
    match text_loader::load(&write_temp(&u16le)).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(t, "UTF-16LEのBOM付きテスト\n日本語");
            assert_eq!(e, "UTF-16 LE");
        }
        _ => panic!("utf16le expected"),
    }
    let bom = [
        vec![0xEF, 0xBB, 0xBF],
        "UTF-8のBOM付きテスト\n日本語".as_bytes().to_vec(),
    ]
    .concat();
    match text_loader::load(&write_temp(&bom)).unwrap() {
        TextLoadResult::Text(t, e) => {
            assert_eq!(t, "UTF-8のBOM付きテスト\n日本語");
            assert_eq!(e, "UTF-8 (BOM)");
        }
        _ => panic!("utf8bom expected"),
    }
    // PNG 相当のバイナリ
    match text_loader::load(&write_temp(&[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D,
    ]))
    .unwrap()
    {
        TextLoadResult::Binary => {}
        _ => panic!("binary expected"),
    }
}

// ---------- StructuredNormalizer ----------
fn norm_text(t: &str, url: &str, mode: SM) -> structured::StructuredNormalizationResult {
    structured::normalize(t.as_bytes(), Some(url), mode)
}

#[test]
fn struct_json() {
    let a = norm_text("{\"b\":2,\"a\":1}", "/tmp/x.json", SM::Json);
    let b = norm_text("{\n  \"a\" : 1,\n  \"b\" : 2\n}", "/tmp/y.json", SM::Json);
    assert!(a.did_normalize && b.did_normalize);
    assert_eq!(a.normalized_text, b.normalized_text);
    assert_eq!(
        a.normalized_text.as_deref(),
        Some("{\n  \"a\" : 1,\n  \"b\" : 2\n}")
    );
    let n = norm_text(
        "{\"c\":{\"y\":1,\"x\":2},\"a\":[3,1]}",
        "/tmp/x.json",
        SM::Json,
    );
    assert_eq!(n.normalized_text.as_deref(),
        Some("{\n  \"a\" : [\n    3,\n    1\n  ],\n  \"c\" : {\n    \"x\" : 2,\n    \"y\" : 1\n  }\n}"));
    assert_eq!(
        norm_text("42", "/tmp/n.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("42")
    );
    assert_eq!(
        norm_text("\"hi\"", "/tmp/s.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("\"hi\"")
    );
    assert_eq!(
        norm_text("true", "/tmp/b.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        norm_text("null", "/tmp/u.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("null")
    );
    let bad = norm_text("{bad json", "/tmp/x.json", SM::Json);
    assert!(!bad.did_normalize && bad.notice.is_some());
    // 精度: 近接 double を区別・往復保持
    let l = norm_text("{\"a\":0.1234567}", "/tmp/l.json", SM::Json);
    let r = norm_text("{\"a\":0.1234568}", "/tmp/r.json", SM::Json);
    assert_ne!(l.normalized_text, r.normalized_text);
    assert_eq!(
        norm_text("{\"a\":0.30000000000000004}", "/tmp/x.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("{\n  \"a\" : 0.30000000000000004\n}")
    );
    assert_eq!(
        norm_text("{\"a\":1.0}", "/tmp/x.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("{\n  \"a\" : 1\n}")
    );
    assert_eq!(
        norm_text("1.0", "/tmp/x.json", SM::Json)
            .normalized_text
            .as_deref(),
        Some("1")
    );
}

#[test]
fn struct_xml() {
    let l = norm_text("<a b=\"2\" a=\"1\"/>", "/tmp/l.xml", SM::Xml);
    let r = norm_text("<a a=\"1\" b=\"2\"/>", "/tmp/r.xml", SM::Xml);
    assert!(l.did_normalize && r.did_normalize);
    assert_eq!(l.normalized_text, r.normalized_text);
    assert_eq!(l.normalized_text.as_deref(), Some("<a a=\"1\" b=\"2\"/>\n"));
    let p = norm_text("<a>\n  <b/>\n  <c>x</c>\n</a>", "/tmp/l.xml", SM::Xml);
    let c = norm_text("<a><b/><c>x</c></a>", "/tmp/r.xml", SM::Xml);
    assert_eq!(p.normalized_text, c.normalized_text);
    assert_eq!(
        p.normalized_text.as_deref(),
        Some("<a>\n  <b/>\n  <c>x</c>\n</a>\n")
    );
    let m = norm_text(
        "<a><!-- note --><b value=\"v\">text</b></a>",
        "/tmp/x.xml",
        SM::Xml,
    );
    assert_eq!(
        m.normalized_text.as_deref(),
        Some("<a>\n  <!-- note -->\n  <b value=\"v\">text</b>\n</a>\n")
    );
    let mixed = norm_text("<p>Hello <b>world</b>!</p>", "/tmp/x.xml", SM::Xml);
    assert_eq!(
        mixed.normalized_text.as_deref(),
        Some("<p>\n  Hello \n  <b>world</b>\n  !\n</p>\n")
    );
    let bad = norm_text("<a><b></a>", "/tmp/x.xml", SM::Xml);
    assert!(!bad.did_normalize && bad.notice.is_some());
}

#[test]
fn struct_csv() {
    let q = norm_text("a,\"x,y\"\n", "/tmp/x.csv", SM::Csv);
    assert_eq!(q.normalized_text.as_deref(), Some("a x,y\n"));
    let e = norm_text("a,\"said \"\"hi\"\"\"\n", "/tmp/x.csv", SM::Csv);
    assert_eq!(
        e.normalized_text.as_deref(),
        Some("a \"said \"\"hi\"\"\"\n")
    );
    assert_eq!(
        norm_text("a,b\nc,d\n", "/tmp/x.csv", SM::Automatic)
            .normalized_text
            .as_deref(),
        Some("a b\nc d\n")
    );
    assert_eq!(
        norm_text("a\tb\nc\td\n", "/tmp/x.tsv", SM::Automatic)
            .normalized_text
            .as_deref(),
        Some("a b\nc d\n")
    );
    assert_eq!(
        norm_text("a;b\n", "/tmp/x.csv", SM::Automatic)
            .normalized_text
            .as_deref(),
        Some("a b\n")
    );
    assert_eq!(
        norm_text("\"a,b\"\n", "/tmp/x.csv", SM::Automatic)
            .normalized_text
            .as_deref(),
        Some("a,b\n")
    );
    let bad = norm_text("a,\"unterminated\n", "/tmp/x.csv", SM::Csv);
    assert!(!bad.did_normalize);
    assert_eq!(
        norm_text("a, b \n", "/tmp/l.csv", SM::Csv).normalized_text,
        norm_text("a,b\n", "/tmp/r.csv", SM::Csv).normalized_text
    );
    assert_eq!(structured::detect_csv_delimiter("a,b;c\n"), ',');
    assert_eq!(structured::detect_csv_delimiter("a\tb;c\n"), '\t');
    assert_eq!(structured::detect_csv_delimiter("a,b\tc\n"), ',');
    assert_eq!(structured::detect_csv_delimiter("no delimiter\n"), ',');
    let rows = structured::parse_csv("a,\"multi\nline\"\n", ',').unwrap();
    assert_eq!(rows, vec![vec!["a".to_string(), "multi\nline".to_string()]]);
    // UTF-8 BOM 除去
    let nobom = norm_text("a,b\n\"x,y\",z\n", "/tmp/n.csv", SM::Csv);
    let bomdata = [vec![0xEF, 0xBB, 0xBF], b"a,b\n\"x,y\",z\n".to_vec()].concat();
    let wb = structured::normalize(&bomdata, Some("/tmp/b.csv"), SM::Csv);
    assert_eq!(nobom.normalized_text, wb.normalized_text);
}

#[test]
fn struct_modes() {
    assert_eq!(
        norm_text("{\"a\":1}", "/tmp/x.json", SM::Automatic).format,
        Some(structured::StructuredFormat::Json)
    );
    assert_eq!(
        norm_text("<a/>", "/tmp/x.xml", SM::Automatic).format,
        Some(structured::StructuredFormat::Xml)
    );
    assert_eq!(
        norm_text("a\tb\n", "/tmp/x.tsv", SM::Automatic).format,
        Some(structured::StructuredFormat::Csv)
    );
    let u = norm_text("just text", "/tmp/x.txt", SM::Automatic);
    assert!(!u.did_normalize && u.format.is_none());
    let d = norm_text("{\"b\":2,\"a\":1}", "/tmp/x.json", SM::Disabled);
    assert!(!d.did_normalize && d.format.is_none());
    let x = norm_text("{\"a\":1}", "/tmp/x.txt", SM::Json);
    assert!(x.did_normalize);
    // サイズ上限
    let big = vec![0x20u8; structured::MAX_PARSE_BYTES + 1];
    let r = structured::normalize(&big, Some("/tmp/x.json"), SM::Json);
    assert!(!r.did_normalize && r.notice.is_some());
}

// ---------- HexDiff ----------
#[test]
fn hex_diff_basics() {
    let r = hex_diff::compare(&[0, 1, 2], &[0, 1, 2], hex_diff::DEFAULT_BYTE_CAP);
    assert!(r.is_equal());
    assert_eq!(r.row_count(), 1);
    assert!(r.row(0).differing_byte_offsets.is_empty());
    let r = hex_diff::compare(
        &[0x41, 0x42, 0x43],
        &[0x41, 0x58, 0x43],
        hex_diff::DEFAULT_BYTE_CAP,
    );
    assert!(!r.is_equal());
    assert_eq!(r.row(0).differing_byte_offsets, vec![1]);
    let r = hex_diff::compare(
        &[0x41, 0x42, 0x43, 0x44, 0x45],
        &[0x41, 0x42, 0x58, 0x43, 0x44, 0x45],
        hex_diff::DEFAULT_BYTE_CAP,
    );
    assert_eq!(r.row(0).differing_byte_offsets, vec![2, 3, 4]);
    let r = hex_diff::compare(
        &(0..20u8).collect::<Vec<_>>(),
        &(0..12u8).collect::<Vec<_>>(),
        hex_diff::DEFAULT_BYTE_CAP,
    );
    assert_eq!(r.row_count(), 2);
    assert_eq!(r.row(1).offset, 16);
    assert_eq!(r.row(1).left_bytes, Some(vec![16, 17, 18, 19]));
    assert_eq!(r.row(1).right_bytes, None);
    let r = hex_diff::compare(&[], &[], hex_diff::DEFAULT_BYTE_CAP);
    assert!(r.is_equal() && r.row_count() == 0 && !r.truncated);
    let mut changed = vec![0xABu8; 100];
    changed[50] = 0x01;
    let r = hex_diff::compare(&[0xABu8; 100], &changed, 64);
    assert!(r.truncated);
    assert_eq!(r.left_data.len(), 64);
    assert_eq!(r.row_count(), 4);
    assert_eq!(r.row(3).differing_byte_offsets, vec![2]);
    // cap 超えの差分は is_equal にならない
    let mut right = vec![0xABu8; 100];
    right[80] = 0x01;
    let r = hex_diff::compare(&[0xABu8; 100], &right, 64);
    assert!(r.truncated && !r.is_equal());
    assert_eq!(hex_diff::DEFAULT_BYTE_CAP, 8 * 1024 * 1024);
    assert_eq!(hex_diff::BYTES_PER_ROW, 16);
    assert_eq!(hex_diff::ascii(&[0x41, 0x00, 0x7E, 0x80]), "A.~.");
}

// ---------- FolderComparator ----------
#[test]
fn comparator_recursive_default() {
    let (base, l, r) = temp_pair("rec");
    write(&l, "sub/nested.txt", b"AAA");
    write(&r, "sub/nested.txt", b"BBB");
    let result = compare(&l, &r, &CompareOptions::default());
    assert!(result.entry_by_path("sub").unwrap().is_directory);
    assert_eq!(
        result.entry_by_path("sub/nested.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(result.summary.different_files, 1);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_non_recursive() {
    let (base, l, r) = temp_pair("nr");
    write(&l, "sub/nested.txt", b"AAA");
    write(&r, "sub/nested.txt", b"BBB");
    let opts = CompareOptions {
        recursive: false,
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert_eq!(
        result.entry_by_path("sub").unwrap().status,
        DiffStatus::Identical
    );
    assert!(result.entry_by_path("sub/nested.txt").is_none());
    assert_eq!(result.summary.total_files(), 0);
    write(&l, "top.txt", b"AAA");
    write(&r, "top.txt", b"BBB");
    write(&l, "only_left.txt", b"o");
    let result = compare(&l, &r, &opts);
    assert_eq!(
        result.entry_by_path("top.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        result.entry_by_path("only_left.txt").unwrap().status,
        DiffStatus::LeftOnly
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_statuses_and_metadata() {
    let (base, l, r) = temp_pair("st");
    write(&l, "same.txt", b"same");
    write(&r, "same.txt", b"same");
    write(&l, "diff.txt", b"aaa");
    write(&r, "diff.txt", b"bbb");
    write(&l, "only_l.txt", b"o");
    write(&r, "only_r.txt", b"o");
    write(&l, ".hidden.txt", b"h");
    write(&r, ".hidden.txt", b"x");
    let result = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        result.entry_by_path("same.txt").unwrap().status,
        DiffStatus::Identical
    );
    assert_eq!(
        result.entry_by_path("diff.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        result.entry_by_path("only_l.txt").unwrap().status,
        DiffStatus::LeftOnly
    );
    assert_eq!(
        result.entry_by_path("only_r.txt").unwrap().status,
        DiffStatus::RightOnly
    );
    // .hidden.txt は「ドット始まりだから除外」ではない
    assert_eq!(
        result.entry_by_path(".hidden.txt").unwrap().status,
        DiffStatus::Different
    );
    let e = result.entry_by_path("same.txt").unwrap();
    assert_eq!(e.left_size, 4);
    assert!(e.left_modification_date.is_some());
    assert!(e.left_posix.and_then(|p| p.permissions).is_some());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_default_excludes_and_filters() {
    let (base, l, r) = temp_pair("ex");
    write(&l, ".git/config", b"x");
    write(&l, "node_modules/pkg/index.js", b"x");
    write(&l, ".DS_Store", b"x");
    write(&l, "keep.txt", b"a");
    write(&r, "keep.txt", b"a");
    let result = compare(&l, &r, &CompareOptions::default());
    assert!(result.entry_by_path(".git").is_none());
    assert!(result.entry_by_path("node_modules").is_none());
    assert!(result.entry_by_path(".DS_Store").is_none());
    let result = compare(
        &l,
        &r,
        &CompareOptions {
            use_default_excludes: false,
            ..Default::default()
        },
    );
    assert!(result.entry_by_path(".git").is_some());
    // ユーザーフィルタ
    let opts = CompareOptions {
        user_exclude_patterns: vec!["*.txt".into()],
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert!(result.entry_by_path("keep.txt").is_none());
    let opts = CompareOptions {
        user_include_patterns: vec!["*.bak".into()],
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert!(result.entry_by_path("keep.txt").is_none());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_include_reaches_nested() {
    let (base, l, r) = temp_pair("inc");
    write(&l, "src/Engine/Foo.swift", b"AAA");
    write(&r, "src/Engine/Foo.swift", b"BBB");
    write(&l, "src/README.md", b"r");
    write(&l, "top.txt", b"t");
    let opts = CompareOptions {
        user_include_patterns: vec!["*.swift".into()],
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert!(result.entry_by_path("src").is_some());
    assert!(result.entry_by_path("src/Engine").is_some());
    assert_eq!(
        result.entry_by_path("src/Engine/Foo.swift").unwrap().status,
        DiffStatus::Different
    );
    assert!(result.entry_by_path("src/README.md").is_none());
    assert!(result.entry_by_path("top.txt").is_none());
    // 除外ディレクトリはインクルードより優先して刈られる
    write(&l, "build/tmp.swift", b"b");
    write(&l, "build/sub/deep.swift", b"c");
    let opts = CompareOptions {
        user_exclude_patterns: vec!["build".into()],
        user_include_patterns: vec!["**".into()],
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert!(result.entry_by_path("build").is_none());
    assert!(result.entry_by_path("src/Engine/Foo.swift").is_some());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_rename_detection() {
    let (base, l, r) = temp_pair("rn");
    write(&l, "old_name.txt", b"same content");
    write(&r, "new_name.txt", b"same content");
    let opts = CompareOptions {
        detect_renames: true,
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    let le = result.entry_by_path("old_name.txt").unwrap();
    assert_eq!(le.status, DiffStatus::LeftOnly);
    let rc = le.rename_candidate.as_ref().unwrap();
    assert_eq!(rc.counterpart_path, "new_name.txt");
    assert!(rc.is_unique_match);
    let re = result.entry_by_path("new_name.txt").unwrap();
    assert_eq!(
        re.rename_candidate.as_ref().unwrap().counterpart_path,
        "old_name.txt"
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_entry_limit_and_cancel() {
    let (base, l, r) = temp_pair("lim");
    for i in 0..5 {
        write(&l, &format!("f{}.txt", i), b"x");
        write(&r, &format!("f{}.txt", i), b"x");
    }
    let opts = CompareOptions {
        max_entries: Some(3),
        ..Default::default()
    };
    let res = FolderComparator::compare(&l, &r, &opts, None, None);
    assert!(matches!(
        res,
        Err(diffloupe_core::comparator::CompareError::EntryLimitExceeded(_))
    ));
    let token = CancellationToken::new();
    token.cancel();
    let res = FolderComparator::compare(&l, &r, &CompareOptions::default(), None, Some(&token));
    assert!(matches!(
        res,
        Err(diffloupe_core::comparator::CompareError::Cancelled)
    ));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_content_modes() {
    let (base, l, r) = temp_pair("cc");
    write(&l, "crlf.txt", b"a\r\nb\r\n");
    write(&r, "crlf.txt", b"a\nb\n");
    write(&l, "case.txt", b"HELLO\n");
    write(&r, "case.txt", b"hello\n");
    // 厳密では両方 different
    let result = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        result.entry_by_path("crlf.txt").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        result.entry_by_path("case.txt").unwrap().status,
        DiffStatus::Different
    );
    // ignore-line-endings で crlf 一致
    let opts = CompareOptions {
        content_comparison: ContentComparison {
            ignore_line_endings: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert_eq!(
        result.entry_by_path("crlf.txt").unwrap().status,
        DiffStatus::Identical
    );
    // ignore-case で case 一致
    let opts = CompareOptions {
        content_comparison: ContentComparison {
            ignore_case: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = compare(&l, &r, &opts);
    assert_eq!(
        result.entry_by_path("case.txt").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn comparator_structured_modes() {
    let (base, l, r) = temp_pair("sm");
    write(&l, "data.json", br#"{"b":2,"a":1}"#);
    write(&r, "data.json", b"{\n  \"a\" : 1,\n  \"b\" : 2\n}");
    // strict では different
    let result = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    for m in [SM::Json, SM::Automatic] {
        let opts = CompareOptions {
            content_comparison: ContentComparison {
                structured_mode: m,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = compare(&l, &r, &opts);
        assert_eq!(
            result.entry_by_path("data.json").unwrap().status,
            DiffStatus::Identical
        );
    }
    // 片側のみパース失敗 → 生比較と同じ
    write(&l, "bad.json", br#"{"a":1}"#);
    write(&r, "bad.json", b"{bad json");
    let opts = CompareOptions {
        content_comparison: ContentComparison {
            structured_mode: SM::Json,
            ..Default::default()
        },
        ..Default::default()
    };
    let structured = compare(&l, &r, &opts);
    let disabled = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        structured.entry_by_path("bad.json").unwrap().status,
        disabled.entry_by_path("bad.json").unwrap().status
    );
    assert_eq!(
        structured.entry_by_path("bad.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

// ---------- SyncPlan ----------
fn make_result_for_plan() -> (PathBuf, PathBuf, diffloupe_core::models::ComparisonResult) {
    let (base, l, r) = temp_pair("sp");
    write(&l, "same.txt", b"s");
    write(&r, "same.txt", b"s");
    write(&l, "only_l.txt", b"abc");
    write(&r, "only_r.txt", b"xy");
    write(&l, "diff.txt", b"a");
    write(&r, "diff.txt", b"b");
    write(&l, "only_dir/inner.txt", b"d");
    let result = compare(&l, &r, &CompareOptions::default());
    (base, l, result)
}

#[test]
fn sync_plan_left_to_right() {
    let (base, _l, result) = make_result_for_plan();
    let plan = SyncPlan::make(&result, SyncDirection::LeftToRight);
    assert_eq!(plan.direction, SyncDirection::LeftToRight);
    let by: std::collections::HashMap<_, _> = plan
        .items
        .iter()
        .map(|i| (i.relative_path.as_str(), i))
        .collect();
    assert_eq!(by["only_l.txt"].action, SyncAction::CopyFile);
    assert_eq!(by["only_l.txt"].bytes, 3);
    assert_eq!(by["only_r.txt"].action, SyncAction::PreservedDestination);
    assert_eq!(by["diff.txt"].action, SyncAction::Conflict);
    assert_eq!(by["only_dir"].action, SyncAction::CreateDirectory);
    assert_eq!(plan.summary.copy_files, 2); // only_l.txt + only_dir/inner.txt
    assert_eq!(plan.summary.preserved_destinations, 1);
    assert_eq!(plan.summary.conflicts, 1);
    assert_eq!(plan.summary.create_directories, 1);
    assert_eq!(plan.summary.total_bytes, 3 + 1);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn sync_plan_right_to_left() {
    let (base, _l, result) = make_result_for_plan();
    let plan = SyncPlan::make(&result, SyncDirection::RightToLeft);
    let by: std::collections::HashMap<_, _> = plan
        .items
        .iter()
        .map(|i| (i.relative_path.as_str(), i))
        .collect();
    assert_eq!(by["only_r.txt"].action, SyncAction::CopyFile);
    assert_eq!(by["only_l.txt"].action, SyncAction::PreservedDestination);
    std::fs::remove_dir_all(&base).ok();
}

// ---------- ThreeWayDiff ----------
#[test]
fn three_way_basic() {
    let cc = ContentComparison::default();
    let d = ThreeWayDiff::make("a\nb\nc\n", "a\nb\nc\n", "a\nb\nc\n", &cc);
    assert!(!d.truncated);
    assert!(d.lines.iter().all(|l| l.status == TWS::Unchanged));
    let d = ThreeWayDiff::make("a\nb\nc\n", "a\nX\nc\n", "a\nb\nc\n", &cc);
    assert!(d.lines.iter().any(|l| l.status == TWS::LeftChanged));
    let d = ThreeWayDiff::make("a\nb\nc\n", "a\nb\nc\n", "a\nY\nc\n", &cc);
    assert!(d.lines.iter().any(|l| l.status == TWS::RightChanged));
    let d = ThreeWayDiff::make("a\nb\nc\n", "a\nX\nc\n", "a\nX\nc\n", &cc);
    assert!(d.lines.iter().any(|l| l.status == TWS::BothChangedSame));
    let d = ThreeWayDiff::make("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n", &cc);
    assert!(d.lines.iter().any(|l| l.status == TWS::Conflict));
    // 行数上限
    let big = "l\n".repeat(three_way::MAX_TOTAL_LINES / 2 + 1);
    let d = ThreeWayDiff::make(&big, &big, &big, &cc);
    assert!(d.truncated);
    // 正規化判定・元表示保持
    let cc = ContentComparison {
        ignore_case: true,
        ..Default::default()
    };
    let d = ThreeWayDiff::make("a\nB\nc\n", "a\nb\nc\n", "a\nB\nc\n", &cc);
    assert!(d
        .lines
        .iter()
        .all(|l| l.status == TWS::Unchanged || l.status == TWS::LeftChanged));
}

// ---------- DirectoryWatcher ----------
#[test]
fn watcher_snapshot() {
    use diffloupe_core::watcher::DirectorySnapshot;
    let (base, l, _r) = temp_pair("w");
    write(&l, "a.txt", b"1");
    write(&l, "sub/b.txt", b"2");
    write(&l, ".git/config", b"x");
    let s1 = DirectorySnapshot::scan(&l, &[".git"]).unwrap();
    let s2 = DirectorySnapshot::scan(&l, &[".git"]).unwrap();
    assert_eq!(s1, s2);
    assert!(s1
        .entries
        .iter()
        .all(|e| !e.relative_path.starts_with(".git")));
    write(&l, "new.txt", b"n");
    let s3 = DirectorySnapshot::scan(&l, &[".git"]).unwrap();
    assert_ne!(s1, s3);
    let notdir = base.join("nope");
    assert!(DirectorySnapshot::scan(&notdir, &[]).is_err());
    std::fs::remove_dir_all(&base).ok();
}

// ---------- ComparisonReport ----------
#[test]
fn report_renderers() {
    use diffloupe_core::report::{ComparisonReport, ReportFormat};
    let (base, l, r) = temp_pair("rep");
    write(&l, "a.txt", b"1");
    write(&r, "a.txt", b"2");
    let result = compare(&l, &r, &CompareOptions::default());
    let report = ComparisonReport::new(&result, 1700000000);
    let json = report.render(ReportFormat::Json);
    assert!(json.contains("\"format\" : \"DiffLoupeComparisonReport\""));
    assert!(json.contains("\"version\" : 1"));
    assert!(json.contains("\"generatedAt\" : \"2023-11-14T22:13:20Z\""));
    let csv = report.render(ReportFormat::Csv);
    assert!(csv.contains("項目,値"));
    assert!(csv.contains("相対パス,状態"));
    let md = report.render(ReportFormat::Markdown);
    assert!(md.contains("# DiffLoupe比較レポート"));
    assert!(md.contains("## サマリ"));
    let html = report.render(ReportFormat::Html);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("DiffLoupe比較レポート"));
    assert!(html.contains("different"));
    std::fs::remove_dir_all(&base).ok();
}
