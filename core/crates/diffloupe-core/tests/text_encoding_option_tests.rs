//! `ContentComparison::text_encoding`(自動判定の代わりに使う文字コード)のテスト。

use diffloupe_core::comparator::{CompareOptions, FolderComparator};
use diffloupe_core::models::DiffStatus;
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured::StructuredComparisonMode;
use diffloupe_core::text_loader::{canonical_encoding_name, encode};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "DiffLoupeTests-encopt-{}-{}-{}",
        tag,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cc(encoding: &str) -> ContentComparison {
    ContentComparison {
        ignore_line_endings: true,
        text_encoding: Some(canonical_encoding_name(encoding).unwrap()),
        ..Default::default()
    }
}

#[test]
fn canonical_names_ignore_case_and_reject_unknown() {
    assert_eq!(canonical_encoding_name("shift_jis"), Some("Shift_JIS"));
    assert_eq!(
        canonical_encoding_name("UTF-16 le (bomなし)"),
        Some("UTF-16 LE (BOMなし)")
    );
    assert_eq!(canonical_encoding_name("koi8-r"), Some("KOI8-R"));
    assert_eq!(canonical_encoding_name("nope"), None);
    assert_eq!(canonical_encoding_name(""), None);
}

#[test]
fn normalized_data_reads_with_the_given_encoding() {
    let crlf = encode("Shift_JIS", "こんにちは\r\n世界\r\n").unwrap();
    let lf = encode("Shift_JIS", "こんにちは\n世界\n").unwrap();
    // 指定なしでは Shift_JIS は UTF-8 として読めないので正規化できない
    let plain = ContentComparison {
        ignore_line_endings: true,
        ..Default::default()
    };
    assert!(plain.normalized_data(&crlf).is_none());
    // 指定すれば、その文字コードで読んで正規化する(結果は UTF-8)
    let c = cc("Shift_JIS");
    assert_eq!(c.normalized_data(&crlf), c.normalized_data(&lf));
    assert_eq!(
        c.normalized_data(&crlf).unwrap(),
        "こんにちは\n世界\n".as_bytes().to_vec()
    );
    // NUL を含む UTF-16 も、指定すればテキストとして読む
    let u16: Vec<u8> = "a\nb\n"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert!(plain.normalized_data(&u16).is_none());
    let c16 = cc("UTF-16 LE (BOMなし)");
    assert_eq!(c16.normalized_data(&u16).unwrap(), b"a\nb\n".to_vec());
    // 不正なバイト列は置換して続行する
    let bad = ContentComparison {
        ignore_case: true,
        text_encoding: Some("UTF-8"),
        ..Default::default()
    };
    assert_eq!(
        bad.normalized_data(&[b'A', 0xFF, b'B']).unwrap(),
        "a\u{fffd}b".as_bytes().to_vec()
    );
    // 厳密比較(正規化なし)はバイト列そのまま
    let strict = ContentComparison {
        text_encoding: Some("Shift_JIS"),
        ..Default::default()
    };
    assert_eq!(strict.normalized_data(&crlf).unwrap(), crlf);
}

#[test]
fn settings_signature_includes_the_encoding_only_when_set() {
    let base = ContentComparison::default();
    assert_eq!(base.settings_signature(), "0000");
    let with = ContentComparison {
        text_encoding: Some("Shift_JIS"),
        ..Default::default()
    };
    assert_eq!(with.settings_signature(), "0000|enc=Shift_JIS");
    assert_ne!(
        with.settings_signature(),
        ContentComparison {
            text_encoding: Some("EUC-JP"),
            ..Default::default()
        }
        .settings_signature()
    );
}

#[test]
fn structured_comparison_uses_the_given_encoding() {
    // JSON(Shift_JIS、キー順違い)
    let l = encode("Shift_JIS", "{\"名前\": \"山田\", \"年齢\": 30}").unwrap();
    let r = encode("Shift_JIS", "{\"年齢\": 30,\n \"名前\": \"山田\"}").unwrap();
    let c = ContentComparison {
        structured_mode: StructuredComparisonMode::Json,
        text_encoding: Some("Shift_JIS"),
        ..Default::default()
    };
    let (a, b) = c
        .structured_canonical_pair(&l, Some("a.json"), &r, Some("b.json"))
        .expect("指定した文字コードで構造化できる");
    assert_eq!(a, b);
    // 指定なしでは UTF-8 でないので構造化できない
    let plain = ContentComparison {
        structured_mode: StructuredComparisonMode::Json,
        ..Default::default()
    };
    assert!(plain
        .structured_canonical_pair(&l, Some("a.json"), &r, Some("b.json"))
        .is_none());
    // XML: 宣言の encoding が実際と違っていても(指定が優先)、指定した文字コードで読める
    let xl = encode(
        "Shift_JIS",
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><r><a x='1' y='2'>日本語</a></r>",
    )
    .unwrap();
    let xr = encode(
        "Shift_JIS",
        "<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><r><a y=\"2\" x=\"1\">日本語</a></r>",
    )
    .unwrap();
    let cx = ContentComparison {
        structured_mode: StructuredComparisonMode::Xml,
        text_encoding: Some("Shift_JIS"),
        ..Default::default()
    };
    let (a, b) = cx
        .structured_canonical_pair(&xl, Some("a.xml"), &xr, Some("b.xml"))
        .expect("XML も指定した文字コードで読める");
    assert!(a.contains("日本語"), "{}", a);
    assert_eq!(a, b);
}

#[test]
fn folder_comparison_applies_the_encoding() {
    let l = temp_dir("l");
    let r = temp_dir("r");
    std::fs::write(
        l.join("a.txt"),
        encode("Shift_JIS", "こんにちは\r\n世界\r\n").unwrap(),
    )
    .unwrap();
    std::fs::write(
        r.join("a.txt"),
        encode("Shift_JIS", "こんにちは\n世界\n").unwrap(),
    )
    .unwrap();
    let with = CompareOptions {
        content_comparison: cc("Shift_JIS"),
        ..Default::default()
    };
    let res = FolderComparator::compare(&l, &r, &with, None, None).unwrap();
    assert_eq!(res.entries[0].status, DiffStatus::Identical);
    // 指定なしでも改行無視は(UTF-8 でなければ正規化できず)バイト比較になり相違
    let without = CompareOptions {
        content_comparison: ContentComparison {
            ignore_line_endings: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let res = FolderComparator::compare(&l, &r, &without, None, None).unwrap();
    assert_eq!(res.entries[0].status, DiffStatus::Different);
    let _ = std::fs::remove_dir_all(l);
    let _ = std::fs::remove_dir_all(r);
}
