//! 追加の文字コード(Shift_JIS-2004 / EUC-JIS-2004 / ISO-2022-JP-2004 / ISO-2022-JP-2 / MacJapanese /
//! EBCDIC / UTF-7)と、自動判定の限界の改善・回帰テスト。
//!
//! 期待バイト列は、生成元(Python の codecs、Apple の libicucore / CoreFoundation)で作ったもの。

use diffloupe_core::text_loader::{
    bom_for, decode, decode_as, encode, encode_body_checked, supported_encodings, EncodeError,
};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

/// `decode_as` で読み、`encode` で元のバイト列に戻ることまで確認する
fn check(name: &str, text: &str, bytes: &[u8]) {
    let d = decode_as(bytes, name).unwrap_or_else(|| panic!("未対応: {}", name));
    assert!(!d.had_replacements, "{}: 置換が起きた", name);
    assert_eq!(d.text, text, "{}: 復号結果", name);
    let enc = encode(name, text).unwrap_or_else(|| panic!("{}: 符号化できない", name));
    assert_eq!(enc, bytes, "{}: 符号化結果", name);
}

#[test]
fn new_encodings_are_listed() {
    let names: Vec<&str> = supported_encodings().iter().map(|e| e.name).collect();
    for n in [
        "Shift_JIS-2004",
        "EUC-JIS-2004",
        "ISO-2022-JP-2004",
        "ISO-2022-JP-2",
        "MacJapanese",
        "IBM037",
        "IBM500",
        "IBM1140",
        "IBM273",
        "IBM930",
        "IBM939",
        "IBM290",
        "UTF-7",
        "ISO-2022-JP (旧JIS)",
        "ISO-2022-JP (JISローマ字)",
        "ISO-2022-JP (旧JIS・JISローマ字)",
    ] {
        assert!(names.contains(&n), "一覧に無い: {}", n);
        assert_eq!(bom_for(n), Some(&[][..]));
    }
    let group = |n: &str| {
        supported_encodings()
            .into_iter()
            .find(|e| e.name == n)
            .unwrap()
            .group
    };
    assert_eq!(group("UTF-7"), "Unicode");
    assert_eq!(group("IBM037"), "EBCDIC");
    assert_eq!(group("IBM930"), "日本語");
    assert_eq!(group("MacJapanese"), "日本語");
    assert_eq!(group("Shift_JIS-2004"), "日本語");
}

#[test]
fn shift_jis_2004_matches_reference_bytes() {
    // 第 3・第 4 水準(𠮟 = 面 1、𩸽 = 面 2)と結合文字(か + U+309A)
    check(
        "Shift_JIS-2004",
        "日本語 A 𠮟𩸽か\u{309a}\n",
        &hex("93fa967b8cea2041209873fc6b82f50a"),
    );
    check("Shift_JIS-2004", "ｱｲｳ\n", &hex("b1b2b30a"));
    check(
        "Shift_JIS-2004",
        "① ㈱ ‰ ∮\n",
        &hex("874020878a2081f12087930a"),
    );
    // ASCII はそのまま(0x5C は円記号ではなくバックスラッシュ)
    check("Shift_JIS-2004", "C:\\a~b\n", b"C:\\a~b\n");
    // 表せない文字(U+00A5 は JIS X 0213 に無い)
    assert_eq!(
        encode_body_checked("Shift_JIS-2004", "a\n¥").unwrap_err(),
        EncodeError::Unmappable { ch: '¥', line: 2 }
    );
}

#[test]
fn euc_jis_2004_matches_reference_bytes() {
    check(
        "EUC-JIS-2004",
        "日本語 A 𠮟𩸽か\u{309a}\n",
        &hex("c6fccbdcb8ec204120cfd48ffdcca4f70a"),
    );
    check("EUC-JIS-2004", "ｱｲｳ\n", &hex("8eb18eb28eb30a"));
    check(
        "EUC-JIS-2004",
        "① ㈱ ‰ ∮\n",
        &hex("ada120adea20a2f320adf30a"),
    );
}

