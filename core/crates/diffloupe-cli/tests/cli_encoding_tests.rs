//! `--encoding` / `--list-encodings` のテスト。

#[path = "../src/cli.rs"]
mod cli;

use cli::*;
use diffloupe_core::cancel::CancellationToken;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn encoding_option_sets_the_canonical_name() {
    for (given, canonical) in [
        ("Shift_JIS", "Shift_JIS"),
        ("shift_jis", "Shift_JIS"),
        ("utf-16 le (bomなし)", "UTF-16 LE (BOMなし)"),
        ("KOI8-R", "KOI8-R"),
    ] {
        for form in [
            args(&["--encoding", given, "l", "r"]),
            args(&[&format!("--encoding={}", given), "l", "r"]),
        ] {
            match parse(&form) {
                ParseResult::Options(o) => {
                    assert_eq!(o.content_comparison.text_encoding, Some(canonical))
                }
                other => panic!("{:?}", other),
            }
        }
    }
    // 未指定なら None
    match parse(&args(&["l", "r"])) {
        ParseResult::Options(o) => assert_eq!(o.content_comparison.text_encoding, None),
        other => panic!("{:?}", other),
    }
}

#[test]
fn unknown_or_missing_encoding_is_a_usage_error() {
    match parse(&args(&["--encoding", "nope", "l", "r"])) {
        ParseResult::Error(ParseError::UnknownEncoding(n)) => assert_eq!(n, "nope"),
        other => panic!("{:?}", other),
    }
    match parse(&args(&["l", "r", "--encoding"])) {
        ParseResult::Error(ParseError::MissingValue(o)) => assert_eq!(o, "--encoding"),
        other => panic!("{:?}", other),
    }
    let token = CancellationToken::new();
    assert_eq!(
        run(&args(&["--encoding", "nope", "l", "r"]), &token),
        EXIT_USAGE_ERROR
    );
}

#[test]
fn list_encodings_exits_zero_and_wins_over_paths() {
    assert!(matches!(
        parse(&args(&["--list-encodings"])),
        ParseResult::ListEncodings
    ));
    assert!(matches!(
        parse(&args(&["--list-encodings", "a", "b", "c"])),
        ParseResult::ListEncodings
    ));
    let token = CancellationToken::new();
    assert_eq!(run(&args(&["--list-encodings"]), &token), EXIT_SUCCESS);
    // --help が優先
    assert!(matches!(
        parse(&args(&["--help", "--list-encodings"])),
        ParseResult::Help
    ));
}

#[test]
fn usage_mentions_both_options() {
    assert!(USAGE.contains("--encoding <名前>"));
    assert!(USAGE.contains("--list-encodings"));
}

#[test]
fn encoding_reaches_the_comparator_options() {
    match parse(&args(&[
        "--encoding",
        "EUC-JP",
        "--ignore-line-endings",
        "l",
        "r",
    ])) {
        ParseResult::Options(o) => {
            let c = comparator_options(&o);
            assert_eq!(c.content_comparison.text_encoding, Some("EUC-JP"));
            assert!(c.content_comparison.ignore_line_endings);
        }
        other => panic!("{:?}", other),
    }
}
