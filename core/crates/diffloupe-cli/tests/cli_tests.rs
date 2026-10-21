//! Swift `CLITests` の移植テスト。詳細は `core/tests/parity/TEST_MAP.md`。

#[path = "../src/cli.rs"]
mod cli;

use cli::*;
use diffloupe_core::report::ReportFormat;
use diffloupe_core::structured::StructuredComparisonMode as SM;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn parse_ok(v: &[&str]) -> CliOptions {
    match parse(&args(v)) {
        ParseResult::Options(o) => o,
        other => panic!(
            "parse failed: {:?}",
            match other {
                ParseResult::Error(e) => format!("{:?}", e),
                _ => "help/version".into(),
            }
        ),
    }
}

#[test]
fn parse_paths_and_defaults() {
    let o = parse_ok(&["l", "r"]);
    assert_eq!(o.left_path.as_deref(), Some("l"));
    assert_eq!(o.right_path.as_deref(), Some("r"));
    assert!(!o.recursive);
    assert!(o.use_default_excludes);
    assert_eq!(o.structured_mode, SM::Disabled);
    assert!(!o.detect_renames && !o.quiet && !o.enable_exit_code);
}

#[test]
fn parse_long_options() {
    let o = parse_ok(&[
        "--recursive",
        "--no-default-excludes",
        "--ignore-line-endings",
        "--ignore-trailing-whitespace",
        "--normalize-whitespace",
        "--ignore-case",
        "--detect-renames",
        "--format",
        "json",
        "--output",
        "out.json",
        "--quiet",
        "l",
        "r",
    ]);
    assert!(o.recursive && !o.use_default_excludes);
    assert!(o.content_comparison.ignore_line_endings);
    assert!(o.content_comparison.ignore_trailing_whitespace);
    assert!(o.content_comparison.normalize_whitespace);
    assert!(o.content_comparison.ignore_case);
    assert!(o.detect_renames && o.quiet);
    assert_eq!(o.report_format, Some(ReportFormat::Json));
    assert_eq!(o.output_path.as_deref(), Some("out.json"));
}

#[test]
fn parse_inline_values() {
    let o = parse_ok(&[
        "--format=json",
        "--output=out.json",
        "--exclude=*.bak",
        "--include=*.txt",
        "l",
        "r",
    ]);
    assert_eq!(o.report_format, Some(ReportFormat::Json));
    assert_eq!(o.output_path.as_deref(), Some("out.json"));
    assert_eq!(o.user_exclude_patterns, vec!["*.bak"]);
    assert_eq!(o.user_include_patterns, vec!["*.txt"]);
}

#[test]
fn parse_short_options() {
    let o = parse_ok(&[
        "-r", "-x", "-e", "*.o", "-i", "*.swift", "-f", "csv", "-o", "o.csv", "-q", "l", "r",
    ]);
    assert!(o.recursive && !o.use_default_excludes && o.quiet);
    assert_eq!(o.user_exclude_patterns, vec!["*.o"]);
    assert_eq!(o.user_include_patterns, vec!["*.swift"]);
    assert_eq!(o.report_format, Some(ReportFormat::Csv));
    assert_eq!(o.output_path.as_deref(), Some("o.csv"));
}

#[test]
fn parse_recursive_variants() {
    assert!(parse_ok(&["-r", "l", "r"]).recursive);
    assert!(parse_ok(&["--recursive", "l", "r"]).recursive);
    assert!(!parse_ok(&["-r", "--no-recursive", "l", "r"]).recursive);
    assert!(parse_ok(&["--no-recursive", "-r", "l", "r"]).recursive);
}

#[test]
fn parse_winmerge_slashes() {
    let o = parse_ok(&[
        "/r",
        "/u",
        "/ul",
        "/wm",
        "/minimize",
        "/noninteractive",
        "l",
        "r",
    ]);
    assert!(o.recursive);
    assert!(!parse_ok(&["/r-", "l", "r"]).recursive);
    assert!(parse_ok(&["/xq", "l", "r"]).quiet);
    assert!(parse_ok(&["/x", "l", "r"]).quiet);
    let o = parse_ok(&["/R", "l", "r"]);
    assert!(o.recursive);
    let o = parse_ok(&["/enableexitcode", "l", "r"]);
    assert!(o.enable_exit_code);
    let o = parse_ok(&["/dl", "左", "/dr", "右", "/dm", "中", "l", "r"]);
    assert_eq!(o.left_path.as_deref(), Some("l"));
    let o = parse_ok(&["/or", "out.xyz", "l", "r"]);
    assert_eq!(o.output_path.as_deref(), Some("out.xyz"));
    assert!(o.output_from_winmerge);
    // POSIX 絶対パスはスイッチではない
    let o = parse_ok(&["/tmp/left", "/tmp/right"]);
    assert_eq!(o.left_path.as_deref(), Some("/tmp/left"));
    assert_eq!(o.right_path.as_deref(), Some("/tmp/right"));
    // 未知スラッシュ引数は位置引数扱い
    let o = parse_ok(&["/unknown/foo", "r"]);
    assert_eq!(o.left_path.as_deref(), Some("/unknown/foo"));
}