#[test]
fn iso_2022_jp_2004_matches_reference_bytes() {
    check(
        "ISO-2022-JP-2004",
        "日本語 A 𠮟𩸽か\u{309a}\n",
        &hex("1b2442467c4b5c386c1b28422041201b2428514f541b2428507d4c1b24285124771b28420a"),
    );
    check(
        "ISO-2022-JP-2004",
        "① ㈱\n",
        &hex("1b2428512d211b2842201b2428512d6a1b28420a"),
    );
    // ESC $ ( O(2000 年版の面 1)も読める
    let d = decode_as(b"\x1b$(O$w\x1b(B\n", "ISO-2022-JP-2004").unwrap();
    assert_eq!(d.text, "か\u{309a}\n");
    // 自動判定でも ISO-2022-JP-2004 になる
    let (t, n) = decode(&hex(
        "1b2442467c4b5c386c1b28422041201b2428514f541b2428507d4c1b24285124771b28420a",
    ))
    .unwrap();
    assert_eq!(
        (t.as_str(), n),
        ("日本語 A 𠮟𩸽か\u{309a}\n", "ISO-2022-JP-2004")
    );
}

#[test]
fn jis_x_0213_encodings_are_detected_when_legacy_decoders_fail() {
    // 面 2 の字(𩸽)を含む EUC-JIS-2004。EUC-JP(WHATWG)では読めない
    let bytes = hex("c6fccbdcb8ec204120cfd48ffdcca4f70a");
    assert!(encoding_rs::EUC_JP
        .decode_without_bom_handling_and_without_replacement(&bytes)
        .is_none());
    let (t, n) = decode(&bytes).unwrap();
    assert_eq!(
        (t.as_str(), n),
        ("日本語 A 𠮟𩸽か\u{309a}\n", "EUC-JIS-2004")
    );
}

#[test]
fn iso_2022_jp_2_matches_reference_bytes() {
    // Python の iso2022_jp_2 が作ったバイト列を読む(GB 2312 は ESC $ ( A でも読める)
    for (hexs, text) in [
        ("1b2442467c4b5c386c1b28420a", "日本語\n"),
        ("1b2428442b311b2842206162630a", "é abc\n"),
        ("1b242843475131393e6e1b28420a", "한국어\n"),
        ("1b2428413c721b2442424e3b7a1b28420a", "简体字\n"),
        (
            "1b2428442b311b2442467c4b5c1b28420a1b244224351b28420a",
            "é日本\nさ\n",
        ),
        ("1b24284426711b28420a", "ά\n"),
    ] {
        let d = decode_as(&hex(hexs), "ISO-2022-JP-2").unwrap();
        assert!(!d.had_replacements, "{}", text);
        assert_eq!(d.text, text);
    }
    // 符号化(GB 2312 は ESC $ A)
    check(
        "ISO-2022-JP-2",
        "日本語\n",
        &hex("1b2442467c4b5c386c1b28420a"),
    );
    check(
        "ISO-2022-JP-2",
        "é abc\n",
        &hex("1b2428442b311b2842206162630a"),
    );
    check(
        "ISO-2022-JP-2",
        "한국어\n",
        &hex("1b242843475131393e6e1b28420a"),
    );
    check(
        "ISO-2022-JP-2",
        "简体字\n",
        &hex("1b24413c721b2442424e3b7a1b28420a"),
    );
    // G2(ISO-8859-1 / ISO-8859-7 の上位。ESC . A / ESC . F + ESC N)の往復
    let text = "x\u{a0}y\u{ad}z\nw\u{a0}\n";
    let bytes = encode("ISO-2022-JP-2", text).unwrap();
    assert!(bytes.windows(3).any(|w| w == b"\x1b.A"));
    assert_eq!(decode_as(&bytes, "ISO-2022-JP-2").unwrap().text, text);
    // 自動判定
    let (t, n) = decode(&hex("1b242843475131393e6e1b28420a")).unwrap();
    assert_eq!((t.as_str(), n), ("한국어\n", "ISO-2022-JP-2"));
    let (_, n) = decode(&hex("1b2428413c721b2442424e3b7a1b28420a")).unwrap();
    assert_eq!(n, "ISO-2022-JP-2");
    // 半角カナは表せない
    assert!(encode("ISO-2022-JP-2", "ｱ").is_none());
}

