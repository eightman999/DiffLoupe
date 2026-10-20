//! 外部(Pro など)向けに追加した公開 API のテスト。既存の振る舞いは変えない追加のみ。

use diffloupe_core::cancel::CancellationToken;
use diffloupe_core::models::*;
use diffloupe_core::report::{iso8601_date, parse_iso8601};
use diffloupe_core::sync_plan::{SyncAction, SyncDirection, SyncItem, SyncPlan, SyncSummary};
use diffloupe_core::text_loader::{bom_for, decode, encode, encode_body};

#[test]
fn encode_roundtrips_through_decode() {
    let text = "日本語のテキスト\nabc ｱｲｳ\n";
    for name in [
        "UTF-8",
        "UTF-8 (BOM)",
        "UTF-16 BE",
        "UTF-16 LE",
        "UTF-32 BE",
        "UTF-32 LE",
        "Shift_JIS",
        "EUC-JP",
        "ISO-2022-JP",
    ] {
        let bytes = encode(name, text).unwrap_or_else(|| panic!("{name}"));
        let (back, got) = decode(&bytes).unwrap_or_else(|| panic!("decode {name}"));
        assert_eq!(back, text, "{name}");
        // Shift_JIS / EUC-JP / UTF-8 は内容次第で判定が揺れうるが、本文は戻る
        if name.starts_with("UTF-16") || name.starts_with("UTF-32") || name == "UTF-8 (BOM)" {
            assert_eq!(got, name);
        }
    }
}

#[test]
fn encode_bom_and_unsupported() {
    assert_eq!(
        encode("UTF-8 (BOM)", "a").unwrap(),
        vec![0xEF, 0xBB, 0xBF, b'a']
    );
    assert_eq!(encode("UTF-16 LE", "a").unwrap(), vec![0xFF, 0xFE, b'a', 0]);
    assert_eq!(bom_for("Shift_JIS"), Some(&[][..]));
    assert!(encode("KOI8-R", "a").is_none());
    assert!(encode_body("Shift_JIS", "😀").is_none());
    assert_eq!(
        encode("ISO-2022-JP", "あ").unwrap(),
        b"\x1b$B$\"\x1b(B".to_vec()
    );
    assert_eq!(
        encode("ISO-2022-JP", "ｱ").unwrap(),
        b"\x1b(I1\x1b(B".to_vec()
    );
}

#[test]
fn sync_names() {
    assert_eq!(SyncDirection::LeftToRight.display_name(), "左 → 右");
    assert_eq!(SyncDirection::RightToLeft.display_name(), "右 → 左");
    assert_eq!(SyncAction::CopyFile.raw_value(), "copyFile");
    assert_eq!(
        SyncAction::PreservedDestination.raw_value(),
        "preservedDestination"
    );
    assert_eq!(SyncAction::Conflict.display_name(), "差分(上書き候補)");
    assert_eq!(SyncAction::Blocked.display_name(), "エラー");
}

#[test]
fn destination_preserves_source_name_form() {
    let nfd = "か\u{3099}.txt"; // が (NFD)
    let nfc = "が.txt";
    let item = |action| SyncItem {
        relative_path: nfc.to_string(),
        action,
        source: Some(format!("/src/{nfd}")),
        destination: Some(format!("/dst/{nfc}")),
        bytes: 0,
        reason: String::new(),
        direction_key: "leftToRight",
    };
    let plan = SyncPlan {
        direction: SyncDirection::LeftToRight,
        source_root: "/src".into(),
        destination_root: "/dst".into(),
        items: vec![],
        summary: SyncSummary::default(),
    };
    // 計画の destination は NFC のまま、新 API はコピー元の表記を使う
    let c = item(SyncAction::CopyFile);
    assert_eq!(c.destination.as_deref(), Some("/dst/が.txt"));
    assert_eq!(
        plan.destination_preserving_source_name(&c).unwrap(),
        format!("/dst/{nfd}")
    );
    // 上書き(既存ファイル)は計画のパスをそのまま返す
    let o = item(SyncAction::Conflict);
    assert_eq!(
        plan.destination_preserving_source_name(&o).unwrap(),
        "/dst/が.txt"
    );
    let mut none = item(SyncAction::CopyFile);
    none.destination = None;
    assert!(plan.destination_preserving_source_name(&none).is_none());
}

#[test]
fn iso8601_parse_inverts_format() {
    for t in [0.0, 1_700_000_000.0, 951_782_400.0, -86_400.0] {
        assert_eq!(parse_iso8601(&iso8601_date(t)), Some(t));
    }
    assert_eq!(
        parse_iso8601("2020-01-01T00:00:00.5Z"),
        Some(1_577_836_800.5)
    );
    for bad in [
        "",
        "2020-01-01",
        "2020-13-01T00:00:00Z",
        "2020-01-01T00:00:00",
        "2020-01-01T00:00:00+09:00",
        "2020-01-01T00:00:0xZ",
    ] {
        assert_eq!(parse_iso8601(bad), None, "{bad}");
    }
}

#[test]
fn comparison_result_new_and_token_clone() {
    let summary = ComparisonSummary::default();
    let node = DiffNode {
        id: String::new(),
        name: String::new(),
        is_directory: true,
        status: DiffStatus::Identical,
        children: None,
    };
    let r = ComparisonResult::new("/l".into(), "/r".into(), vec![], node, summary);
    assert_eq!(r.left_root, "/l");
    assert!(r.entries.is_empty());

    let a = CancellationToken::new();
    let b = a.clone();
    assert!(!b.is_cancelled());
    a.cancel();
    assert!(b.is_cancelled());
}
