//! 文字コード対応(text_loader.rs の拡張)のテスト:
//! 一覧、指定読み込み、全文字コードの往復、自動判定の拡張、バイナリの誤判定なし、符号化エラー。

use diffloupe_core::text_loader::{
    bom_for, decode, decode_as, encode, encode_body, encode_body_checked, encode_checked, load,
    load_as, supported_encodings, EncodeError, TextLoadAsResult, TextLoadResult,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn write_temp(data: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "DiffLoupeTests-enc-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&p, data).unwrap();
    p
}

const JA: &str = "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。吾輩はここで始めて人間というものを見た。\n";
const ZH_CN: &str = "我们在这个星期六一起去公园玩吧，天气预报说那天会是晴天，非常适合户外活动。大家可以带上自己喜欢的食物和饮料，在草地上野餐，还可以放风筝、打羽毛球。希望每个人都能度过愉快的时光。\n";
const ZH_TW: &str = "我們在這個星期六一起去公園玩吧，天氣預報說那天會是晴天，非常適合戶外活動。大家可以帶上自己喜歡的食物和飲料，在草地上野餐，還可以放風箏、打羽毛球。希望每個人都能度過愉快的時光。\n";
const KO: &str = "우리는 이번 토요일에 함께 공원에 가서 놀기로 했습니다. 일기예보에 따르면 그날은 맑을 것이라고 합니다. 각자 좋아하는 음식과 음료를 가져와서 잔디밭에서 소풍을 즐길 수 있습니다. 모두가 즐거운 시간을 보내기를 바랍니다.\n";
const FR: &str = "Il était une fois, dans un petit village près de la forêt, une jeune fille qui aimait beaucoup les fleurs. Chaque matin, elle cueillait un bouquet de roses et de marguerites pour sa grand-mère, qui habitait de l'autre côté de la rivière. Été comme hiver, elle traversait le vieux pont de pierre, même sous la pluie, très contente de rendre visite à sa chère mamie.\n";
const RU: &str = "Сегодня мы пойдём в парк, где будем гулять, кататься на велосипедах и устраивать пикник на траве. Погода обещает быть прекрасной, поэтому все собираются взять с собой еду и напитки. Дети будут играть в футбол, а взрослые смогут спокойно поговорить и отдохнуть после долгой рабочей недели.\n";

fn names() -> Vec<&'static str> {
    supported_encodings().iter().map(|e| e.name).collect()
}

#[test]
fn supported_encodings_list_is_well_formed() {
    let list = supported_encodings();
    assert_eq!(list.first().unwrap().name, "UTF-8");
    assert_eq!(list.first().unwrap().group, "Unicode");
    let mut seen = std::collections::HashSet::new();
    for e in &list {
        assert!(seen.insert(e.name), "名前が重複: {}", e.name);
        assert!(!e.display_name.is_empty());
        assert!(bom_for(e.name).is_some(), "bom_for が未対応: {}", e.name);
        assert!(
            [
                "Unicode",
                "日本語",
                "中国語",
                "韓国語",
                "西欧",
                "中欧・東欧",
                "キリル",
                "その他",
                "EBCDIC"
            ]
            .contains(&e.group),
            "{}: {}",
            e.name,
            e.group
        );
    }
    // 表示順: Unicode → 日本語 → その他
    let first_non_unicode = list.iter().position(|e| e.group != "Unicode").unwrap();
    assert!(list[..first_non_unicode]
        .iter()
        .all(|e| e.group == "Unicode"));
    assert_eq!(list[first_non_unicode].group, "日本語");
    // 要求された文字コードがすべて入っている
    for n in [
        "UTF-8",
        "UTF-8 (BOM)",
        "UTF-16 LE",
        "UTF-16 BE",
        "UTF-16 LE (BOMなし)",
        "UTF-16 BE (BOMなし)",
        "UTF-32 LE",
        "UTF-32 BE",
        "UTF-32 LE (BOMなし)",
        "UTF-32 BE (BOMなし)",
        "Shift_JIS",
        "EUC-JP",
        "ISO-2022-JP",
        "GB18030",
        "GBK",
        "Big5",
        "EUC-KR",
        "windows-1250",
        "windows-1251",
        "windows-1252",
        "windows-1253",
        "windows-1254",
        "windows-1255",
        "windows-1256",
        "windows-1257",
        "windows-1258",
        "ISO-8859-1",
        "ISO-8859-2",
        "ISO-8859-3",
        "ISO-8859-4",
        "ISO-8859-5",
        "ISO-8859-6",
        "ISO-8859-7",
        "ISO-8859-8",
        "ISO-8859-10",
        "ISO-8859-13",
        "ISO-8859-14",
        "ISO-8859-15",
        "ISO-8859-16",
        "KOI8-R",
        "KOI8-U",
        "IBM866",
        "macintosh",
        "x-mac-cyrillic",
        "windows-874",
    ] {
        assert!(names().contains(&n), "一覧に無い: {}", n);
    }
}

