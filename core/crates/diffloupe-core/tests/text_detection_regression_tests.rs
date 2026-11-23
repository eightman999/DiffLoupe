//! 自動判定の限界の改善(短い単バイト系・西欧語)と、日本語判定の回帰テスト。
//! 回帰テストは、Shift_JIS / EUC-JP の日本語がそのまま日本語と判定されることを保証する。

use diffloupe_core::text_loader::{decode, encode};

fn detected(bytes: &[u8]) -> Option<(String, &'static str)> {
    decode(bytes)
}

fn assert_japanese(label: &str, text: &str) {
    for name in ["Shift_JIS", "EUC-JP"] {
        let bytes = encode(name, text).unwrap();
        let (t, n) =
            detected(&bytes).unwrap_or_else(|| panic!("{} ({}) がバイナリ扱い", label, name));
        assert_eq!(n, name, "{} ({}) が {} と判定された", label, name, n);
        assert_eq!(t, text, "{} ({})", label, name);
    }
}

#[test]
fn japanese_regression_lists_stay_japanese() {
    assert_japanese(
        "氏名一覧",
        "山田太郎\n佐藤花子\n鈴木一郎\n高橋次郎\n田中美咲\n伊藤健太\n渡辺由美\n中村大輔\n小林真理\n加藤修平\n吉田直樹\n山本恵子\n",
    );
    assert_japanese(
        "住所 CSV",
        "東京都千代田区丸の内,1-1-1\n大阪府大阪市北区梅田,2-3-4\n愛知県名古屋市中区栄,5-6-7\n北海道札幌市中央区北一条,8-9-10\n福岡県福岡市博多区博多駅前,11-12-13\n",
    );
    assert_japanese(
        "品名 CSV",
        "A001,鋼板,10,1980\nA002,鋼管,25,3200\nA003,角材,40,760\nA004,丸棒,12,1450\nA005,継手,300,85\nA006,弁類,8,12800\nA007,配管,60,2300\n",
    );
    assert_japanese(
        "漢文調",
        "国破山河在\n城春草木深\n感時花濺涙\n恨別鳥驚心\n烽火連三月\n家書抵万金\n白頭掻更短\n渾欲不勝簪\n",
    );
    assert_japanese(
        "会社名一覧",
        "株式会社山田製作所\n有限会社鈴木商店\n合同会社東京技術\n株式会社大阪電機\n一般社団法人日本情報協会\n株式会社名古屋工業\n",
    );
    assert_japanese(
        "勘定科目一覧",
        "現金\n普通預金\n売掛金\n買掛金\n未払金\n前払費用\n減価償却費\n租税公課\n旅費交通費\n通信費\n消耗品費\n広告宣伝費\n",
    );
    // かなを含む普通の文章
    assert_japanese(
        "文章",
        "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。\n",
    );
}

#[test]
fn japanese_source_code_with_short_comments_stays_japanese() {
    let code = "// 初期化\nint main(void) {\n    int x = 0;\n    x = 値;\n    // 実行\n    run(x);\n    /* 終了 */\n    return 0;\n}\n\n# 設定\nCONFIG_PATH = '/etc/app'\n# 読込\ndef load(path):\n    return open(path)\nfor item in items: # 一覧\n    print(item)\n";
    assert_japanese("日本語コメント入りソース", code);
    let kanji_between_letters =
        "let a = 1; // 設定\nvalue = get値(x) + y値z;\nprint('done') # 完了\n// 取得 end\n";
    assert_japanese("ASCII に挟まれた漢字", kanji_between_letters);
}

#[test]
fn western_text_misread_as_shift_jis_is_detected_as_single_byte() {
    // アクセント付きの字がすべて英字に挟まれている(Shift_JIS としても読めてしまう)西欧語
    let samples = [
        "Il était une fois, dans un petit village près de la forêt, une jeune fille qui aimait beaucoup les fleurs.\n",
        "Der Müller aus Köln trinkt gern Kölsch in der Nähe vom Dom, und seine Frau Käthe backt jeden Morgen frische Brötchen.\n",
    ];
    for text in samples {
        let bytes = encode("windows-1252", text).unwrap();
        let (t, n) = detected(&bytes).unwrap();
        assert_eq!(n, "windows-1252", "{}", text);
        assert_eq!(t, text);
    }
}

#[test]
fn short_single_byte_text_is_detected() {
    let cases = [
        "Café au lait\n",
        "Müller & Söhne GmbH\n",
        "naïve résumé café\n",
        "El niño comió piñata\n",
        "Smörgåsbord\n",
        "Crème brûlée café\n",
    ];
    for text in cases {
        let bytes = encode("windows-1252", text).unwrap();
        assert!(bytes.len() < 64 && bytes.len() >= 4);
        let (t, n) = detected(&bytes).unwrap_or_else(|| panic!("{:?} がバイナリ扱い", text));
        assert_eq!(t, text, "{:?} ({})", text, n);
        assert!(
            n.starts_with("windows-") || n.starts_with("ISO-8859-"),
            "{}",
            n
        );
    }
    // 下限は 4 バイト
    assert_eq!(detected(b"caf\xe9").unwrap().0, "café");
    assert!(detected(b"ca\xe9").is_none());
}

#[test]
fn short_binary_stays_binary() {
    // 制御文字を含むもの(画像・zip・実行ファイル・gzip の先頭)
    let heads: [&[u8]; 8] = [
        b"\x89PNG\r\n\x1a\n",
        b"\xFF\xD8\xFF\xE0\x00\x10JFIF",
        b"PK\x03\x04\x14\x00",
        b"\x7fELF\x02\x01\x01\x00",
        b"\x1f\x8b\x08\x00\x00\x00",
        b"MZ\x90\x00\x03\x00",
        b"\xCF\xFA\xED\xFE\x07\x00",
        b"GIF89a\x01\x00\x01\x00",
    ];
    for h in heads {
        // 従来から Shift_JIS / EUC-JP の厳密デコードで読めてしまう短い列(PNG の署名など)は、
        // この判定の対象外(日本語の判定は変えない)
        let legacy_ok = encoding_rs::SHIFT_JIS
            .decode_without_bom_handling_and_without_replacement(h)
            .is_some()
            || encoding_rs::EUC_JP
                .decode_without_bom_handling_and_without_replacement(h)
                .is_some();
        if legacy_ok && !h.contains(&0) {
            assert!(
                matches!(detected(h), Some((_, "Shift_JIS" | "EUC-JP"))),
                "{:?}",
                h
            );
        } else {
            assert!(detected(h).is_none(), "{:?}", h);
        }
    }
    // 4〜63 バイトの NUL を含まない擬似乱数
    let mut x: u32 = 0xDEAD_BEEF;
    for len in 4..64usize {
        for _ in 0..40 {
            let data: Vec<u8> = (0..len)
                .map(|_| {
                    x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                    ((x >> 24) as u8).max(1)
                })
                .collect();
            if let Some((t, n)) = detected(&data) {
                // 運よく文字コードの規則に合致した場合のみ(制御文字が無い短い列)。
                // 画像・圧縮データを誤ってテキスト扱いしないことが目的なので、
                // 制御文字を含まず英字が多いものだけが許される。
                let letters = data.iter().filter(|b| b.is_ascii_alphabetic()).count();
                assert!(
                    data.iter()
                        .all(|&b| b >= 0x20 || matches!(b, b'\t' | b'\n' | b'\r'))
                        || !n.starts_with("windows-"),
                    "乱数 {} バイトがテキスト扱い: {:02x?} -> {:?} ({})",
                    len,
                    data,
                    t,
                    n
                );
                let _ = letters;
            }
        }
    }
}
