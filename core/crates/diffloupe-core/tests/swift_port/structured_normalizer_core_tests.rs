//! Swift `CoreTests/StructuredNormalizerTests.swift` の 1:1 移植(31件)

use super::support::{compare, temp_pair, write};
use diffloupe_core::comparator::CompareOptions;
use diffloupe_core::models::DiffStatus;
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured::{self, StructuredComparisonMode as SM, StructuredFormat};

fn normalize(text: &str, url: Option<&str>, mode: SM) -> structured::StructuredNormalizationResult {
    structured::normalize(text.as_bytes(), url, mode)
}

// MARK: - JSON

#[test]
fn test_json_key_order_is_normalized() {
    let compact = normalize("{\"b\":2,\"a\":1}", Some("/tmp/x.json"), SM::Json);
    let pretty = normalize(
        "{\n  \"a\" : 1,\n  \"b\" : 2\n}",
        Some("/tmp/y.json"),
        SM::Json,
    );
    assert!(compact.did_normalize);
    assert!(pretty.did_normalize);
    assert_eq!(compact.normalized_text, pretty.normalized_text);
    assert_eq!(compact.format, Some(StructuredFormat::Json));
}

#[test]
fn test_json_deterministic_pretty_print() {
    let result = normalize("{\"b\":2,\"a\":1}", Some("/tmp/x.json"), SM::Json);
    assert_eq!(
        result.normalized_text.as_deref(),
        Some("{\n  \"a\" : 1,\n  \"b\" : 2\n}")
    );
}

#[test]
fn test_json_array_order_is_preserved() {
    let left = normalize("{\"a\":[1,2,3]}", Some("/tmp/l.json"), SM::Json);
    let right = normalize("{\"a\":[3,2,1]}", Some("/tmp/r.json"), SM::Json);
    assert!(left.did_normalize);
    assert!(right.did_normalize);
    assert_ne!(left.normalized_text, right.normalized_text);
}

#[test]
fn test_json_nested_structure() {
    let result = normalize(
        "{\"c\":{\"y\":1,\"x\":2},\"a\":[3,1]}",
        Some("/tmp/x.json"),
        SM::Json,
    );
    assert_eq!(
        result.normalized_text.as_deref(),
        Some("{\n  \"a\" : [\n    3,\n    1\n  ],\n  \"c\" : {\n    \"x\" : 2,\n    \"y\" : 1\n  }\n}")
    );
}