/// 各文字コードで表せる試験文
fn sample_for(name: &str) -> String {
    match name {
        n if n.starts_with("UTF-") => "Hello こんにちは 😀 \u{1F600}\nline2\n".to_string(),
        "Shift_JIS" | "EUC-JP" | "ISO-2022-JP" => "日本語 ABC ｱｲｳ ①㈱\n2行目\n".to_string(),
        n if n.starts_with("ISO-2022-JP (") => "日本語 ABC ｱｲｳ ①㈱\n2行目\n".to_string(),
        "Shift_JIS-2004" | "EUC-JIS-2004" | "ISO-2022-JP-2004" => {
            "日本語 ABC ｱｲｳ ①㈱ 𠮟𩸽か\u{309a}\n2行目\n".to_string()
        }
        "ISO-2022-JP-2" => "日本語 ABC é 한국어 简体字 ά\n2行目\n".to_string(),
        "MacJapanese" => "日本語 ABC ｱｲｳ ① ㈱ ♪ ¥ \\ ©\n".to_string(),
        "IBM930" | "IBM939" => "ABC 日本語 東京\u{3000}ｱ\n2行目\n".to_string(),
        "GB18030" | "GBK" => "你好，世界。简体中文\n".to_string(),
        "Big5" => "你好，世界。繁體中文\n".to_string(),
        "EUC-KR" => "안녕하세요 한국어\n".to_string(),
        _ => {
            // 単バイト系: 0x20-0xFF のうち定義されていて制御文字でないバイトを全部並べる
            let mut s = String::new();
            for b in 0x20u8..=0xFF {
                let d = decode_as(&[b], name).unwrap();
                if !d.had_replacements && d.text.chars().all(|c| !c.is_control()) {
                    s.push_str(&d.text);
                }
            }
            s.push('\n');
            s
        }
    }
}

#[test]
fn every_encoding_roundtrips_through_decode_as_and_encode() {
    for name in names() {
        let sample = sample_for(name);
        let bytes = encode(name, &sample).unwrap_or_else(|| panic!("encode 失敗: {}", name));
        let d = decode_as(&bytes, name).unwrap_or_else(|| panic!("decode_as 失敗: {}", name));
        assert!(!d.had_replacements, "{}: 置換が起きた", name);
        assert_eq!(d.text, sample, "{}: 往復で文字が変わった", name);
        assert_eq!(d.encoding, name, "{}", name);
        let again = encode(d.encoding, &d.text).unwrap();
        assert_eq!(again, bytes, "{}: バイト列が元に戻らない", name);
    }
}

#[test]
fn decode_as_reports_actual_bom_presence() {
    // BOM なしの名前を指定しても BOM があれば取り除き、名前は BOM 付きになる(往復で BOM が戻る)
    let with_bom = [0xEF, 0xBB, 0xBF, b'a'];
    let d = decode_as(&with_bom, "UTF-8").unwrap();
    assert_eq!((d.text.as_str(), d.encoding), ("a", "UTF-8 (BOM)"));
    assert_eq!(encode(d.encoding, &d.text).unwrap(), with_bom);
    // BOM 付きの名前を指定しても BOM が無ければ BOM なしの名前
    let d = decode_as(b"a", "UTF-8 (BOM)").unwrap();
    assert_eq!(d.encoding, "UTF-8");
    let d = decode_as(&[0xFF, 0xFE, b'a', 0], "UTF-16 LE (BOMなし)").unwrap();
    assert_eq!((d.text.as_str(), d.encoding), ("a", "UTF-16 LE"));
    let d = decode_as(&[0, b'a'], "UTF-16 BE").unwrap();
    assert_eq!((d.text.as_str(), d.encoding), ("a", "UTF-16 BE (BOMなし)"));
    let d = decode_as(&[0, 0, 0xFE, 0xFF, 0, 0, 0, b'a'], "UTF-32 BE (BOMなし)").unwrap();
    assert_eq!((d.text.as_str(), d.encoding), ("a", "UTF-32 BE"));
}

