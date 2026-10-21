//! Swift `DiffLoupeTests/ComparisonReportTests.swift` の 1:1 移植(14件)

use diffloupe_core::models::{
    ComparedEntry, ComparisonResult, ComparisonSummary, DiffNode, DiffStatus, RenameCandidate,
};
use diffloupe_core::report::{iso8601_date, ComparisonReport, ReportFormat};
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
fn entry(
    path: &str,
    status: DiffStatus,
    is_directory: bool,
    left_size: i64,
    right_size: i64,
    left_date: Option<f64>,
    right_date: Option<f64>,
    error_message: Option<&str>,
    rename_candidate: Option<RenameCandidate>,
) -> ComparedEntry {
    ComparedEntry {
        relative_path: path.to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        is_directory,
        status,
        left_path: None,
        right_path: None,
        left_size,
        right_size,
        error_message: error_message.map(String::from),
        left_modification_date: left_date,
        right_modification_date: right_date,
        left_posix: None,
        right_posix: None,
        rename_candidate,
    }
}

fn make_result(entries: Vec<ComparedEntry>) -> ComparisonResult {
    let mut summary = ComparisonSummary::default();
    for e in &entries {
        if e.is_directory {
            continue;
        }
        match e.status {
            DiffStatus::Identical => summary.identical_files += 1,
            DiffStatus::Different => summary.different_files += 1,
            DiffStatus::LeftOnly => summary.left_only_files += 1,
            DiffStatus::RightOnly => summary.right_only_files += 1,
            DiffStatus::Error => summary.error_files += 1,
        }
    }
    ComparisonResult {
        left_root: "/tmp/left".into(),
        right_root: "/tmp/right".into(),
        entries,
        root_node: DiffNode {
            id: String::new(),
            name: String::new(),
            is_directory: true,
            status: DiffStatus::Different,
            children: None,
        },
        summary,
    }
}

fn report(entries: Vec<ComparedEntry>) -> (ComparisonResult, i64) {
    (make_result(entries), 1_700_000_000)
}

fn simple_entry(path: &str, status: DiffStatus) -> ComparedEntry {
    entry(path, status, false, 0, 0, None, None, None, None)
}

#[test]
fn test_json_structure() {
    let entries = vec![
        entry(
            "a.txt",
            DiffStatus::Identical,
            false,
            3,
            3,
            None,
            None,
            None,
            None,
        ),
        entry(
            "sub/b,1.txt",
            DiffStatus::Different,
            false,
            1,
            2,
            Some(1700000000.0),
            Some(1_700_000_001.0),
            Some("読み込みエラー"),
            None,
        ),
    ];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Json);
    let data: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(data["format"], "DiffLoupeComparisonReport");
    assert_eq!(data["version"], 1);
    assert_eq!(data["generatedAt"], "2023-11-14T22:13:20Z");
    assert_eq!(data["left"]["path"], "/tmp/left");
    assert!(data["left"]["gitRef"].is_null());
    assert_eq!(data["summary"]["identicalFiles"], 1);
    assert_eq!(data["summary"]["differentFiles"], 1);
    assert_eq!(data["summary"]["totalFiles"], 2);
    let entries_json = data["entries"].as_array().unwrap();
    assert_eq!(entries_json.len(), 2);
    let first = entries_json
        .iter()
        .find(|e| e["relativePath"] == "sub/b,1.txt")
        .unwrap();
    assert_eq!(first["status"], "different");
    assert_eq!(first["isDirectory"], false);
    assert_eq!(first["leftSize"], 1);
    assert_eq!(first["rightSize"], 2);
    assert_eq!(first["leftModificationDate"], "2023-11-14T22:13:20Z");
    assert_eq!(first["rightModificationDate"], "2023-11-14T22:13:21Z");
    assert_eq!(first["errorMessage"], "読み込みエラー");
    assert!(first["renameCandidate"].is_null());
}

#[test]
fn test_json_is_pretty_printed_and_deterministic() {
    let mk = || {
        vec![
            simple_entry("b.txt", DiffStatus::Different),
            simple_entry("a.txt", DiffStatus::Identical),
        ]
    };
    let (r1, at1) = report(mk());
    let first = ComparisonReport::new(&r1, at1).render(ReportFormat::Json);
    let (r2, at2) = report(mk());
    let second = ComparisonReport::new(&r2, at2).render(ReportFormat::Json);
    assert_eq!(first, second);
    assert!(first.contains('\n'));
}