#[test]
fn mac_japanese_matches_reference_bytes() {
    // Apple CoreFoundation(kCFStringEncodingMacJapanese)で作ったバイト列
    check(
        "MacJapanese",
        "日本語 ABC ｱｲｳ\n",
        b"\x93\xfa\x96{\x8c\xea ABC \xb1\xb2\xb3\n",
    );
    // Apple 拡張(0x85 行の丸数字、0x87 行の㈱)と、0x5C = ¥、0x80 = バックスラッシュ
    check("MacJapanese", "① ㈱ ♪\n", b"\x85@ \x87M \x81\xf4\n");
    check("MacJapanese", "a\\b ¥ ~\n", b"a\x80b \\ ~\n");
    check(
        "MacJapanese",
        "東京 \u{a9}\u{2122}\n",
        b"\x93\x8c\x8b\x9e \xfd\xfe\n",
    );
}

#[test]
fn ebcdic_single_byte_matches_reference_bytes() {
    check(
        "IBM037",
        "Hello [World]!\n",
        &hex("c88593939640bae696999384bb5a25"),
    );
    check(
        "IBM500",
        "Hello [World]!\n",
        &hex("c885939396404ae6969993845a4f25"),
    );
    check("IBM1140", "Price: 5€\n", &hex("d7998983857a40f59f25"));
    check(
        "IBM273",
        "Größe äöü ß [x]\n",
        &hex("c7996aa18540c06ad040a14063a7fc25"),
    );
    // NEL(0x15)も LF として読む(符号化する LF は 0x25)
    let d = decode_as(&hex("c11 5c225".replace(' ', "").as_str()), "IBM037").unwrap();
    assert_eq!(d.text, "A\nB\n");
    assert_eq!(encode("IBM037", "A\nB\n").unwrap(), hex("c125c225"));
    // NEL そのもの(U+0085)は 0x15 に符号化できる
    assert_eq!(encode("IBM037", "\u{85}").unwrap(), hex("15"));
    // EBCDIC に無い文字
    assert!(encode("IBM037", "日本").is_none());
    assert!(encode("IBM037", "€").is_none());
}

#[test]
fn japanese_ebcdic_matches_reference_bytes() {
    // Apple libicucore(ibm-930_P120-1999 / ibm-939_P120-1999 / ibm-290_P100-1995)で作ったバイト列
    check(
        "IBM930",
        "ABC日本語ｱｲｳ 東京\n",
        &hex("c1c2c30e4562456648e70f818283400e45574575 0f25"
            .replace(' ', "")
            .as_str()),
    );
    check(
        "IBM939",
        "abc ABC日本語 東京\n",
        &hex("818283 40c1c2c30e4562456648e70f400e455745750f25"
            .replace(' ', "")
            .as_str()),
    );
    check("IBM290", "ｱｲｳ ABC\n", &hex("81828340c1c2c325"));
    // 全角空白は DBCS の 0x4040
    let b = encode("IBM930", "あ\u{3000}い").unwrap();
    assert_eq!(b.first(), Some(&0x0E));
    assert_eq!(b.last(), Some(&0x0F));
    assert_eq!(decode_as(&b, "IBM930").unwrap().text, "あ\u{3000}い");
    // SI のない終端や、不正な DBCS は置換
    let d = decode_as(&hex("0e4562"), "IBM930").unwrap();
    assert_eq!(d.text, "日");
    assert!(
        decode_as(&hex("0e4560ff0f"), "IBM930")
            .unwrap()
            .had_replacements
    );
}