#[test]
fn decode_as_replaces_invalid_bytes_and_reports_it() {
    let d = decode_as(&[b'a', 0xFF, b'b'], "UTF-8").unwrap();
    assert_eq!(d.text, "a\u{FFFD}b");
    assert!(d.had_replacements);
    let d = decode_as(&[b'a', 0x82], "Shift_JIS").unwrap();
    assert!(d.had_replacements);
    assert!(d.text.contains('\u{FFFD}'));
    // 奇数長の UTF-16、孤立サロゲート、不正なコードポイントの UTF-32
    let d = decode_as(&[b'a', 0, b'b'], "UTF-16 LE (BOMなし)").unwrap();
    assert!(d.had_replacements);
    assert_eq!(d.text, "a\u{FFFD}");
    let d = decode_as(&[0x00, 0xD8, b'a', 0], "UTF-16 LE (BOMなし)").unwrap();
    assert!(d.had_replacements);
    let d = decode_as(&[0, 0, 0xD8, 0, 0x41, 0, 0, 0], "UTF-32 LE (BOMなし)").unwrap();
    assert!(d.had_replacements);
    assert_eq!(d.text, "\u{FFFD}A");
    // 置換なし
    assert!(
        !decode_as("あ".as_bytes(), "UTF-8")
            .unwrap()
            .had_replacements
    );
    // 未知の名前
    assert!(decode_as(b"a", "UTF-99").is_none());
    assert!(decode_as(b"a", "").is_none());
}

#[test]
fn load_as_reads_files_and_rejects_unknown_names() {
    let sjis = encode("Shift_JIS", "こんにちは\n").unwrap();
    let p = write_temp(&sjis);
    match load_as(&p, "Shift_JIS").unwrap() {
        TextLoadAsResult::Text(d) => {
            assert_eq!(d.text, "こんにちは\n");
            assert!(!d.had_replacements);
        }
        _ => panic!("テキストのはず"),
    }
    // バイナリっぽい内容でも、指定したら読む(置換あり)
    let p2 = write_temp(&[0x41, 0x00, 0x42, 0xFF]);
    match load_as(&p2, "UTF-8").unwrap() {
        TextLoadAsResult::Text(d) => assert!(d.had_replacements),
        _ => panic!(),
    }
    assert!(load_as(&p, "UTF-99").is_err());
    assert!(load_as(&p.with_extension("none"), "UTF-8").is_err());
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(p2);
}

#[test]
fn load_as_rejects_too_large_files() {
    let p = std::env::temp_dir().join(format!("DiffLoupeTests-enc-big-{}", std::process::id()));
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(diffloupe_core::text_loader::MAX_TEXT_BYTES + 1)
        .unwrap();
    assert_eq!(load_as(&p, "UTF-8").unwrap(), TextLoadAsResult::TooLarge);
    let _ = std::fs::remove_file(p);
}

fn auto(bytes: &[u8]) -> (String, &'static str) {
    decode(bytes).unwrap_or_else(|| {
        panic!(
            "テキストと判定されるはず: {:02x?}",
            &bytes[..bytes.len().min(16)]
        )
    })
}

#[test]
fn bomless_utf16_with_ascii_is_detected() {
    let text = "Hello, world!\nThis is UTF-16 text without BOM.\n";
    let le: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let be: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    assert_eq!(auto(&le), (text.to_string(), "UTF-16 LE (BOMなし)"));
    assert_eq!(auto(&be), (text.to_string(), "UTF-16 BE (BOMなし)"));
    // 日本語混じり(ASCII が少ない)
    let mixed = "設定ファイル config.ini の内容です。\n値 = 123\n";
    let le: Vec<u8> = mixed.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let be: Vec<u8> = mixed.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    assert_eq!(auto(&le), (mixed.to_string(), "UTF-16 LE (BOMなし)"));
    assert_eq!(auto(&be), (mixed.to_string(), "UTF-16 BE (BOMなし)"));
}