#[test]
fn test_json_includes_rename_candidate() {
    let candidate = RenameCandidate {
        counterpart_path: "new.txt".into(),
        is_unique_match: false,
        total_matches: 3,
    };
    let entries = vec![entry(
        "old.txt",
        DiffStatus::LeftOnly,
        false,
        0,
        0,
        None,
        None,
        None,
        Some(candidate),
    )];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Json);
    let data: serde_json::Value = serde_json::from_str(&output).unwrap();
    let rename = &data["entries"][0]["renameCandidate"];
    assert_eq!(rename["counterpartPath"], "new.txt");
    assert_eq!(rename["isUniqueMatch"], false);
    assert_eq!(rename["totalMatches"], 3);
}

#[test]
fn test_json_includes_git_refs_when_provided() {
    let (r, at) = report(vec![]);
    let mut rep = ComparisonReport::new(&r, at);
    rep.left_git_ref = Some("main".into());
    rep.right_git_ref = Some("HEAD".into());
    let output = rep.render(ReportFormat::Json);
    let data: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(data["left"]["gitRef"], "main");
    assert_eq!(data["right"]["gitRef"], "HEAD");
}

#[test]
fn test_csv_row_count_and_header() {
    let entries = vec![
        simple_entry("a.txt", DiffStatus::Identical),
        simple_entry("b.txt", DiffStatus::Different),
    ];
    let n = entries.len();
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Csv);
    let lines: Vec<&str> = output.split('\n').collect();
    assert_eq!(lines.len(), 13 + n);
    assert!(lines[0].starts_with("項目,値"));
    assert_eq!(
        lines[12],
        "相対パス,状態,種別,左サイズ,右サイズ,左更新日時,右更新日時,リネーム候補,エラー"
    );
}

#[test]
fn test_csv_escapes_comma_quote_and_newline() {
    let entries = vec![entry(
        "a,\"b\".txt",
        DiffStatus::Different,
        false,
        0,
        0,
        None,
        None,
        Some("bad, \"quote\"\nline"),
        None,
    )];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Csv);
    assert!(output.contains("\"a,\"\"b\"\".txt\",相違,ファイル"));
    assert!(output.contains("\"bad, \"\"quote\"\"\nline\""));
}

#[test]
fn test_csv_quotes_only_when_needed() {
    let entries = vec![simple_entry("plain.txt", DiffStatus::Identical)];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Csv);
    assert!(output.contains("plain.txt,一致,ファイル"));
}

#[test]
fn test_markdown_row_count_and_escaping() {
    let entries = vec![
        entry(
            "a|b.txt",
            DiffStatus::Different,
            false,
            0,
            0,
            None,
            None,
            Some("x|y"),
            None,
        ),
        simple_entry("plain.txt", DiffStatus::Identical),
    ];
    let n = entries.len();
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Markdown);
    let lines: Vec<&str> = output.split('\n').collect();
    assert!(output.starts_with("# DiffLoupe比較レポート"));
    assert!(output.contains("## サマリ"));
    assert!(output.contains("## 詳細"));
    assert!(output.contains("a\\|b.txt"));
    assert!(output.contains("x\\|y"));
    let table_rows: Vec<_> = lines.iter().filter(|l| l.starts_with('|')).collect();
    assert_eq!(table_rows.len(), 10 + n);
}

#[test]
fn test_html_escapes_special_characters() {
    let entries = vec![entry(
        "a<b>&\"c'.txt",
        DiffStatus::Different,
        false,
        0,
        0,
        None,
        None,
        Some("& < > \" '"),
        None,
    )];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Html);
    assert!(output.contains("a&lt;b&gt;&amp;&quot;c&#39;.txt"));
    assert!(output.contains("&amp; &lt; &gt; &quot; &#39;"));
    assert!(!output.contains("a<b>&"));
}