#[test]
fn test_json_scalar_fragments() {
    assert_eq!(
        normalize("42", Some("/tmp/n.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("42")
    );
    assert_eq!(
        normalize("\"hi\"", Some("/tmp/s.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("\"hi\"")
    );
    assert_eq!(
        normalize("true", Some("/tmp/b.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        normalize("null", Some("/tmp/u.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("null")
    );
}

#[test]
fn test_json_malformed_falls_back() {
    let result = normalize("{bad json", Some("/tmp/x.json"), SM::Json);
    assert!(!result.did_normalize);
    assert!(result.normalized_text.is_none());
    assert!(result.notice.is_some());
    assert_eq!(result.format, Some(StructuredFormat::Json));
}

// MARK: - JSON数値の精度(IEEE-754 の往復保持)

#[test]
fn test_json_nearby_doubles_stay_distinct() {
    let left = normalize("{\"a\":0.1234567}", Some("/tmp/l.json"), SM::Json);
    let right = normalize("{\"a\":0.1234568}", Some("/tmp/r.json"), SM::Json);
    assert!(left.did_normalize);
    assert!(right.did_normalize);
    assert_ne!(left.normalized_text, right.normalized_text);
}

#[test]
fn test_json_full_precision_is_round_tripped() {
    let result = normalize("{\"a\":0.30000000000000004}", Some("/tmp/x.json"), SM::Json);
    assert!(result.did_normalize);
    assert_eq!(
        result.normalized_text.as_deref(),
        Some("{\n  \"a\" : 0.30000000000000004\n}")
    );
}

#[test]
fn test_json_integral_double_still_renders_as_integer() {
    assert_eq!(
        normalize("{\"a\":1.0}", Some("/tmp/x.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("{\n  \"a\" : 1\n}")
    );
    assert_eq!(
        normalize("1.0", Some("/tmp/x.json"), SM::Json)
            .normalized_text
            .as_deref(),
        Some("1")
    );
}

// MARK: - XML

#[test]
fn test_xml_attribute_order_is_sorted() {
    let left = normalize("<a b=\"2\" a=\"1\"/>", Some("/tmp/l.xml"), SM::Xml);
    let right = normalize("<a a=\"1\" b=\"2\"/>", Some("/tmp/r.xml"), SM::Xml);
    assert!(left.did_normalize);
    assert!(right.did_normalize);
    assert_eq!(left.normalized_text, right.normalized_text);
    assert_eq!(
        left.normalized_text.as_deref(),
        Some("<a a=\"1\" b=\"2\"/>\n")
    );
}

#[test]
fn test_xml_indentation_whitespace_removed() {
    let pretty = normalize("<a>\n  <b/>\n  <c>x</c>\n</a>", Some("/tmp/l.xml"), SM::Xml);
    let compact = normalize("<a><b/><c>x</c></a>", Some("/tmp/r.xml"), SM::Xml);
    assert_eq!(pretty.normalized_text, compact.normalized_text);
    assert_eq!(
        pretty.normalized_text.as_deref(),
        Some("<a>\n  <b/>\n  <c>x</c>\n</a>\n")
    );
}

#[test]
fn test_xml_text_and_comments_preserved() {
    let result = normalize(
        "<a><!-- note --><b value=\"v\">text</b></a>",
        Some("/tmp/x.xml"),
        SM::Xml,
    );
    assert_eq!(
        result.normalized_text.as_deref(),
        Some("<a>\n  <!-- note -->\n  <b value=\"v\">text</b>\n</a>\n")
    );
}

#[test]
fn test_xml_mixed_content_keeps_meaningful_whitespace() {
    let result = normalize("<p>Hello <b>world</b>!</p>", Some("/tmp/x.xml"), SM::Xml);
    assert!(result.did_normalize);
    assert_eq!(
        result.normalized_text.as_deref(),
        Some("<p>\n  Hello \n  <b>world</b>\n  !\n</p>\n")
    );
}

#[test]
fn test_xml_malformed_falls_back() {
    let result = normalize("<a><b></a>", Some("/tmp/x.xml"), SM::Xml);
    assert!(!result.did_normalize);
    assert!(result.normalized_text.is_none());
    assert!(result.notice.is_some());
    assert_eq!(result.format, Some(StructuredFormat::Xml));
}

// MARK: - CSV / TSV

#[test]
fn test_csv_quoted_commas_and_escaped_quotes() {
    let quoted_comma = normalize("a,\"x,y\"\n", Some("/tmp/x.csv"), SM::Csv);
    assert!(quoted_comma.did_normalize);
    assert_eq!(quoted_comma.normalized_text.as_deref(), Some("a x,y\n"));

    let escaped_quotes = normalize("a,\"said \"\"hi\"\"\"\n", Some("/tmp/x.csv"), SM::Csv);
    assert!(escaped_quotes.did_normalize);
    assert_eq!(
        escaped_quotes.normalized_text.as_deref(),
        Some("a \"said \"\"hi\"\"\"\n")
    );
}

#[test]
fn test_csv_delimiter_detection() {
    let comma = normalize("a,b\nc,d\n", Some("/tmp/x.csv"), SM::Automatic);
    assert_eq!(comma.format, Some(StructuredFormat::Csv));
    assert_eq!(comma.normalized_text.as_deref(), Some("a b\nc d\n"));

    let tab = normalize("a\tb\nc\td\n", Some("/tmp/x.tsv"), SM::Automatic);
    assert_eq!(tab.format, Some(StructuredFormat::Csv));
    assert_eq!(tab.normalized_text.as_deref(), Some("a b\nc d\n"));

    let semicolon = normalize("a;b\n", Some("/tmp/x.csv"), SM::Automatic);
    assert_eq!(semicolon.normalized_text.as_deref(), Some("a b\n"));

    let quoted_comma_not_delimiter = normalize("\"a,b\"\n", Some("/tmp/x.csv"), SM::Automatic);
    assert_eq!(
        quoted_comma_not_delimiter.normalized_text.as_deref(),
        Some("a,b\n")
    );
}

#[test]
fn test_csv_unterminated_quote_falls_back() {
    let result = normalize("a,\"unterminated\n", Some("/tmp/x.csv"), SM::Csv);
    assert!(!result.did_normalize);
    assert!(result.normalized_text.is_none());
    assert!(result.notice.is_some());
    assert_eq!(result.format, Some(StructuredFormat::Csv));
}

#[test]
fn test_csv_whitespace_around_unquoted_fields_trimmed() {
    let left = normalize("a, b \n", Some("/tmp/l.csv"), SM::Csv);
    let right = normalize("a,b\n", Some("/tmp/r.csv"), SM::Csv);
    assert_eq!(left.normalized_text, right.normalized_text);
}

// MARK: - サイズ上限

#[test]
fn test_size_cap_falls_back_to_raw() {
    let oversized = vec![0x20u8; structured::MAX_PARSE_BYTES + 1];
    let result = structured::normalize(&oversized, Some("/tmp/x.json"), SM::Json);
    assert!(!result.did_normalize);
    assert!(result.normalized_text.is_none());
    assert!(result.notice.is_some());
    assert_eq!(result.format, Some(StructuredFormat::Json));
}

#[test]
fn test_at_or_below_cap_is_parsed() {
    let result = normalize("{\"a\":1}", Some("/tmp/x.json"), SM::Json);
    assert!(result.did_normalize);
}

// MARK: - モード/拡張子判定

#[test]
fn test_automatic_mode_detects_by_extension() {
    assert_eq!(
        normalize("{\"a\":1}", Some("/tmp/x.json"), SM::Automatic).format,
        Some(StructuredFormat::Json)
    );
    assert_eq!(
        normalize("<a/>", Some("/tmp/x.xml"), SM::Automatic).format,
        Some(StructuredFormat::Xml)
    );
    assert_eq!(
        normalize("a,b\n", Some("/tmp/x.csv"), SM::Automatic).format,
        Some(StructuredFormat::Csv)
    );
    assert_eq!(
        normalize("a\tb\n", Some("/tmp/x.tsv"), SM::Automatic).format,
        Some(StructuredFormat::Csv)
    );
}

#[test]
fn test_automatic_mode_unknown_extension_falls_back() {
    let result = normalize("just text", Some("/tmp/x.txt"), SM::Automatic);
    assert!(!result.did_normalize);
    assert!(result.format.is_none());
    assert!(result.notice.is_none());
}

#[test]
fn test_explicit_mode_overrides_extension() {
    let result = normalize("{\"a\":1}", Some("/tmp/x.txt"), SM::Json);
    assert!(result.did_normalize);
    assert_eq!(result.format, Some(StructuredFormat::Json));
}

#[test]
fn test_disabled_mode_is_no_op() {
    let result = normalize("{\"b\":2,\"a\":1}", Some("/tmp/x.json"), SM::Disabled);
    assert!(!result.did_normalize);
    assert!(result.format.is_none());
    assert!(result.normalized_text.is_none());
    assert!(result.notice.is_none());
}

// MARK: - UTF-8 BOM 除去(共通入口)

#[test]
fn test_csv_with_and_without_utf8_bom_normalize_equal() {
    let no_bom = normalize("a,b\n\"x,y\",z\n", Some("/tmp/n.csv"), SM::Csv);
    let bom_data = [vec![0xEF, 0xBB, 0xBF], b"a,b\n\"x,y\",z\n".to_vec()].concat();
    let with_bom = structured::normalize(&bom_data, Some("/tmp/b.csv"), SM::Csv);
    assert!(no_bom.did_normalize);
    assert!(with_bom.did_normalize);
    assert_eq!(no_bom.normalized_text, with_bom.normalized_text);
}

#[test]
fn test_utf8_bom_is_stripped_for_all_formats() {
    let json_bom = [vec![0xEF, 0xBB, 0xBF], b"{\"b\":2,\"a\":1}".to_vec()].concat();
    let json = structured::normalize(&json_bom, Some("/tmp/b.json"), SM::Json);
    assert_eq!(
        json.normalized_text,
        normalize("{\"b\":2,\"a\":1}", Some("/tmp/x.json"), SM::Json).normalized_text
    );
}

#[test]
fn test_csv_with_bom_identical_under_folder_compare() {
    let (base, l, r) = temp_pair("csnbom");
    let bom = [vec![0xEF, 0xBB, 0xBF], b"a,b\n\"x,y\",z\n".to_vec()].concat();
    write(&l, "data.csv", &bom);
    write(&r, "data.csv", b"a,b\n\"x,y\",z\n");
    let options = CompareOptions {
        content_comparison: ContentComparison {
            structured_mode: SM::Csv,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = compare(&l, &r, &options);
    assert_eq!(
        result.entry_by_path("data.csv").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 片側のみパース失敗時のフォールバック(P2-1)

#[test]
fn test_structured_canonical_pair_requires_both_sides_success() {
    let comparison = ContentComparison {
        structured_mode: SM::Json,
        ..Default::default()
    };
    let valid = br#"{"a":1}"#;
    let invalid = b"{bad json";
    let left_url = "/tmp/l.json";
    let right_url = "/tmp/r.json";

    assert!(comparison
        .structured_canonical_pair(valid, Some(left_url), valid, Some(right_url))
        .is_some());
    assert!(comparison
        .structured_canonical_pair(valid, Some(left_url), invalid, Some(right_url))
        .is_none());
    assert!(comparison
        .structured_canonical_pair(invalid, Some(left_url), valid, Some(right_url))
        .is_none());
    assert!(comparison
        .structured_canonical_pair(invalid, Some(left_url), invalid, Some(right_url))
        .is_none());

    let disabled = ContentComparison {
        structured_mode: SM::Disabled,
        ..Default::default()
    };
    assert!(disabled
        .structured_canonical_pair(valid, Some(left_url), valid, Some(right_url))
        .is_none());
}

#[test]
fn test_mixed_structured_pair_falls_back_under_folder_compare() {
    let (base, l, r) = temp_pair("csnmix");
    write(&l, "data.json", br#"{"a":1}"#);
    write(&r, "data.json", b"{bad json");
    let structured_options = CompareOptions {
        content_comparison: ContentComparison {
            structured_mode: SM::Json,
            ..Default::default()
        },
        ..Default::default()
    };
    let structured_result = compare(&l, &r, &structured_options);
    let disabled = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        structured_result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        structured_result.entry_by_path("data.json").unwrap().status,
        disabled.entry_by_path("data.json").unwrap().status
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 内部CSVパーサ(区切り検出)

#[test]
fn test_detect_delimiter_preference_on_tie() {
    assert_eq!(structured::detect_csv_delimiter("a,b;c\n"), ',');
    assert_eq!(structured::detect_csv_delimiter("a\tb;c\n"), '\t');
    assert_eq!(structured::detect_csv_delimiter("a,b\tc\n"), ',');
    assert_eq!(structured::detect_csv_delimiter("no delimiter\n"), ',');
}

#[test]
fn test_csv_parser_quoted_multiline_field() {
    let rows = structured::parse_csv("a,\"multi\nline\"\n", ',').unwrap();
    assert_eq!(rows, vec![vec!["a".to_string(), "multi\nline".to_string()]]);
}