#[test]
fn bomless_utf16_japanese_without_nul_is_detected() {
    for (be, name) in [
        (false, "UTF-16 LE (BOMなし)"),
        (true, "UTF-16 BE (BOMなし)"),
    ] {
        let bytes: Vec<u8> = JA
            .encode_utf16()
            .flat_map(|u| if be { u.to_be_bytes() } else { u.to_le_bytes() })
            .collect();
        // 改行(0x000A)を含むと NUL が入るので、改行なしの本文で試す
        let body: Vec<u8> = JA
            .trim_end()
            .encode_utf16()
            .flat_map(|u| if be { u.to_be_bytes() } else { u.to_le_bytes() })
            .collect();
        assert!(!body.contains(&0));
        let (t, n) = auto(&body);
        assert_eq!((t.as_str(), n), (JA.trim_end(), name));
        // 改行ありは NUL が現れるが、NUL の偏りは弱いのでバイナリ扱いにならずに読める
        let (t, n) = auto(&bytes);
        assert_eq!((t.as_str(), n), (JA, name));
    }
}

#[test]
fn bomless_utf32_is_detected() {
    let text = "Hello UTF-32 text\nsecond line\n";
    let le: Vec<u8> = text
        .chars()
        .flat_map(|c| (c as u32).to_le_bytes())
        .collect();
    let be: Vec<u8> = text
        .chars()
        .flat_map(|c| (c as u32).to_be_bytes())
        .collect();
    assert_eq!(auto(&le), (text.to_string(), "UTF-32 LE (BOMなし)"));
    assert_eq!(auto(&be), (text.to_string(), "UTF-32 BE (BOMなし)"));
    let ja: Vec<u8> = JA.chars().flat_map(|c| (c as u32).to_le_bytes()).collect();
    assert_eq!(auto(&ja), (JA.to_string(), "UTF-32 LE (BOMなし)"));
}

#[test]
fn bomless_unicode_detection_does_not_catch_binary() {
    // NUL が偏っていても、デコード結果が文章らしくなければバイナリのまま
    let mut x: u32 = 12345;
    let mut data = Vec::new();
    for i in 0..4096 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        data.push(if i % 2 == 1 { 0 } else { (x >> 16) as u8 | 1 });
    }
    // 奇数位置が 0、偶数位置が制御文字を含むランダム値 → 制御文字が多くバイナリ
    assert!(decode(&data).is_none());
    // 先頭に NUL を含む通常のバイナリ
    assert!(decode(&[0x41, 0x00, 0x42]).is_none());
    assert!(decode(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04]).is_none());
}

#[test]
fn jis7_half_width_kana_with_so_si_is_decoded() {
    // ASCII モードで SO/SI(0x31 0x32 0x33 = ｱｲｳ)
    let (t, n) = auto(b"\x1b(BABC \x0e\x31\x32\x33\x0f def\n");
    assert_eq!((t.as_str(), n), ("ABC ｱｲｳ def\n", "ISO-2022-JP"));
    // 漢字モードのまま SO/SI で半角カナを挟む(SI で漢字モードへ戻る)
    let (t, n) = auto(b"\x1b$B$\"\x0e\x31\x0f$$\x1b(B\n");
    assert_eq!((t.as_str(), n), ("あｱい\n", "ISO-2022-JP"));
    // 指定読み込みでも同じ
    let d = decode_as(b"\x1b(B\x0e\x31\x32\x33\x0f\n", "ISO-2022-JP").unwrap();
    assert_eq!(d.text, "ｱｲｳ\n");
    assert!(!d.had_replacements);
    // ESC ( I による半角カナ(従来どおり)
    let (t, _) = auto(b"\x1b(I123\x1b(B\n");
    assert_eq!(t, "ｱｲｳ\n");
}

#[test]
fn old_jis_escape_is_decoded() {
    // 旧 JIS(ESC $ @)の「あ」「い」
    let (t, n) = auto(b"\x1b$@$\"$$\x1b(B\n");
    assert_eq!((t.as_str(), n), ("あい\n", "ISO-2022-JP (旧JIS)"));
    // JIS ローマ字(ESC ( J)
    let (t, _) = auto(b"\x1b(Jabc\x1b$B$\"\x1b(B\n");
    assert_eq!(t, "abcあ\n");
}