#[test]
fn test_html_standalone_structure() {
    let entries = vec![simple_entry("a.txt", DiffStatus::Identical)];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Html);
    assert!(output.starts_with("<!DOCTYPE html>"));
    assert!(output.contains("<html"));
    assert!(output.contains("</html>"));
    assert!(output.contains("<meta charset=\"utf-8\">"));
    assert!(output.contains("<table>"));
    assert!(output.contains("相対パス"));
}

#[test]
fn test_html_row_class_for_status() {
    let entries = vec![
        simple_entry("a.txt", DiffStatus::Identical),
        simple_entry("b.txt", DiffStatus::Error),
        simple_entry("c.txt", DiffStatus::LeftOnly),
    ];
    let (r, at) = report(entries);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Html);
    assert!(output.contains("<tr class=\"identical\">"));
    assert!(output.contains("<tr class=\"error\">"));
    assert!(output.contains("<tr class=\"leftonly\">"));
}

#[test]
fn test_html_meta_includes_sides_and_refs() {
    let (r, at) = report(vec![]);
    let mut rep = ComparisonReport::new(&r, at);
    rep.left_git_ref = Some("main".into());
    rep.right_git_ref = None;
    let output = rep.render(ReportFormat::Html);
    assert!(output.contains("/tmp/left @ main"));
    assert!(output.contains("/tmp/right"));
    assert!(output.contains("2023-11-14T22:13:20Z"));
}

#[test]
fn test_generated_at_is_locale_independent_iso8601() {
    let (r, at) = report(vec![]);
    let output = ComparisonReport::new(&r, at).render(ReportFormat::Json);
    assert!(output.contains("2023-11-14T22:13:20Z"));
}

#[test]
fn test_report_formats_cover_all_cases() {
    let all = [
        ReportFormat::Html,
        ReportFormat::Csv,
        ReportFormat::Json,
        ReportFormat::Markdown,
    ];
    assert_eq!(all.len(), 4);
    assert_eq!(ReportFormat::Html.file_extension(), "html");
    assert_eq!(ReportFormat::Csv.file_extension(), "csv");
    assert_eq!(ReportFormat::Json.file_extension(), "json");
    assert_eq!(ReportFormat::Markdown.file_extension(), "md");
    let _ = PathBuf::new();
}

// ISO8601DateFormatter のミリ秒丸め規則(実機検証済み)のユニットテスト。
// parity mtime フィクスチャと合わせて小数秒の決定性をピン留めする。
#[test]
fn test_iso8601_date_ms_rounding_rule() {
    // round(t*1000) へ最近接丸め(同値は偶数)→ 秒へ floor
    assert_eq!(iso8601_date(0.0), "1970-01-01T00:00:00Z");
    assert_eq!(iso8601_date(1700000000.0), "2023-11-14T22:13:20Z");
    assert_eq!(iso8601_date(1700000000.4999), "2023-11-14T22:13:20Z");
    assert_eq!(iso8601_date(1700000000.5), "2023-11-14T22:13:20Z");
    assert_eq!(iso8601_date(1700000000.5001), "2023-11-14T22:13:20Z");
    assert_eq!(iso8601_date(1700000000.9994), "2023-11-14T22:13:20Z");
    assert_eq!(iso8601_date(1700000000.9995), "2023-11-14T22:13:21Z");
    assert_eq!(iso8601_date(1700000000.9999), "2023-11-14T22:13:21Z");
    // 負側・同値偶数丸め・単位跨ぎ
    assert_eq!(iso8601_date(-0.0004), "1970-01-01T00:00:00Z");
    assert_eq!(iso8601_date(-0.0005), "1970-01-01T00:00:00Z"); // -0.5ms → -0(偶数)
    assert_eq!(iso8601_date(-0.0006), "1969-12-31T23:59:59Z");
    assert_eq!(iso8601_date(-1.0005), "1969-12-31T23:59:59Z"); // -1000.5ms → -1000(偶数)
    assert_eq!(iso8601_date(-0.5), "1969-12-31T23:59:59Z"); // floor(-0.5)=-1
    assert_eq!(iso8601_date(59.9995), "1970-01-01T00:01:00Z");
    assert_eq!(iso8601_date(3599.9995), "1970-01-01T01:00:00Z");
    assert_eq!(iso8601_date(86399.9995), "1970-01-02T00:00:00Z");
}