#[test]
fn parse_double_dash() {
    let o = parse_ok(&["-r", "--", "-weird", "r"]);
    assert_eq!(o.left_path.as_deref(), Some("-weird"));
}

#[test]
fn parse_errors() {
    assert!(matches!(
        parse(&args(&[])),
        ParseResult::Error(ParseError::MissingPaths)
    ));
    assert!(matches!(
        parse(&args(&["l"])),
        ParseResult::Error(ParseError::MissingPaths)
    ));
    assert!(matches!(
        parse(&args(&["a", "b", "c"])),
        ParseResult::Error(ParseError::ExtraArguments(_))
    ));
    assert!(matches!(
        parse(&args(&["--bogus", "l", "r"])),
        ParseResult::Error(ParseError::UnknownOption(_))
    ));
    assert!(matches!(
        parse(&args(&["-z", "l", "r"])),
        ParseResult::Error(ParseError::UnknownOption(_))
    ));
    assert!(matches!(
        parse(&args(&["-ab", "l", "r"])),
        ParseResult::Error(ParseError::UnknownOption(_))
    ));
    assert!(matches!(
        parse(&args(&["-f"])),
        ParseResult::Error(ParseError::MissingValue(_))
    ));
    assert!(matches!(
        parse(&args(&["--format", "yaml", "l", "r"])),
        ParseResult::Error(ParseError::InvalidFormat(_))
    ));
    assert!(matches!(
        parse(&args(&["--structured", "yaml", "l", "r"])),
        ParseResult::Error(ParseError::InvalidStructuredMode(_))
    ));
    assert!(matches!(
        parse(&args(&["--preset", "bogus", "l", "r"])),
        ParseResult::Error(ParseError::UnknownPreset(_))
    ));
    assert!(matches!(
        parse(&args(&["-i"])),
        ParseResult::Error(ParseError::MissingValue(_))
    ));
}

#[test]
fn parse_help_version() {
    assert!(matches!(parse(&args(&["--help"])), ParseResult::Help));
    assert!(matches!(parse(&args(&["-h"])), ParseResult::Help));
    // パス未指定でも help が優先
    assert!(matches!(
        parse(&args(&["--help", "l", "r"])),
        ParseResult::Help
    ));
    assert!(matches!(parse(&args(&["--version"])), ParseResult::Version));
    assert!(matches!(parse(&args(&["-V"])), ParseResult::Version));
}

#[test]
fn parse_structured_modes() {
    for (raw, mode) in [
        ("off", SM::Disabled),
        ("disabled", SM::Disabled),
        ("auto", SM::Automatic),
        ("automatic", SM::Automatic),
        ("json", SM::Json),
        ("xml", SM::Xml),
        ("csv", SM::Csv),
    ] {
        assert_eq!(parse_structured_mode(raw), Some(mode));
    }
    assert_eq!(parse_structured_mode("JSON"), Some(SM::Json));
    assert_eq!(parse_structured_mode("yaml"), None);
    let o = parse_ok(&["--structured", "json", "l", "r"]);
    assert_eq!(o.structured_mode, SM::Json);
    let o = parse_ok(&["--normalize-json", "l", "r"]);
    assert_eq!(o.structured_mode, SM::Json);
    let o = parse_ok(&["--normalize-xml", "l", "r"]);
    assert_eq!(o.structured_mode, SM::Xml);
    let o = parse_ok(&["--table-csv", "l", "r"]);
    assert_eq!(o.structured_mode, SM::Csv);
}

#[test]
fn parse_format_fn() {
    assert_eq!(parse_format("json"), Some(ReportFormat::Json));
    assert_eq!(parse_format("csv"), Some(ReportFormat::Csv));
    assert_eq!(parse_format("md"), Some(ReportFormat::Markdown));
    assert_eq!(parse_format("markdown"), Some(ReportFormat::Markdown));
    assert_eq!(parse_format("html"), Some(ReportFormat::Html));
    assert_eq!(parse_format("JSON"), Some(ReportFormat::Json));
    assert_eq!(parse_format("xml"), None);
}