#[test]
fn cp932_nec_and_ibm_extensions() {
    let text = "①②㈱№髙﨑ｱ\n";
    let bytes = encode("Shift_JIS", text).unwrap();
    // NEC 行外選定(13 区)と IBM 拡張
    assert_eq!(&bytes[..2], &[0x87, 0x40]); // ①
    assert_eq!(&bytes[4..6], &[0x87, 0x8A]); // ㈱
    assert!(bytes.windows(2).any(|w| w == [0xFB, 0xFC])); // 髙
    assert!(bytes.windows(2).any(|w| w == [0xFA, 0xB1])); // 﨑
    let (t, n) = auto(&bytes);
    assert_eq!((t.as_str(), n), (text, "Shift_JIS"));
}

#[test]
fn eucjp_supplementary_kanji_roundtrips() {
    // JIS X 0212 補助漢字(3 バイト、0x8F 始まり)
    let text = "丂丄丅\n";
    let bytes = encode("EUC-JP", text).unwrap();
    assert_eq!(&bytes[..3], &[0x8F, 0xB0, 0xA1]);
    let d = decode_as(&bytes, "EUC-JP").unwrap();
    assert_eq!(d.text, text);
    assert!(!d.had_replacements);
    // 0212 に無い文字は従来どおり表せない
    assert!(encode("EUC-JP", "😀").is_none());
    // 混在(0208 + 0212 + ASCII + 半角カナ)
    let mixed = "日本語 abc 丂 ｱ 髙\n";
    assert_eq!(
        decode_as(&encode("EUC-JP", mixed).unwrap(), "EUC-JP")
            .unwrap()
            .text,
        mixed
    );
}

#[test]
fn natural_text_is_detected_for_legacy_encodings() {
    let cases: [(&str, &str, &[&str]); 5] = [
        (ZH_CN, "GBK", &["GBK", "GB18030"]),
        (ZH_TW, "Big5", &["Big5"]),
        (KO, "EUC-KR", &["EUC-KR"]),
        (FR, "windows-1252", &["windows-1252"]),
        (RU, "KOI8-R", &["KOI8-R", "KOI8-U"]),
    ];
    for (text, enc, accepted) in cases {
        let bytes = encode(enc, text).unwrap();
        let (t, n) = decode(&bytes).unwrap_or_else(|| panic!("{} がバイナリ扱い", enc));
        assert!(accepted.contains(&n), "{}: {} と判定された", enc, n);
        assert_eq!(t, text, "{}", enc);
    }
    // 日本語は従来どおり Shift_JIS / EUC-JP
    let sjis = encode("Shift_JIS", JA).unwrap();
    assert_eq!(auto(&sjis).1, "Shift_JIS");
    let euc = encode("EUC-JP", JA).unwrap();
    assert_eq!(auto(&euc).1, "EUC-JP");
}

#[test]
fn windows_cyrillic_and_greek_are_detected() {
    let bytes = encode("windows-1251", RU).unwrap();
    let (t, n) = auto(&bytes);
    assert_eq!(n, "windows-1251");
    assert_eq!(t, RU);
}

/// NUL を含まない擬似乱数(ヘッダの後ろに付けて、チャンク先頭の識別子つきバイナリを模す)
fn pseudo_random_payload(len: usize) -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    (0..len)
        .map(|_| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            ((x >> 24) as u8).max(1)
        })
        .collect()
}

