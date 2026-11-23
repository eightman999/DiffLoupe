//! 短い CJK テキストの自動判定(GBK / Big5 / EUC-KR が日本語系に化けないこと)。

use diffloupe_core::text_loader::{decode, encode};

const ZH_CN: &str = "这是一个测试文件，用来检查编码的自动判断是否正常工作。第一行。\n";
const ZH_TW: &str = "這是一個測試檔案，用來檢查編碼的自動判斷是否正常運作。第一行。\n";
const KO: &str = "이것은 인코딩의 자동 판별이 올바르게 동작하는지 확인하는 시험 파일입니다.\n";

fn japanese_family(name: &str) -> bool {
    matches!(
        name,
        "Shift_JIS"
            | "EUC-JP"
            | "Shift_JIS-2004"
            | "EUC-JIS-2004"
            | "ISO-2022-JP"
            | "ISO-2022-JP-2004"
            | "ISO-2022-JP-2"
    )
}

fn prefix_bytes(enc: &str, text: &str, n: usize) -> Vec<u8> {
    // n バイト以内で、文字の途中で切れないように切る
    let mut out = String::new();
    for c in text.chars() {
        let mut next = out.clone();
        next.push(c);
        if encode(enc, &next).unwrap().len() > n {
            break;
        }
        out = next;
    }
    encode(enc, &out).unwrap()
}

#[test]
fn the_reported_63_byte_gbk_file_is_not_japanese() {
    let bytes = encode("GBK", ZH_CN).unwrap();
    assert_eq!(bytes.len(), 63);
    let r = decode(&bytes);
    println!("63 バイトの GBK -> {:?}", r.as_ref().map(|x| x.1));
    if let Some((t, n)) = r {
        assert!(!japanese_family(n), "日本語系に化けた: {}", n);
        assert_eq!((n, t.as_str()), ("GBK", ZH_CN));
    }
}

#[test]
fn big5_and_euc_kr_do_not_turn_into_japanese() {
    for (enc, text) in [("Big5", ZH_TW), ("EUC-KR", KO), ("GBK", ZH_CN)] {
        for n in 20..=encode(enc, text).unwrap().len() {
            let bytes = prefix_bytes(enc, text, n);
            if bytes.len() < 20 {
                continue;
            }
            if let Some((_, name)) = decode(&bytes) {
                assert!(
                    !japanese_family(name),
                    "{} の {} バイトが {} と判定された",
                    enc,
                    bytes.len(),
                    name
                );
            }
        }
    }
}

#[test]
fn cjk_encodings_are_detected_at_short_and_long_lengths() {
    let mut report = Vec::new();
    for (enc, text, accepted) in [
        ("GBK", ZH_CN, &["GBK", "gb18030"][..]),
        ("Big5", ZH_TW, &["Big5"][..]),
        ("EUC-KR", KO, &["EUC-KR"][..]),
    ] {
        let full = encode(enc, text).unwrap();
        for n in [20usize, 32, 48, 63, 64, 80, full.len()] {
            let bytes = prefix_bytes(enc, text, n.min(full.len()));
            let r = decode(&bytes).map(|x| x.1);
            report.push(format!("{}/{}B -> {:?}", enc, bytes.len(), r));
            if bytes.len() >= 64 {
                assert!(
                    r.is_some_and(|n| accepted.contains(&n)),
                    "{} {} バイト: {:?}",
                    enc,
                    bytes.len(),
                    r
                );
            }
        }
    }
    println!("{}", report.join("\n"));
}

#[test]
fn jis_x_0213_files_with_kana_are_still_detected() {
    let text = "日本語 A 𠮟𩸽か\u{309a}\n";
    let sjis = encode("Shift_JIS-2004", text).unwrap();
    assert_eq!(decode(&sjis).unwrap().1, "Shift_JIS-2004");
    let euc = encode("EUC-JIS-2004", text).unwrap();
    assert_eq!(decode(&euc).unwrap().1, "EUC-JIS-2004");
    // かな(半角カナ)を含まない JIS X 0213 の文字だけの列は、この段では採用しない
    let no_kana = encode("EUC-JIS-2004", "𠮟𩸽\n").unwrap();
    assert!(!matches!(decode(&no_kana), Some((_, "EUC-JIS-2004"))));
    // 半角カナだけでも採用する
    let half = encode("EUC-JIS-2004", "𩸽 ｱｲｳ\n").unwrap();
    assert_eq!(decode(&half).unwrap().1, "EUC-JIS-2004");
}

#[test]
fn short_random_bytes_are_not_adopted_as_cjk_text() {
    let mut x: u64 = 0x1234_5678_9ABC_DEF1;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut total = 0usize;
    let mut adopted = Vec::new();
    for len in 16..64usize {
        for _ in 0..2200 {
            let data: Vec<u8> = (0..len).map(|_| ((next() >> 24) as u8).max(1)).collect();
            total += 1;
            if let Some((_, name)) = decode(&data) {
                adopted.push((len, name));
            }
        }
    }
    let cjk: Vec<_> = adopted
        .iter()
        .filter(|(_, n)| matches!(*n, "GBK" | "gb18030" | "GB18030" | "Big5" | "EUC-KR"))
        .collect();
    let mut hist: std::collections::BTreeMap<&str, usize> = Default::default();
    for (_, n) in &adopted {
        *hist.entry(n).or_default() += 1;
    }
    println!("内訳 {:?}", hist);
    println!(
        "擬似乱数 {} 件: テキスト扱い {} 件(うち CJK {} 件) {:?}",
        total,
        adopted.len(),
        cjk.len(),
        &cjk[..cjk.len().min(5)]
    );
    assert!(cjk.is_empty(), "CJK として誤採用: {:?}", cjk);
}