#[test]
fn report_target_cases() {
    let o = parse_ok(&["l", "r"]);
    assert_eq!(report_target(&o).unwrap(), ReportTarget::None);
    let o = parse_ok(&["-f", "json", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::Stdout(ReportFormat::Json)
    );
    let o = parse_ok(&["-o", "out.json", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::File("out.json".into(), ReportFormat::Json)
    );
    let o = parse_ok(&["-f", "csv", "-o", "out.anything", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::File("out.anything".into(), ReportFormat::Csv)
    );
    let o = parse_ok(&["-o", "out.md", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::File("out.md".into(), ReportFormat::Markdown)
    );
    let o = parse_ok(&["-o", "out.markdown", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::File("out.markdown".into(), ReportFormat::Markdown)
    );
    let o = parse_ok(&["-o", "out.xyz", "l", "r"]);
    assert!(matches!(
        report_target(&o),
        Err(ParseError::OutputExtensionUnknown(_))
    ));
    // /or 経由は拡張子不明時 HTML フォールバック
    let o = parse_ok(&["/or", "out.xyz", "l", "r"]);
    assert_eq!(
        report_target(&o).unwrap(),
        ReportTarget::File("out.xyz".into(), ReportFormat::Html)
    );
}

#[test]
fn exit_code_rules() {
    assert_eq!(winmerge_code(EXIT_SUCCESS), 0);
    assert_eq!(winmerge_code(EXIT_DIFFERENCES), 1);
    assert_eq!(winmerge_code(EXIT_USAGE_ERROR), 2);
    assert_eq!(winmerge_code(EXIT_FAILURE), 2);
    let all_ok = diffloupe_core::models::ComparisonSummary {
        identical_files: 3,
        different_files: 0,
        left_only_files: 0,
        right_only_files: 0,
        error_files: 0,
    };
    assert_eq!(exit_for_summary(&all_ok), EXIT_SUCCESS);
    let empty = diffloupe_core::models::ComparisonSummary::default();
    assert_eq!(exit_for_summary(&empty), EXIT_SUCCESS);
    for (d, l, r, e) in [
        (1, 0, 0, 0),
        (0, 1, 0, 0),
        (0, 0, 1, 0),
        (0, 0, 0, 1),
        (1, 1, 1, 1),
    ] {
        let s = diffloupe_core::models::ComparisonSummary {
            identical_files: 0,
            different_files: d,
            left_only_files: l,
            right_only_files: r,
            error_files: e,
        };
        assert_eq!(exit_for_summary(&s), EXIT_DIFFERENCES);
    }
}

#[test]
fn run_end_to_end() {
    let dir = std::env::temp_dir().join(format!("dlcli-{}", std::process::id()));
    let l = dir.join("l");
    let r = dir.join("r");
    std::fs::create_dir_all(&l).unwrap();
    std::fs::create_dir_all(&r).unwrap();
    std::fs::write(l.join("same.txt"), b"s").unwrap();
    std::fs::write(r.join("same.txt"), b"s").unwrap();
    std::fs::write(l.join("diff.txt"), b"a").unwrap();
    std::fs::write(r.join("diff.txt"), b"b").unwrap();
    let token = diffloupe_core::cancel::CancellationToken::new();
    let a = args(&["-q", l.to_str().unwrap(), r.to_str().unwrap()]);
    assert_eq!(run(&a, &token), EXIT_DIFFERENCES);
    let a = args(&["-q", l.to_str().unwrap(), l.to_str().unwrap()]);
    assert_eq!(run(&a, &token), EXIT_SUCCESS);
    let a = args(&[
        "-q",
        "/enableexitcode",
        l.to_str().unwrap(),
        r.to_str().unwrap(),
    ]);
    assert_eq!(run(&a, &token), 1);
    let a = args(&["-q", "/enableexitcode", l.to_str().unwrap(), "nonexistent"]);
    assert_eq!(run(&a, &token), 2);
    // 構造化モードで JSON キー順を正規化
    std::fs::write(l.join("d.json"), br#"{"b":2,"a":1}"#).unwrap();
    std::fs::write(r.join("d.json"), br#"{"a":1,"b":2}"#).unwrap();
    let a = args(&[
        "-q",
        "--structured",
        "json",
        l.to_str().unwrap(),
        r.to_str().unwrap(),
    ]);
    assert_eq!(run(&a, &token), EXIT_DIFFERENCES); // diff.txt が残る
    std::fs::remove_file(l.join("diff.txt")).unwrap();
    std::fs::remove_file(r.join("diff.txt")).unwrap();
    let a = args(&[
        "-q",
        "--structured",
        "json",
        l.to_str().unwrap(),
        r.to_str().unwrap(),
    ]);
    assert_eq!(run(&a, &token), EXIT_SUCCESS);
    let a = args(&["-q", l.to_str().unwrap(), r.to_str().unwrap()]);
    assert_eq!(run(&a, &token), EXIT_DIFFERENCES);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn comparator_options_propagates() {
    let o = parse_ok(&[
        "-r",
        "-x",
        "-e",
        "*.bak",
        "-i",
        "*.swift",
        "--preset",
        "source-control",
        "--structured",
        "json",
        "--detect-renames",
        "l",
        "r",
    ]);
    let c = comparator_options(&o);
    assert!(c.recursive && !c.use_default_excludes);
    assert!(c.user_exclude_patterns.contains(&"*.bak".to_string()));
    assert!(c.user_exclude_patterns.contains(&".git".to_string()));
    assert_eq!(c.user_include_patterns, vec!["*.swift"]);
    assert_eq!(c.content_comparison.structured_mode, SM::Json);
    assert!(c.detect_renames);
}
