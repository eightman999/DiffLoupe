//! Swift `DiffLoupeTests/StructuredNormalizerTests.swift` の 1:1 移植
//! (testStructuredModePersistsInUserDefaults は AppModel/UserDefaults のため対象外)

use super::support::{compare, temp_pair, write};
use diffloupe_core::comparator::CompareOptions;
use diffloupe_core::models::DiffStatus;
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured::StructuredComparisonMode as SM;

fn structured_options(mode: SM) -> CompareOptions {
    CompareOptions {
        content_comparison: ContentComparison {
            structured_mode: mode,
            ..Default::default()
        },
        ..Default::default()
    }
}

// MARK: - JSON

#[test]
fn test_json_key_order_is_identical_under_structured_mode() {
    let (base, l, r) = temp_pair("sn1");
    write(&l, "data.json", br#"{"b":2,"a":1}"#);
    write(&r, "data.json", br#"{"a":1,"b":2}"#);
    let strict = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        strict.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    let structured = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        structured.entry_by_path("data.json").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_json_real_difference_still_detected() {
    let (base, l, r) = temp_pair("sn2");
    write(&l, "data.json", br#"{"a":1,"b":2}"#);
    write(&r, "data.json", br#"{"a":1,"b":99}"#);
    let result = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_json_array_order_difference_still_detected() {
    let (base, l, r) = temp_pair("sn3");
    write(&l, "data.json", br#"{"a":[1,2,3]}"#);
    write(&r, "data.json", br#"{"a":[3,2,1]}"#);
    let result = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - XML

#[test]
fn test_xml_attribute_order_is_identical_under_structured_mode() {
    let (base, l, r) = temp_pair("sn4");
    write(&l, "data.xml", br#"<a b="2" a="1"/>"#);
    write(&r, "data.xml", br#"<a a="1" b="2"/>"#);
    let strict = compare(&l, &r, &CompareOptions::default());
    assert_eq!(
        strict.entry_by_path("data.xml").unwrap().status,
        DiffStatus::Different
    );
    let structured = compare(&l, &r, &structured_options(SM::Xml));
    assert_eq!(
        structured.entry_by_path("data.xml").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_xml_indentation_only_difference_is_identical() {
    let (base, l, r) = temp_pair("sn5");
    write(&l, "data.xml", b"<a>\n  <b/>\n  <c>x</c>\n</a>");
    write(&r, "data.xml", b"<a><b/><c>x</c></a>");
    let structured = compare(&l, &r, &structured_options(SM::Xml));
    assert_eq!(
        structured.entry_by_path("data.xml").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - CSV

#[test]
fn test_csv_quoting_difference_is_identical_under_structured_mode() {
    let (base, l, r) = temp_pair("sn6");
    write(&l, "data.csv", b"a,b\n\"x,y\",z\n");
    write(&r, "data.csv", b"a,b\n\"x,y\",z\n");
    let structured = compare(&l, &r, &structured_options(SM::Csv));
    assert_eq!(
        structured.entry_by_path("data.csv").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - モード / フォールバック

#[test]
fn test_automatic_mode_uses_extension() {
    let (base, l, r) = temp_pair("sn7");
    write(&l, "data.json", br#"{"b":2,"a":1}"#);
    write(&r, "data.json", br#"{"a":1,"b":2}"#);
    let result = compare(&l, &r, &structured_options(SM::Automatic));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_malformed_structured_file_falls_back_to_raw_compare() {
    let (base, l, r) = temp_pair("sn8");
    write(&l, "data.json", b"{bad json");
    write(&r, "data.json", b"{bad json");
    let same = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        same.entry_by_path("data.json").unwrap().status,
        DiffStatus::Identical
    );
    write(&r, "data.json", b"{bad json!");
    let different = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        different.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_hash_equal_files_stay_identical_under_structured_mode() {
    let (base, l, r) = temp_pair("sn9");
    write(&l, "data.json", b"same bytes");
    write(&r, "data.json", b"same bytes");
    let result = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_disabled_mode_preserves_strict_behavior() {
    let (base, l, r) = temp_pair("sn10");
    write(&l, "data.json", br#"{"b":2,"a":1}"#);
    write(&r, "data.json", br#"{"a":1,"b":2}"#);
    let result = compare(&l, &r, &structured_options(SM::Disabled));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 精度・BOM・片側失敗フォールバック

#[test]
fn test_json_nearby_doubles_detected_under_structured_mode() {
    let (base, l, r) = temp_pair("sn11");
    write(&l, "data.json", br#"{"a":0.1234567}"#);
    write(&r, "data.json", br#"{"a":0.1234568}"#);
    let result = compare(&l, &r, &structured_options(SM::Json));
    assert_eq!(
        result.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_csv_utf8_bom_is_equivalent_to_no_bom_under_structured_mode() {
    let (base, l, r) = temp_pair("sn12");
    let bom = [vec![0xEF, 0xBB, 0xBF], b"a,b\n\"x,y\",z\n".to_vec()].concat();
    write(&l, "data.csv", &bom);
    write(&r, "data.csv", b"a,b\n\"x,y\",z\n");
    let result = compare(&l, &r, &structured_options(SM::Csv));
    assert_eq!(
        result.entry_by_path("data.csv").unwrap().status,
        DiffStatus::Identical
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn test_structured_mixed_success_falls_back_to_raw_comparison() {
    let (base, l, r) = temp_pair("sn13");
    write(&l, "data.json", br#"{"a":1}"#);
    write(&r, "data.json", b"{bad json");
    let structured = compare(&l, &r, &structured_options(SM::Json));
    let disabled = compare(&l, &r, &structured_options(SM::Disabled));
    assert_eq!(
        structured.entry_by_path("data.json").unwrap().status,
        DiffStatus::Different
    );
    assert_eq!(
        structured.entry_by_path("data.json").unwrap().status,
        disabled.entry_by_path("data.json").unwrap().status
    );
    std::fs::remove_dir_all(&base).ok();
}

// MARK: - 詳細差分と設定署名

#[test]
fn test_structured_mode_disables_strict_flag() {
    let mut comparison = ContentComparison::default();
    assert!(comparison.is_strict());
    assert_eq!(comparison.settings_signature(), "0000");
    comparison.structured_mode = SM::Json;
    assert!(!comparison.is_strict());
    assert_eq!(comparison.settings_signature(), "0000|json");
}