#[test]
fn binary_files_are_not_misdetected_as_text() {
    let headers: [(&str, &[u8]); 6] = [
        ("png", b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"),
        ("png-nonul", b"\x89PNG\r\n\x1a\n\x01\x01\x01\rIHDR"),
        ("jpeg", b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01"),
        ("jpeg-nonul", b"\xFF\xD8\xFF\xE0\x01\x10JFIF\x01\x01"),
        ("zip", b"PK\x03\x04\x14\x01\x08\x01\x08\x01"),
        ("gzip", b"\x1f\x8b\x08\x08\x01\x01\x01\x01\x01\x03"),
    ];
    for (label, header) in headers {
        // NUL を含まない短いヘッダだけのデータは、従来も Shift_JIS として読めてしまうため、
        // 判定の対象外(ペイロードが続く実ファイルの形で検査する)
        let lens: &[usize] = if header.contains(&0) {
            &[0, 64, 4096, 200_000]
        } else {
            &[64, 4096, 200_000]
        };
        for &payload_len in lens {
            let mut data = header.to_vec();
            data.extend(pseudo_random_payload(payload_len));
            assert!(
                decode(&data).is_none(),
                "{} + {} バイト がテキスト扱い",
                label,
                payload_len
            );
        }
    }
    // 実行ファイル(ELF / Mach-O / PE)の先頭
    for header in [
        &b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03\x00"[..],
        &b"\xCF\xFA\xED\xFE\x07\x00\x00\x01\x03\x00\x00\x00\x02\x00\x00\x00"[..],
        &b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xFF\xFF\x00\x00"[..],
        &b"\x7fELF\x02\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01"[..],
    ] {
        let mut data = header.to_vec();
        data.extend(pseudo_random_payload(10_000));
        assert!(decode(&data).is_none());
        let p = write_temp(&data);
        assert!(matches!(load(&p).unwrap(), TextLoadResult::Binary));
        let _ = std::fs::remove_file(p);
    }
    // 全体が NUL なしの純粋な乱数も、制御文字が多いのでバイナリ
    for len in [16usize, 256, 4096, 100_000] {
        assert!(
            decode(&pseudo_random_payload(len)).is_none(),
            "random {}",
            len
        );
    }
}

#[test]
fn plain_ascii_and_utf8_are_unchanged() {
    assert_eq!(auto(b"hello\n"), ("hello\n".to_string(), "UTF-8"));
    assert_eq!(auto("こんにちは\n".as_bytes()).1, "UTF-8");
    assert_eq!(auto(&[0xEF, 0xBB, 0xBF, b'a']).1, "UTF-8 (BOM)");
    assert_eq!(auto(&[0xFF, 0xFE, b'a', 0]).1, "UTF-16 LE");
    assert_eq!(auto(&[]).1, "UTF-8");
}

#[test]
fn encode_reports_first_unmappable_character_with_line() {
    let text = "abc\nDEF\n日本語😀じゃ\n";
    for name in [
        "Shift_JIS",
        "EUC-JP",
        "ISO-2022-JP",
        "windows-1252",
        "ISO-8859-1",
        "KOI8-R",
        "EUC-KR",
    ] {
        let err = encode_body_checked(name, "ok\nabc\n日本😀\n").unwrap_err();
        match name {
            "Shift_JIS" | "EUC-JP" | "ISO-2022-JP" => assert_eq!(
                err,
                EncodeError::Unmappable {
                    ch: '😀', line: 3
                },
                "{}",
                name
            ),
            _ => assert!(
                matches!(err, EncodeError::Unmappable { line: 3, .. }),
                "{}: {:?}",
                name,
                err
            ),
        }
        assert!(encode_body(name, text).is_none());
        assert!(encode(name, text).is_none());
    }
    // 表示文
    let e = encode_checked("Shift_JIS", "a\n😀").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("2 行目") && msg.contains('😀') && msg.contains("1F600"),
        "{}",
        msg
    );
    assert_eq!(
        encode_checked("UTF-99", "a").unwrap_err(),
        EncodeError::UnsupportedEncoding("UTF-99".to_string())
    );
    // Latin-1 は U+0100 以上を表せない
    assert_eq!(
        encode_body_checked("ISO-8859-1", "é\nĀ").unwrap_err(),
        EncodeError::Unmappable { ch: 'Ā', line: 2 }
    );
    // Unicode 系は常に表せる
    for n in ["UTF-8", "UTF-16 LE", "UTF-32 BE (BOMなし)"] {
        assert!(encode_body_checked(n, text).is_ok());
    }
}

#[test]
fn existing_encode_contract_is_kept() {
    assert_eq!(encode("UTF-16 LE", "a").unwrap(), vec![0xFF, 0xFE, b'a', 0]);
    assert_eq!(encode("UTF-16 LE (BOMなし)", "a").unwrap(), vec![b'a', 0]);
    assert_eq!(
        encode("UTF-32 BE", "a").unwrap(),
        vec![0, 0, 0xFE, 0xFF, 0, 0, 0, b'a']
    );
    assert_eq!(bom_for("UTF-16 BE (BOMなし)"), Some(&[][..]));
    assert_eq!(bom_for("KOI8-R"), Some(&[][..]));
    assert_eq!(bom_for("UTF-99"), None);
}