#[test]
fn utf7_matches_reference_bytes() {
    // Python の utf_7 が作ったバイト列
    check("UTF-7", "日本語", b"+ZeVnLIqe-");
    check("UTF-7", "a+b", b"a+-b");
    check("UTF-7", "Hi Mom -\u{263a}-!", b"Hi Mom -+Jjo--!");
    check(
        "UTF-7",
        "~\\!#$%&*;<=>@[]^_`{|}",
        b"+AH4AXA!#$%&*;<=>@[]^_`{|}",
    );
    check("UTF-7", "日本語abc日本", b"+ZeVnLIqe-abc+ZeVnLA-");
    check("UTF-7", "日-", b"+ZeU--");
    check("UTF-7", "\u{1F600}", b"+2D3eAA-");
    check("UTF-7", "a \u{e9} b", b"a +AOk b");
    // 終端の `-` が無くても読める。不正な Base64 は置換
    assert_eq!(decode_as(b"+ZeVnLIqe", "UTF-7").unwrap().text, "日本語");
    assert!(decode_as(b"+!", "UTF-7").unwrap().had_replacements);
    assert!(decode_as(&[b'a', 0xFF], "UTF-7").unwrap().had_replacements);
    // UTF-7 は自動判定しない(ASCII なので UTF-8 として読まれる)
    assert_eq!(decode(b"+ZeVnLIqe-").unwrap().1, "UTF-8");
}

#[test]
fn iso_2022_jp_keeps_old_jis_and_jis_roman_designations() {
    let cases: [(&[u8], &str); 5] = [
        (b"\x1b$@$\"$$\x1b(B\n", "ISO-2022-JP (旧JIS)"),
        (b"\x1b$B$\"$$\x1b(J\n", "ISO-2022-JP (JISローマ字)"),
        (b"\x1b$@$\"$$\x1b(J\n", "ISO-2022-JP (旧JIS・JISローマ字)"),
        (
            b"abc \x1b$@F|K\\\x1b(B xyz\n\x1b$@$\"\x1b(B\n",
            "ISO-2022-JP (旧JIS)",
        ),
        (
            b"abc \x1b$BF|K\\\x1b(J xyz\n\x1b$B$\"\x1b(J\n",
            "ISO-2022-JP (JISローマ字)",
        ),
    ];
    for (bytes, name) in cases {
        // 自動判定・名前指定のどちらでも、同じ指示で保存すると元のバイト列に戻る
        let (t, n) = decode(bytes).unwrap();
        assert_eq!(n, name, "{:?}", String::from_utf8_lossy(bytes));
        assert_eq!(encode(n, &t).unwrap(), bytes);
        let d = decode_as(bytes, "ISO-2022-JP").unwrap();
        assert_eq!(d.encoding, name);
        assert_eq!(encode(d.encoding, &d.text).unwrap(), bytes);
    }
    // 通常の ISO-2022-JP は従来どおり
    let plain = b"abc \x1b$BF|K\\\x1b(B xyz\n";
    let (t, n) = decode(plain).unwrap();
    assert_eq!(n, "ISO-2022-JP");
    assert_eq!(encode(n, &t).unwrap(), plain);
    // JIS ローマ字の指示を保つときは U+00A5 / U+203E も書ける(0x5C / 0x7E)。バックスラッシュは ESC ( B へ戻す
    let roman = encode("ISO-2022-JP (JISローマ字)", "あ¥\\~‾\n").unwrap();
    assert_eq!(roman, b"\x1b$B$\"\x1b(J\x5c\x1b(B\\~\x1b(J\x7e\n".to_vec());
    assert_eq!(decode_as(&roman, "ISO-2022-JP").unwrap().text, "あ¥\\~‾\n");
    assert!(encode("ISO-2022-JP", "¥").is_none());
}
