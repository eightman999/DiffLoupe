//! Swift `CLITests` の 1:1 移植
//! (CLIOptionsTests / CLIRunnerTests / ExitCodeTests / FilterCLITests / ReportOutputTests)

#[path = "../src/cli.rs"]
mod cli;

use cli::*;
use diffloupe_core::cancel::CancellationToken;
use diffloupe_core::models::{
    ComparedEntry, ComparisonResult, ComparisonSummary, DiffNode, DiffStatus,
};
use diffloupe_core::report::{ComparisonReport, ReportFormat};
use diffloupe_core::structured::StructuredComparisonMode as SM;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn parse_ok(v: &[&str]) -> CliOptions {
    match parse(&args(v)) {
        ParseResult::Options(o) => o,
        other => panic!("parse failed: {:?}", other),
    }
}

fn is_error(result: ParseResult, expect: &ParseError) -> bool {
    match result {
        ParseResult::Error(e) => &e == expect,
        _ => false,
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "swift-port-cli-{}-{}-{}",
        tag,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_file(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// Swift `CLIOptionsTests.swift` の 1:1 移植(32件)
mod cli_options_tests {
    use super::*;

    #[test]
    fn test_only_paths_use_defaults() {
        let o = parse_ok(&["left/", "right/"]);
        assert_eq!(o.left_path.as_deref(), Some("left/"));
        assert_eq!(o.right_path.as_deref(), Some("right/"));
        assert!(!o.recursive);
        assert!(o.use_default_excludes);
        assert!(o.user_exclude_patterns.is_empty());
        assert!(o.content_comparison.is_strict());
        assert!(!o.detect_renames);
        assert!(o.report_format.is_none());
        assert!(o.output_path.is_none());
        assert!(!o.enable_exit_code);
        assert!(!o.quiet);
    }

    #[test]
    fn test_long_options() {
        let o = parse_ok(&[
            "--no-default-excludes",
            "--detect-renames",
            "--ignore-line-endings",
            "--ignore-trailing-whitespace",
            "--normalize-whitespace",
            "--ignore-case",
            "--quiet",
            "a",
            "b",
        ]);
        assert!(!o.use_default_excludes);
        assert!(o.detect_renames);
        assert!(o.content_comparison.ignore_line_endings);
        assert!(o.content_comparison.ignore_trailing_whitespace);
        assert!(o.content_comparison.normalize_whitespace);
        assert!(o.content_comparison.ignore_case);
        assert!(!o.content_comparison.is_strict());
        assert!(o.quiet);
    }

    #[test]
    fn test_short_options_and_inline_values() {
        let o = parse_ok(&[
            "-x",
            "-e",
            "*.tmp",
            "--exclude=build/",
            "-e",
            "*.log",
            "-f",
            "json",
            "-o",
            "/tmp/report.json",
            "-q",
            "a",
            "b",
        ]);
        assert!(!o.use_default_excludes);
        assert_eq!(o.user_exclude_patterns, ["*.tmp", "build/", "*.log"]);
        assert_eq!(o.report_format, Some(ReportFormat::Json));
        assert_eq!(o.output_path.as_deref(), Some("/tmp/report.json"));
        assert!(o.quiet);
    }

    #[test]
    fn test_recursive_short_and_long_are_accepted() {
        assert!(parse_ok(&["-r", "a", "b"]).recursive);
        assert!(parse_ok(&["--recursive", "a", "b"]).recursive);
    }

    #[test]
    fn test_no_recursive_disables_recursion() {
        assert!(!parse_ok(&["-r", "--no-recursive", "a", "b"]).recursive);
        assert!(!parse_ok(&["--no-recursive", "a", "b"]).recursive);
    }

    #[test]
    fn test_win_merge_slash_switches() {
        assert!(parse_ok(&["/r", "a", "b"]).recursive);
        assert!(!parse_ok(&["/r-", "a", "b"]).recursive);
        assert!(!parse_ok(&["/r", "/r-", "a", "b"]).recursive);
    }

    #[test]
    fn test_win_merge_slash_switch_case_insensitive() {
        assert!(parse_ok(&["/R", "a", "b"]).recursive);
        assert!(parse_ok(&["/ENABLEEXITCODE", "a", "b"]).enable_exit_code);
    }

    #[test]
    fn test_win_merge_no_op_and_quiet_switches() {
        for flag in [
            "/u",
            "/ul",
            "/um",
            "/ur",
            "/wl",
            "/wm",
            "/wr",
            "/minimize",
            "/maximize",
            "/noninteractive",
        ] {
            let o = parse_ok(&[flag, "a", "b"]);
            assert!(!o.quiet, "{} はquietを変更しないこと", flag);
            assert!(!o.recursive, "{} はrecursiveを変更しないこと", flag);
        }
        for flag in ["/x", "/xq"] {
            assert!(parse_ok(&[flag, "a", "b"]).quiet);
        }
    }

    #[test]
    fn test_win_merge_label_switches_consume_value() {
        let o = parse_ok(&["/dl", "左", "/dm", "中", "/dr", "右", "a", "b"]);
        assert_eq!(o.left_path.as_deref(), Some("a"));
        assert_eq!(o.right_path.as_deref(), Some("b"));
        assert!(is_error(
            parse(&args(&["/dl"])),
            &ParseError::MissingValue("/dl".into())
        ));
    }

    #[test]
    fn test_enable_exit_code_switch() {
        assert!(parse_ok(&["/enableexitcode", "a", "b"]).enable_exit_code);
    }

    #[test]
    fn test_or_switch_sets_output_path_and_inference() {
        let o = parse_ok(&["/or", "/tmp/report.csv", "a", "b"]);
        assert_eq!(o.output_path.as_deref(), Some("/tmp/report.csv"));
        assert!(o.output_from_winmerge);
        assert_eq!(
            report_target(&o).unwrap(),
            ReportTarget::File("/tmp/report.csv".into(), ReportFormat::Csv)
        );
        let o2 = parse_ok(&["/or", "/tmp/report.txt", "a", "b"]);
        assert_eq!(
            report_target(&o2).unwrap(),
            ReportTarget::File("/tmp/report.txt".into(), ReportFormat::Html)
        );
    }

    #[test]
    fn test_absolute_paths_are_not_confused_with_slash_switches() {
        let o = parse_ok(&["/tmp/left", "/tmp/right"]);
        assert_eq!(o.left_path.as_deref(), Some("/tmp/left"));
        assert_eq!(o.right_path.as_deref(), Some("/tmp/right"));
        assert!(!o.recursive);
    }

    #[test]
    fn test_unknown_slash_argument_is_not_a_switch() {
        let o = parse_ok(&["/somewhere", "/elsewhere"]);
        assert_eq!(o.left_path.as_deref(), Some("/somewhere"));
        assert_eq!(o.right_path.as_deref(), Some("/elsewhere"));
    }

    #[test]
    fn test_double_dash_terminates_options() {
        let o = parse_ok(&["--", "-weird", "dir"]);
        assert_eq!(o.left_path.as_deref(), Some("-weird"));
        assert_eq!(o.right_path.as_deref(), Some("dir"));
    }

    #[test]
    fn test_missing_paths_is_error() {
        assert!(is_error(parse(&args(&[])), &ParseError::MissingPaths));
        assert!(is_error(
            parse(&args(&["only-left"])),
            &ParseError::MissingPaths
        ));
    }

    #[test]
    fn test_extra_arguments_is_error() {
        assert!(is_error(
            parse(&args(&["a", "b", "c", "d"])),
            &ParseError::ExtraArguments(vec!["c".into(), "d".into()])
        ));
    }

    #[test]
    fn test_unknown_option_is_error() {
        assert!(is_error(
            parse(&args(&["--nope", "a", "b"])),
            &ParseError::UnknownOption("--nope".into())
        ));
        assert!(is_error(
            parse(&args(&["-z", "a", "b"])),
            &ParseError::UnknownOption("-z".into())
        ));
    }

    #[test]
    fn test_missing_value_is_error() {
        assert!(is_error(
            parse(&args(&["--exclude"])),
            &ParseError::MissingValue("--exclude".into())
        ));
        assert!(is_error(
            parse(&args(&["-e"])),
            &ParseError::MissingValue("-e".into())
        ));
        assert!(is_error(
            parse(&args(&["a", "b", "--format"])),
            &ParseError::MissingValue("--format".into())
        ));
        assert!(is_error(
            parse(&args(&["a", "b", "--structured"])),
            &ParseError::MissingValue("--structured".into())
        ));
    }

    #[test]
    fn test_invalid_format_is_error() {
        assert!(is_error(
            parse(&args(&["-f", "yaml", "a", "b"])),
            &ParseError::InvalidFormat("yaml".into())
        ));
        assert!(is_error(
            parse(&args(&["--format=pdf", "a", "b"])),
            &ParseError::InvalidFormat("pdf".into())
        ));
    }

    #[test]
    fn test_structured_mode_default_disabled() {
        assert_eq!(parse_ok(&["a", "b"]).structured_mode, SM::Disabled);
    }

    #[test]
    fn test_structured_mode_parsing() {
        for (flag, expected) in [
            ("off", SM::Disabled),
            ("disabled", SM::Disabled),
            ("auto", SM::Automatic),
            ("automatic", SM::Automatic),
            ("json", SM::Json),
            ("xml", SM::Xml),
            ("csv", SM::Csv),
        ] {
            let o = parse_ok(&["--structured", flag, "a", "b"]);
            assert_eq!(o.structured_mode, expected, "--structured {}", flag);
        }
        assert_eq!(
            parse_ok(&["--structured", "JSON", "a", "b"]).structured_mode,
            SM::Json
        );
        assert_eq!(
            parse_ok(&["--structured=auto", "a", "b"]).structured_mode,
            SM::Automatic
        );
    }

    #[test]
    fn test_structured_mode_aliases() {
        assert_eq!(
            parse_ok(&["--normalize-json", "a", "b"]).structured_mode,
            SM::Json
        );
        assert_eq!(
            parse_ok(&["--normalize-xml", "a", "b"]).structured_mode,
            SM::Xml
        );
        assert_eq!(
            parse_ok(&["--table-csv", "a", "b"]).structured_mode,
            SM::Csv
        );
    }

    #[test]
    fn test_invalid_structured_mode_is_error() {
        assert!(is_error(
            parse(&args(&["--structured", "yaml", "a", "b"])),
            &ParseError::InvalidStructuredMode("yaml".into())
        ));
        assert!(is_error(
            parse(&args(&["--structured=bogus", "a", "b"])),
            &ParseError::InvalidStructuredMode("bogus".into())
        ));
    }

    #[test]
    fn test_parse_structured_mode() {
        assert_eq!(parse_structured_mode("off"), Some(SM::Disabled));
        assert_eq!(parse_structured_mode("AUTO"), Some(SM::Automatic));
        assert_eq!(parse_structured_mode("json"), Some(SM::Json));
        assert_eq!(parse_structured_mode("Xml"), Some(SM::Xml));
        assert_eq!(parse_structured_mode("csv"), Some(SM::Csv));
        assert_eq!(parse_structured_mode("yaml"), None);
    }

    #[test]
    fn test_help_and_version_precede_path_validation() {
        assert!(matches!(parse(&args(&["-h"])), ParseResult::Help));
        assert!(matches!(
            parse(&args(&["--help", "a", "b"])),
            ParseResult::Help
        ));
        assert!(matches!(parse(&args(&["-V"])), ParseResult::Version));
        assert!(matches!(parse(&args(&["--version"])), ParseResult::Version));
    }

    #[test]
    fn test_parse_format() {
        assert_eq!(parse_format("json"), Some(ReportFormat::Json));
        assert_eq!(parse_format("JSON"), Some(ReportFormat::Json));
        assert_eq!(parse_format("csv"), Some(ReportFormat::Csv));
        assert_eq!(parse_format("md"), Some(ReportFormat::Markdown));
        assert_eq!(parse_format("markdown"), Some(ReportFormat::Markdown));
        assert_eq!(parse_format("html"), Some(ReportFormat::Html));
        assert_eq!(parse_format("txt"), None);
    }

    #[test]
    fn test_format_for_output_path() {
        assert_eq!(format_for_output_path("r.json"), Some(ReportFormat::Json));
        assert_eq!(
            format_for_output_path("report.CSV"),
            Some(ReportFormat::Csv)
        );
        assert_eq!(format_for_output_path("r.md"), Some(ReportFormat::Markdown));
        assert_eq!(
            format_for_output_path("r.markdown"),
            Some(ReportFormat::Markdown)
        );
        assert_eq!(format_for_output_path("r.html"), Some(ReportFormat::Html));
        assert_eq!(format_for_output_path("r.txt"), None);
    }

    #[test]
    fn test_report_target_none_when_unspecified() {
        let o = parse_ok(&["a", "b"]);
        assert_eq!(report_target(&o).unwrap(), ReportTarget::None);
    }

    #[test]
    fn test_report_target_stdout_when_format_only() {
        let mut o = parse_ok(&["a", "b"]);
        o.report_format = Some(ReportFormat::Json);
        assert_eq!(
            report_target(&o).unwrap(),
            ReportTarget::Stdout(ReportFormat::Json)
        );
    }

    #[test]
    fn test_report_target_file_inferring_format_from_extension() {
        let mut o = parse_ok(&["a", "b"]);
        o.output_path = Some("/tmp/report.csv".into());
        assert_eq!(
            report_target(&o).unwrap(),
            ReportTarget::File("/tmp/report.csv".into(), ReportFormat::Csv)
        );
    }

    #[test]
    fn test_report_target_file_with_explicit_format() {
        let mut o = parse_ok(&["a", "b"]);
        o.output_path = Some("/tmp/report.csv".into());
        o.report_format = Some(ReportFormat::Markdown);
        assert_eq!(
            report_target(&o).unwrap(),
            ReportTarget::File("/tmp/report.csv".into(), ReportFormat::Markdown)
        );
    }

    #[test]
    fn test_report_target_rejects_unknown_extension() {
        let mut o = parse_ok(&["a", "b"]);
        o.output_path = Some("/tmp/report.txt".into());
        assert_eq!(
            report_target(&o),
            Err(ParseError::OutputExtensionUnknown("/tmp/report.txt".into()))
        );
    }
}

/// Swift `CLIRunnerTests.swift` の 1:1 移植(13件)
mod cli_runner_tests {
    use super::*;

    fn run(v: &[&str]) -> i32 {
        cli::run(&args(v), &CancellationToken::new())
    }

    #[test]
    fn test_equal_trees_return_success() {
        let base = temp_dir("cr1");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "same");
        write_file(&r, "a.txt", "same");
        assert_eq!(run(&["-q", l.to_str().unwrap(), r.to_str().unwrap()]), 0);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_different_files_return_differences() {
        let base = temp_dir("cr2");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "AAA");
        write_file(&r, "a.txt", "BBB");
        assert_eq!(run(&["-q", l.to_str().unwrap(), r.to_str().unwrap()]), 1);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_structured_json_key_order_treated_as_identical() {
        let base = temp_dir("cr3");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "data.json", "{\"b\":2,\"a\":1}");
        write_file(&r, "data.json", "{\"a\":1,\"b\":2}");
        let (ls, rs) = (l.to_str().unwrap(), r.to_str().unwrap());
        assert_eq!(run(&["-q", ls, rs]), 1);
        assert_eq!(run(&["-q", "--structured", "json", ls, rs]), 0);
        assert_eq!(run(&["-q", "--normalize-json", ls, rs]), 0);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_structured_xml_attribute_order_treated_as_identical() {
        let base = temp_dir("cr4");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "data.xml", "<a b=\"2\" a=\"1\"/>");
        write_file(&r, "data.xml", "<a a=\"1\" b=\"2\"/>");
        let (ls, rs) = (l.to_str().unwrap(), r.to_str().unwrap());
        assert_eq!(run(&["-q", ls, rs]), 1);
        assert_eq!(run(&["-q", "--structured", "xml", ls, rs]), 0);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_structured_csv_quoting_treated_as_identical() {
        let base = temp_dir("cr5");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "data.csv", "a,b\n\"x,y\",z\n");
        write_file(&r, "data.csv", "a,b\n\"x,y\",z\n");
        let (ls, rs) = (l.to_str().unwrap(), r.to_str().unwrap());
        assert_eq!(run(&["-q", ls, rs]), 0);
        assert_eq!(run(&["-q", "--structured", "csv", ls, rs]), 0);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_structured_mode_still_detects_real_difference() {
        let base = temp_dir("cr6");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "data.json", "{\"a\":1,\"b\":2}");
        write_file(&r, "data.json", "{\"a\":1,\"b\":99}");
        assert_eq!(
            run(&[
                "-q",
                "--structured",
                "json",
                l.to_str().unwrap(),
                r.to_str().unwrap()
            ]),
            1
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_structured_auto_mode_uses_extension() {
        let base = temp_dir("cr7");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "data.json", "{\"b\":2,\"a\":1}");
        write_file(&r, "data.json", "{\"a\":1,\"b\":2}");
        assert_eq!(
            run(&[
                "-q",
                "--structured",
                "auto",
                l.to_str().unwrap(),
                r.to_str().unwrap()
            ]),
            0
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_non_recursive_default_ignores_nested_difference() {
        let base = temp_dir("cr8");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "sub/nested.txt", "AAA");
        write_file(&r, "sub/nested.txt", "BBB");
        let (ls, rs) = (l.to_str().unwrap(), r.to_str().unwrap());
        assert_eq!(run(&["-q", ls, rs]), 0);
        assert_eq!(run(&["-q", "-r", ls, rs]), 1);
        assert_eq!(run(&["-q", "/r", ls, rs]), 1);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_non_directory_argument_returns_usage_error() {
        let base = temp_dir("cr9");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "file");
        let file = l.join("a.txt");
        assert_eq!(run(&["-q", file.to_str().unwrap(), r.to_str().unwrap()]), 2);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_or_report_file_created_and_overwrite_refused() {
        let base = temp_dir("cr10");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "AAA");
        write_file(&r, "a.txt", "BBB");
        let report = base.join("report.html");
        let rp = report.to_str().unwrap().to_string();
        let (ls, rs) = (l.to_str().unwrap(), r.to_str().unwrap());
        assert_eq!(run(&["-q", "/or", &rp, ls, rs]), 1);
        assert!(report.exists());
        assert_eq!(run(&["-q", "/or", &rp, ls, rs]), 2);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_slash_or_with_unknown_extension_falls_back_to_html() {
        let base = temp_dir("cr11");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "AAA");
        write_file(&r, "a.txt", "BBB");
        let report = base.join("report.txt");
        assert_eq!(
            run(&[
                "-q",
                "/or",
                report.to_str().unwrap(),
                l.to_str().unwrap(),
                r.to_str().unwrap()
            ]),
            1
        );
        assert!(report.exists());
        let text = std::fs::read_to_string(&report).unwrap();
        assert!(text.contains("<!DOCTYPE html>"));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_enable_exit_code_accepts_and_keeps_differences_mapping() {
        let base = temp_dir("cr12");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "AAA");
        write_file(&r, "a.txt", "BBB");
        assert_eq!(
            run(&[
                "-q",
                "/enableexitcode",
                l.to_str().unwrap(),
                r.to_str().unwrap()
            ]),
            1
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_enable_exit_code_maps_usage_error_to_winmerge_2() {
        let base = temp_dir("cr13");
        let (l, r) = (base.join("left"), base.join("right"));
        write_file(&l, "a.txt", "file");
        let file = l.join("a.txt");
        assert_eq!(
            run(&[
                "-q",
                "/enableexitcode",
                file.to_str().unwrap(),
                r.to_str().unwrap()
            ]),
            2
        );
        std::fs::remove_dir_all(&base).ok();
    }
}

/// Swift `ExitCodeTests.swift` の 1:1 移植(9件)
mod exit_code_tests {
    use super::*;

    fn summary(
        identical: usize,
        different: usize,
        left_only: usize,
        right_only: usize,
        error: usize,
    ) -> ComparisonSummary {
        ComparisonSummary {
            identical_files: identical,
            different_files: different,
            left_only_files: left_only,
            right_only_files: right_only,
            error_files: error,
        }
    }

    #[test]
    fn test_constants() {
        assert_eq!(cli::EXIT_SUCCESS, 0);
        assert_eq!(cli::EXIT_DIFFERENCES, 1);
        assert_eq!(cli::EXIT_USAGE_ERROR, 2);
        assert_eq!(cli::EXIT_FAILURE, 3);
    }

    #[test]
    fn test_all_identical_returns_success() {
        assert_eq!(exit_for_summary(&summary(5, 0, 0, 0, 0)), cli::EXIT_SUCCESS);
    }

    #[test]
    fn test_empty_tree_returns_success() {
        assert_eq!(
            exit_for_summary(&ComparisonSummary::default()),
            cli::EXIT_SUCCESS
        );
    }

    #[test]
    fn test_different_files_returns_differences() {
        assert_eq!(
            exit_for_summary(&summary(2, 1, 0, 0, 0)),
            cli::EXIT_DIFFERENCES
        );
    }

    #[test]
    fn test_left_only_returns_differences() {
        assert_eq!(
            exit_for_summary(&summary(0, 0, 1, 0, 0)),
            cli::EXIT_DIFFERENCES
        );
    }

    #[test]
    fn test_right_only_returns_differences() {
        assert_eq!(
            exit_for_summary(&summary(0, 0, 0, 3, 0)),
            cli::EXIT_DIFFERENCES
        );
    }

    #[test]
    fn test_error_files_returns_differences() {
        assert_eq!(
            exit_for_summary(&summary(0, 0, 0, 0, 2)),
            cli::EXIT_DIFFERENCES
        );
    }

    #[test]
    fn test_mixed_returns_differences() {
        assert_eq!(
            exit_for_summary(&summary(10, 1, 1, 1, 1)),
            cli::EXIT_DIFFERENCES
        );
    }

    #[test]
    fn test_win_merge_code_mapping() {
        assert_eq!(winmerge_code(cli::EXIT_SUCCESS), 0);
        assert_eq!(winmerge_code(cli::EXIT_DIFFERENCES), 1);
        assert_eq!(winmerge_code(cli::EXIT_USAGE_ERROR), 2);
        assert_eq!(winmerge_code(cli::EXIT_FAILURE), 2);
    }
}

/// Swift `FilterCLITests.swift` の 1:1 移植(12件)
mod filter_cli_tests {
    use super::*;

    #[test]
    fn test_defaults_are_empty() {
        let o = parse_ok(&["a", "b"]);
        assert!(o.user_include_patterns.is_empty());
        assert!(o.filter_presets.is_empty());
    }

    #[test]
    fn test_short_include_is_repeatable() {
        let o = parse_ok(&["-i", "*.swift", "-i", "src/**", "a", "b"]);
        assert_eq!(o.user_include_patterns, ["*.swift", "src/**"]);
        assert_eq!(o.left_path.as_deref(), Some("a"));
        assert_eq!(o.right_path.as_deref(), Some("b"));
    }

    #[test]
    fn test_long_include_accepts_inline_and_separate_values() {
        let o = parse_ok(&["--include=*.swift", "--include", "src/**", "a", "b"]);
        assert_eq!(o.user_include_patterns, ["*.swift", "src/**"]);
    }

    #[test]
    fn test_preset_option_is_opt_in_and_repeatable() {
        let o = parse_ok(&[
            "--preset",
            "build-artifacts",
            "--preset=web-nodejs",
            "a",
            "b",
        ]);
        assert_eq!(o.filter_presets, ["build-artifacts", "web-nodejs"]);
    }

    #[test]
    fn test_unknown_preset_is_error() {
        assert!(is_error(
            parse(&args(&["--preset", "bogus", "a", "b"])),
            &ParseError::UnknownPreset("bogus".into())
        ));
    }

    #[test]
    fn test_missing_include_value_is_error() {
        assert!(is_error(
            parse(&args(&["-i"])),
            &ParseError::MissingValue("-i".into())
        ));
        assert!(is_error(
            parse(&args(&["a", "b", "--include"])),
            &ParseError::MissingValue("--include".into())
        ));
        assert!(is_error(
            parse(&args(&["a", "b", "--preset"])),
            &ParseError::MissingValue("--preset".into())
        ));
    }

    #[test]
    fn test_include_with_slash_switch_and_absolute_paths_still_parse() {
        let o = parse_ok(&["/r", "-i", "*.swift", "/tmp/left", "/tmp/right"]);
        assert!(o.recursive);
        assert_eq!(o.user_include_patterns, ["*.swift"]);
        assert_eq!(o.left_path.as_deref(), Some("/tmp/left"));
        assert_eq!(o.right_path.as_deref(), Some("/tmp/right"));
    }

    #[test]
    fn test_comparator_options_propagates_includes() {
        let mut o = parse_ok(&["a", "b"]);
        o.user_include_patterns = vec!["*.swift".into()];
        let c = comparator_options(&o);
        assert_eq!(c.user_include_patterns, ["*.swift"]);
        assert!(c.user_exclude_patterns.is_empty());
    }

    #[test]
    fn test_comparator_options_expands_presets_into_excludes() {
        let mut o = parse_ok(&["a", "b"]);
        o.filter_presets = vec!["source-control".into(), "macos-metadata".into()];
        let c = comparator_options(&o);
        assert!(c.user_exclude_patterns.contains(&".git".to_string()));
        assert!(c.user_exclude_patterns.contains(&".DS_Store".to_string()));
        assert!(c.user_include_patterns.is_empty());
    }

    #[test]
    fn test_comparator_options_keeps_default_excludes_flag() {
        let mut o = parse_ok(&["a", "b"]);
        o.use_default_excludes = false;
        o.recursive = true;
        let c = comparator_options(&o);
        assert!(!c.use_default_excludes);
        assert!(c.recursive);
        assert!(c.max_entries.is_none());
    }

    #[test]
    fn test_comparator_options_preset_excludes_merge_with_user_excludes() {
        let mut o = parse_ok(&["a", "b"]);
        o.user_exclude_patterns = vec!["*.tmp".into()];
        o.filter_presets = vec!["build-artifacts".into()];
        let c = comparator_options(&o);
        assert!(c.user_exclude_patterns.contains(&"*.tmp".to_string()));
        assert!(c.user_exclude_patterns.contains(&"build".to_string()));
    }

    #[test]
    fn test_comparator_options_unselected_preset_does_nothing() {
        let o = parse_ok(&["a", "b"]);
        let c = comparator_options(&o);
        assert!(c.user_exclude_patterns.is_empty());
        assert!(c.user_include_patterns.is_empty());
    }
}

/// Swift `ReportOutputTests.swift` の 1:1 移植(8件)
mod report_output_tests {
    use super::*;

    fn entry(path: &str, status: DiffStatus, error: Option<&str>) -> ComparedEntry {
        ComparedEntry {
            relative_path: path.into(),
            name: path.rsplit('/').next().unwrap_or(path).into(),
            is_directory: false,
            status,
            left_path: None,
            right_path: None,
            left_size: 0,
            right_size: 0,
            error_message: error.map(String::from),
            left_modification_date: None,
            right_modification_date: None,
            left_posix: None,
            right_posix: None,
            rename_candidate: None,
        }
    }

    fn sample_result() -> ComparisonResult {
        let entries = vec![
            entry("same.txt", DiffStatus::Identical, None),
            entry("changed.txt", DiffStatus::Different, None),
            entry("only_left.txt", DiffStatus::LeftOnly, None),
            entry("only_right.txt", DiffStatus::RightOnly, None),
            entry("broken.txt", DiffStatus::Error, Some("読み込み不可")),
        ];
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
            summary: ComparisonSummary {
                identical_files: 1,
                different_files: 1,
                left_only_files: 1,
                right_only_files: 1,
                error_files: 1,
            },
        }
    }

    fn report_text(result: &ComparisonResult, format: ReportFormat) -> String {
        ComparisonReport::new(result, 1_700_000_000).render(format)
    }

    #[test]
    fn test_json_report_text_contains_summary() {
        let text = report_text(&sample_result(), ReportFormat::Json);
        assert!(text.contains("DiffLoupeComparisonReport"));
        assert!(text.contains("\"differentFiles\" : 1"));
        assert!(text.contains("\"identicalFiles\" : 1"));
    }

    #[test]
    fn test_csv_report_text_contains_header() {
        let text = report_text(&sample_result(), ReportFormat::Csv);
        assert!(text.contains("レポート種別,DiffLoupe比較レポート"));
        assert!(text.contains("相対パス,状態,種別,左サイズ,右サイズ"));
        assert!(text.contains("changed.txt,相違"));
    }

    #[test]
    fn test_markdown_report_text_contains_summary_table() {
        let text = report_text(&sample_result(), ReportFormat::Markdown);
        assert!(text.contains("## サマリ"));
        assert!(text.contains("## 詳細"));
        assert!(text.contains("| 一致 | 1 |"));
    }

    #[test]
    fn test_html_report_text_is_standalone() {
        let text = report_text(&sample_result(), ReportFormat::Html);
        assert!(text.starts_with("<!DOCTYPE html>"));
        assert!(text.contains("<h1>DiffLoupe比較レポート</h1>"));
        assert!(text.contains("</html>"));
    }

    #[test]
    fn test_write_creates_file_with_utf8_text() {
        let dir = temp_dir("ro1");
        let path = dir.join("report.csv");
        write_report_file("abc,def\n", path.to_str().unwrap()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "abc,def\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_write_refuses_existing_file() {
        let dir = temp_dir("ro2");
        let path = dir.join("report.json");
        write_report_file("first", path.to_str().unwrap()).unwrap();
        assert!(write_report_file("second", path.to_str().unwrap()).is_err());
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "first");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_write_refuses_missing_parent_directory() {
        let dir = temp_dir("ro3");
        let path = dir.join("no-such-dir/report.json");
        assert!(write_report_file("x", path.to_str().unwrap()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_validate_output_accepts_new_path_in_existing_directory() {
        let dir = temp_dir("ro4");
        let path = dir.join("new.md");
        assert!(validate_output(path.to_str().unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }
}
