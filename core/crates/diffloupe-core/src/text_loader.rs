//! テキスト読み込み・エンコード判定(Swift `TextFileLoader` 相当)
//!
//! 動作設計:
//! - 自動判定(`decode` / `load`)の順序:
//!   1. BOM(UTF-32 → UTF-8 → UTF-16)
//!   2. BOM なし UTF-32 / UTF-16(ASCII 主体で NUL が一方の位置に強く偏るもの。NUL 判定より前)
//!   3. 先頭 8192 バイトに NUL があればバイナリ
//!   4. ISO-2022-JP(7bit で ESC を含む。SO/SI による半角カナ JIS7 も読む)
//!   5. UTF-8
//!   6. Shift_JIS(Windows-31J)→ EUC-JP。ただし、ひらがな・カタカナを含まず非 ASCII が 30% 以上の
//!      64 バイト以上のデータは、chardetng が日本語以外(GBK・Big5・EUC-KR・KOI8-R など)と推定し、
//!      エラーなくデコードできるならそちらを採る(それらは日本語としても誤ってデコードできるため)
//!   7. BOM なし UTF-16(NUL を含まない日本語主体のもの。かな・全角の割合で向きを決める)
//!   8. chardetng による推定(64 バイト以上。単バイト系・GBK・Big5・EUC-KR など)。推定した文字コードで
//!      エラーなくデコードでき、制御文字(C0 のうちタブ・改行・復帰・改ページ・ESC 以外と C1)が
//!      0.1% 未満のときだけテキスト扱い。単バイト系はどんなバイト列でもデコードできてしまうため、
//!      この制御文字の検査が画像・実行ファイル・圧縮データを誤ってテキスト扱いしない要になる。
//!   9. いずれも当たらなければバイナリ
//! - 手動指定(`decode_as` / `load_as`)は、ユーザーが明示した文字コードで読む。不正なバイト列は
//!   U+FFFD に置換して続行し、置換が起きたかを返す(置換ありのまま保存すると壊れるため、
//!   呼び出し側は保存を禁止すること)。対応する文字コードは `supported_encodings` の一覧。
//! - 符号化(`encode` 系)は一覧の全文字コードに対応する。表せない文字があれば最初の文字と
//!   行番号つきのエラー(`encode_checked` / `encode_body_checked`)。従来の `Option` 版は互換用。
//! - 未対応: Shift_JIS-2004 / EUC-JIS-2004 / ISO-2022-JP-2・-3 / JIS X 0212 の ISO-2022 指示 /
//!   EBCDIC / MacJapanese / UTF-7 / 8bit 版 JIS(ISO-2022-JP-8)。

use crate::legacy_codecs::{self as legacy, CustomCodec};
use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::Encoding;
use std::borrow::Cow;
use std::path::Path;

pub const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;
pub const BINARY_SNIFF_LENGTH: usize = 8192;

pub enum TextLoadResult {
    Text(String, &'static str),
    Binary,
    TooLarge,
}

pub fn load(path: &Path) -> std::io::Result<TextLoadResult> {
    if let Ok(md) = std::fs::metadata(path) {
        if md.len() > MAX_TEXT_BYTES {
            return Ok(TextLoadResult::TooLarge);
        }
    }
    let data = std::fs::read(path)?;
    if data.len() as u64 > MAX_TEXT_BYTES {
        return Ok(TextLoadResult::TooLarge);
    }
    if data.is_empty() {
        return Ok(TextLoadResult::Text(String::new(), "UTF-8"));
    }
    Ok(match decode(&data) {
        Some((t, name)) => TextLoadResult::Text(t, name),
        None => TextLoadResult::Binary,
    })
}

/// バイト列 → (テキスト, エンコード名)。全判定失敗で None。
/// 判定順はファイル先頭の動作設計コメントを参照。
pub fn decode(data: &[u8]) -> Option<(String, &'static str)> {
    if let Some(r) = decode_by_bom(data) {
        return Some(r);
    }
    if let Some(r) = detect_bomless_unicode_with_nul(data) {
        return Some(r);
    }
    if data[..data.len().min(BINARY_SNIFF_LENGTH)].contains(&0) {
        return None;
    }
    if looks_like_iso2022jp(data) {
        let converted = jis7_to_iso2022jp(data);
        if let Some(t) = encoding_rs::ISO_2022_JP
            .decode_without_bom_handling_and_without_replacement(&converted)
            .map(|c| c.into_owned())
        {
            return Some((t, iso2022jp_variant_name(data)));
        }
        // ISO-2022-JP として読めず、JIS X 0213(-2004)や -2 の指示を含むものは、それらとして読む
        if let Some(r) = detect_iso2022jp_extensions(data) {
            return Some(r);
        }
    }
    if let Ok(t) = std::str::from_utf8(data) {
        return Some((t.to_string(), "UTF-8"));
    }
    if let Some(r) = decode_japanese_legacy(data) {
        // かなを含まない文は、中国語・韓国語・キリル文字などの文字コードでも日本語としてデコード
        // できてしまう(EUC-JP は 2 バイトの範囲が GB2312・KS X 1001 と重なり、KOI8-R の小文字は
        // Shift_JIS の半角カナの範囲)。そこだけ chardetng の推定で上書きする。
        if let Some(other) = detect_non_japanese_override(data, &r.0) {
            return Some(other);
        }
        // 西欧語(アクセント付きの字が英字に挟まれる)は Shift_JIS として読めてしまうことがある
        if let Some(west) = detect_western_override(data, &r.0) {
            return Some(west);
        }
        return Some(r);
    }
    // JIS X 0213 の字(第 3・第 4 水準など)を含む Shift_JIS-2004 / EUC-JIS-2004
    // (中国語・韓国語の文字コードも、表の広い EUC-JIS-2004 では読めてしまうため、上と同じ上書きを適用する)
    if let Some(r) = decode_jis2004_legacy(data) {
        if let Some(other) = detect_non_japanese_override(data, &r.0) {
            return Some(other);
        }
        if let Some(west) = detect_western_override(data, &r.0) {
            return Some(west);
        }
        return Some(r);
    }
    if let Some(r) = detect_bomless_utf16_japanese(data) {
        return Some(r);
    }
    detect_with_chardetng(data)
        .or_else(|| detect_short_cjk(data))
        .or_else(|| detect_short_single_byte(data))
}

/// `decode` が返すエンコード名に対応する BOM。BOM を付けない名前は空、未対応の名前は None。
pub fn bom_for(name: &str) -> Option<&'static [u8]> {
    find_entry(name).map(|e| e.bom)
}

/// 符号化の失敗
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// 未対応の文字コード名
    UnsupportedEncoding(String),
    /// その文字コードで表せない文字(最初の 1 文字と、その行番号。1 始まり、LF 区切り)
    Unmappable { ch: char, line: usize },
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::UnsupportedEncoding(n) => write!(f, "未対応の文字コードです: {}", n),
            EncodeError::Unmappable { ch, line } => write!(
                f,
                "{} 行目の「{}」(U+{:04X})は、この文字コードで表せません",
                line, ch, *ch as u32
            ),
        }
    }
}
impl std::error::Error for EncodeError {}

fn line_of(text: &str, byte_index: usize) -> usize {
    text.as_bytes()[..byte_index]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// `decode` の逆変換: テキストを `name` で符号化する(BOM は付けない)。
/// 表せない最初の文字と行番号を `Err` で返す。
pub fn encode_body_checked(name: &str, text: &str) -> Result<Vec<u8>, EncodeError> {
    let entry =
        find_entry(name).ok_or_else(|| EncodeError::UnsupportedEncoding(name.to_string()))?;
    match entry.codec {
        Codec::Utf8 => Ok(text.as_bytes().to_vec()),
        Codec::Utf16 { be } => Ok(text
            .encode_utf16()
            .flat_map(|u| if be { u.to_be_bytes() } else { u.to_le_bytes() })
            .collect()),
        Codec::Utf32 { be } => Ok(text
            .chars()
            .flat_map(|c| {
                if be {
                    (c as u32).to_be_bytes()
                } else {
                    (c as u32).to_le_bytes()
                }
            })
            .collect()),
        Codec::Latin1 => {
            let mut out = Vec::with_capacity(text.len());
            for (i, c) in text.char_indices() {
                if (c as u32) > 0xFF {
                    return Err(EncodeError::Unmappable {
                        ch: c,
                        line: line_of(text, i),
                    });
                }
                out.push(c as u8);
            }
            Ok(out)
        }
        Codec::Rs(enc) => {
            let (bytes, _, had_unmappable) = enc.encode(text);
            if !had_unmappable {
                return Ok(bytes.into_owned());
            }
            // encoding_rs の EUC-JP 符号化は JIS X 0212 補助漢字を出さないため、自前の表で補う
            if std::ptr::eq(enc, encoding_rs::EUC_JP) {
                return encode_eucjp_with_supplementary(text).map_err(|(ch, i)| {
                    EncodeError::Unmappable {
                        ch,
                        line: line_of(text, i),
                    }
                });
            }
            // 失敗時だけ 1 文字ずつ調べて、最初の表せない文字を探す
            let mut buf = [0u8; 4];
            for (i, c) in text.char_indices() {
                let (_, _, bad) = enc.encode(c.encode_utf8(&mut buf));
                if bad {
                    return Err(EncodeError::Unmappable {
                        ch: c,
                        line: line_of(text, i),
                    });
                }
            }
            Err(EncodeError::UnsupportedEncoding(name.to_string()))
        }
        Codec::Iso2022Jp { old_kanji, roman } => {
            encode_iso2022jp(text, old_kanji, roman).map_err(|(c, i)| EncodeError::Unmappable {
                ch: c,
                line: line_of(text, i),
            })
        }
        Codec::Custom(c) => (c.encode)(text).map_err(|(ch, i)| EncodeError::Unmappable {
            ch,
            line: line_of(text, i),
        }),
    }
}

/// JIS X 0212 の補助漢字の符号(EUC-JP の 0x8F 始まり 3 バイト)。`legacy_codecs` の ISO-2022-JP-2 が使う。
pub(crate) fn jis0212_lookup(c: char) -> Option<[u8; 3]> {
    jis0212_table().get(&c).copied()
}

/// JIS X 0212 補助漢字(EUC-JP の 0x8F 始まり 3 バイト)の逆引き表。encoding_rs のデコーダで
/// 全 94x94 の符号位置を復号して作る(初回のみ)。
fn jis0212_table() -> &'static std::collections::HashMap<char, [u8; 3]> {
    use std::sync::OnceLock;
    static TABLE: OnceLock<std::collections::HashMap<char, [u8; 3]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for b1 in 0xA1u8..=0xFE {
            for b2 in 0xA1u8..=0xFE {
                let seq = [0x8F, b1, b2];
                if let Some(t) =
                    encoding_rs::EUC_JP.decode_without_bom_handling_and_without_replacement(&seq)
                {
                    let mut it = t.chars();
                    if let (Some(c), None) = (it.next(), it.next()) {
                        // 先に登録されたもの(小さい符号位置)を優先する
                        map.entry(c).or_insert(seq);
                    }
                }
            }
        }
        map
    })
}

/// EUC-JP + JIS X 0212。表せない最初の文字と、そのバイト位置を Err で返す。
fn encode_eucjp_with_supplementary(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut buf = [0u8; 4];
    for (i, c) in text.char_indices() {
        let (bytes, _, bad) = encoding_rs::EUC_JP.encode(c.encode_utf8(&mut buf));
        if !bad {
            out.extend_from_slice(&bytes);
        } else if let Some(seq) = jis0212_table().get(&c) {
            out.extend_from_slice(seq);
        } else {
            return Err((c, i));
        }
    }
    Ok(out)
}

/// `decode` の逆変換: テキストを `name`(`decode` が返す名前)で符号化する(BOM は付けない)。
/// 未対応の名前・表現できない文字があるときは None。詳細が要るときは `encode_body_checked`。
pub fn encode_body(name: &str, text: &str) -> Option<Vec<u8>> {
    encode_body_checked(name, text).ok()
}

/// `decode` の逆変換: BOM(該当する名前のみ)+本文。`decode(&encode(n, t)?)` は `(t, n)` に戻る
/// (ISO-2022-JP / Shift_JIS 等は内容が判定可能な場合)。未対応・表現不可は None。
pub fn encode(name: &str, text: &str) -> Option<Vec<u8>> {
    encode_checked(name, text).ok()
}

/// `encode` のエラー詳細つき版
pub fn encode_checked(name: &str, text: &str) -> Result<Vec<u8>, EncodeError> {
    let bom = bom_for(name).ok_or_else(|| EncodeError::UnsupportedEncoding(name.to_string()))?;
    let mut out = bom.to_vec();
    out.extend(encode_body_checked(name, text)?);
    Ok(out)
}

/// ISO-2022-JP(JIS X 0208 + JIS X 0201 カナ)。encoding_rs の ISO-2022-JP は符号化側が UTF-8 に
/// なる(WHATWG 仕様)ため使えない。EUC-JP の符号化結果から 7bit へ変換して組み立てる。
/// Swift(`String.Encoding.iso2022JP`)と同じく、2バイト文字は `ESC $ B`、半角カナは `ESC ( I`、
/// ASCII へ戻るとき `ESC ( B`、末尾は ASCII モードで閉じる。JIS X 0212 は表現不可として扱う。
///
/// `old_kanji` なら 2 バイト文字の指示を旧 JIS(`ESC $ @`)にする。`roman` なら、2 バイト文字・半角カナ
/// から ASCII へ戻るとき `ESC ( J`(JIS ローマ字。0x5C = ¥、0x7E = ‾)を使い、U+00A5 / U+203E も書ける
/// (`\` と `~` は表せないので、その前では `ESC ( B` に戻す)。開いたときと同じ指示で保存するための指定。
fn encode_iso2022jp(text: &str, old_kanji: bool, roman: bool) -> Result<Vec<u8>, (char, usize)> {
    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
        Ascii,
        Roman,
        Kanji,
        Kana,
    }
    const ESC_ASCII: &[u8] = b"\x1b(B";
    const ESC_ROMAN: &[u8] = b"\x1b(J";
    let esc_kanji: &[u8] = if old_kanji { b"\x1b$@" } else { b"\x1b$B" };
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut mode = Mode::Ascii;
    let mut buf = [0u8; 4];
    for (idx, ch) in text.char_indices() {
        let want;
        let mut bytes: [u8; 2] = [0, 0];
        let len;
        let u = ch as u32;
        if u < 0x80 {
            let needs_ascii = ch == '\\' || ch == '~';
            want = match mode {
                Mode::Ascii => Mode::Ascii,
                Mode::Roman if !needs_ascii => Mode::Roman,
                Mode::Roman => Mode::Ascii,
                // 2 バイト文字・半角カナから戻るときの行き先
                Mode::Kanji | Mode::Kana if roman && !needs_ascii => Mode::Roman,
                _ => Mode::Ascii,
            };
            bytes[0] = u as u8;
            len = 1;
        } else if roman && (ch == '\u{A5}' || ch == '\u{203E}') {
            want = Mode::Roman;
            bytes[0] = if ch == '\u{A5}' { 0x5C } else { 0x7E };
            len = 1;
        } else {
            let s = ch.encode_utf8(&mut buf);
            let (enc, _, bad) = encoding_rs::EUC_JP.encode(s);
            if bad {
                return Err((ch, idx));
            }
            match enc.as_ref() {
                [0x8E, k] if (0xA1..=0xDF).contains(k) => {
                    want = Mode::Kana;
                    bytes[0] = k & 0x7F;
                    len = 1;
                }
                [a, b] if (0xA1..=0xFE).contains(a) && (0xA1..=0xFE).contains(b) => {
                    want = Mode::Kanji;
                    bytes = [a & 0x7F, b & 0x7F];
                    len = 2;
                }
                _ => return Err((ch, idx)),
            }
        }
        if want != mode {
            out.extend_from_slice(match want {
                Mode::Ascii => ESC_ASCII,
                Mode::Roman => ESC_ROMAN,
                Mode::Kanji => esc_kanji,
                Mode::Kana => b"\x1b(I",
            });
            mode = want;
        }
        out.extend_from_slice(&bytes[..len]);
    }
    // 2 バイト文字・半角カナのままなら ASCII(JIS ローマ字指定なら ESC ( J)へ戻して閉じる
    if matches!(mode, Mode::Kanji | Mode::Kana) {
        out.extend_from_slice(if roman { ESC_ROMAN } else { ESC_ASCII });
    }
    Ok(out)
}

fn decode_by_bom(data: &[u8]) -> Option<(String, &'static str)> {
    let b = data;
    if b.len() >= 4 && b[0] == 0x00 && b[1] == 0x00 && b[2] == 0xFE && b[3] == 0xFF {
        return decode_utf32(&b[4..], true).map(|t| (t, "UTF-32 BE"));
    }
    if b.len() >= 4 && b[0] == 0xFF && b[1] == 0xFE && b[2] == 0x00 && b[3] == 0x00 {
        return decode_utf32(&b[4..], false).map(|t| (t, "UTF-32 LE"));
    }
    if b.len() >= 3 && b[0] == 0xEF && b[1] == 0xBB && b[2] == 0xBF {
        return std::str::from_utf8(&b[3..])
            .ok()
            .map(|t| (t.to_string(), "UTF-8 (BOM)"));
    }
    if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
        return decode_utf16(&b[2..], true).map(|t| (t, "UTF-16 BE"));
    }
    if b.len() >= 2 && b[0] == 0xFF && b[1] == 0xFE {
        return decode_utf16(&b[2..], false).map(|t| (t, "UTF-16 LE"));
    }
    None
}

fn decode_utf16(data: &[u8], big_endian: bool) -> Option<String> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            if big_endian {
                u16::from_be_bytes(*c)
            } else {
                u16::from_le_bytes(*c)
            }
        })
        .collect();
    String::from_utf16(&units).ok()
}

fn decode_utf32(data: &[u8], big_endian: bool) -> Option<String> {
    if !data.len().is_multiple_of(4) {
        return None;
    }
    let mut out = String::with_capacity(data.len() / 4);
    for c in data.as_chunks::<4>().0 {
        let cp = if big_endian {
            u32::from_be_bytes(*c)
        } else {
            u32::from_le_bytes(*c)
        };
        out.push(char::from_u32(cp)?);
    }
    Some(out)
}

fn looks_like_iso2022jp(data: &[u8]) -> bool {
    let mut found = false;
    for &b in data {
        if b > 0x7F {
            return false;
        }
        if b == 0x1B {
            found = true;
        }
    }
    found
}

/// Shift_JIS → EUC-JP の順で厳密デコードを試す
/// (NSString.stringEncoding 検出の近似: 両方デコード可能な曖昧入力は
///  Shift_JIS を優先する)
fn decode_japanese_legacy(data: &[u8]) -> Option<(String, &'static str)> {
    if let Some(t) = encoding_rs::SHIFT_JIS
        .decode_without_bom_handling_and_without_replacement(data)
        .map(|c| c.into_owned())
    {
        return Some((t, "Shift_JIS"));
    }
    if let Some(t) = encoding_rs::EUC_JP
        .decode_without_bom_handling_and_without_replacement(data)
        .map(|c| c.into_owned())
    {
        return Some((t, "EUC-JP"));
    }
    None
}

// ==================== 文字コード一覧 ====================

#[derive(Clone, Copy)]
enum Codec {
    Utf8,
    Utf16 {
        be: bool,
    },
    Utf32 {
        be: bool,
    },
    /// ISO-8859-1(0x00-0xFF をそのまま U+0000-U+00FF へ)
    Latin1,
    /// encoding_rs が対応する文字コード
    Rs(&'static Encoding),
    /// ISO-2022-JP(復号は encoding_rs、符号化は自前)。旧 JIS(ESC $ @)・JIS ローマ字(ESC ( J)を
    /// 使うファイルは、保存でも同じ指示を使うため別名で区別する。
    Iso2022Jp {
        old_kanji: bool,
        roman: bool,
    },
    /// 表・状態機械による自前実装(`legacy_codecs`)
    Custom(&'static CustomCodec),
}

struct Entry {
    name: &'static str,
    display_name: &'static str,
    group: &'static str,
    codec: Codec,
    bom: &'static [u8],
}

const NO_BOM: &[u8] = &[];

const fn entry(
    name: &'static str,
    display_name: &'static str,
    group: &'static str,
    codec: Codec,
) -> Entry {
    Entry {
        name,
        display_name,
        group,
        codec,
        bom: NO_BOM,
    }
}

const fn entry_bom(
    name: &'static str,
    display_name: &'static str,
    group: &'static str,
    codec: Codec,
    bom: &'static [u8],
) -> Entry {
    Entry {
        name,
        display_name,
        group,
        codec,
        bom,
    }
}

/// 表示順の一覧(Unicode → 日本語 → 中国語 → 韓国語 → 西欧 → 中欧・東欧 → キリル → その他)。
/// 名前は `decode` が返す名前と同じ。encoding_rs 由来のものは encoding_rs の正規名(gb18030 のみ大文字)。
static TABLE: &[Entry] = &[
    entry("UTF-8", "Unicode (UTF-8)", "Unicode", Codec::Utf8),
    entry_bom(
        "UTF-8 (BOM)",
        "Unicode (UTF-8、BOM付き)",
        "Unicode",
        Codec::Utf8,
        &[0xEF, 0xBB, 0xBF],
    ),
    entry_bom(
        "UTF-16 LE",
        "Unicode (UTF-16 LE、BOM付き)",
        "Unicode",
        Codec::Utf16 { be: false },
        &[0xFF, 0xFE],
    ),
    entry_bom(
        "UTF-16 BE",
        "Unicode (UTF-16 BE、BOM付き)",
        "Unicode",
        Codec::Utf16 { be: true },
        &[0xFE, 0xFF],
    ),
    entry(
        "UTF-16 LE (BOMなし)",
        "Unicode (UTF-16 LE、BOMなし)",
        "Unicode",
        Codec::Utf16 { be: false },
    ),
    entry(
        "UTF-16 BE (BOMなし)",
        "Unicode (UTF-16 BE、BOMなし)",
        "Unicode",
        Codec::Utf16 { be: true },
    ),
    entry_bom(
        "UTF-32 LE",
        "Unicode (UTF-32 LE、BOM付き)",
        "Unicode",
        Codec::Utf32 { be: false },
        &[0xFF, 0xFE, 0x00, 0x00],
    ),
    entry_bom(
        "UTF-32 BE",
        "Unicode (UTF-32 BE、BOM付き)",
        "Unicode",
        Codec::Utf32 { be: true },
        &[0x00, 0x00, 0xFE, 0xFF],
    ),
    entry(
        "UTF-32 LE (BOMなし)",
        "Unicode (UTF-32 LE、BOMなし)",
        "Unicode",
        Codec::Utf32 { be: false },
    ),
    entry(
        "UTF-32 BE (BOMなし)",
        "Unicode (UTF-32 BE、BOMなし)",
        "Unicode",
        Codec::Utf32 { be: true },
    ),
    entry(
        "UTF-7",
        "Unicode (UTF-7)",
        "Unicode",
        Codec::Custom(&legacy::UTF_7),
    ),
    entry(
        "Shift_JIS",
        "日本語 (Shift_JIS / Windows-31J)",
        "日本語",
        Codec::Rs(encoding_rs::SHIFT_JIS),
    ),
    entry(
        "EUC-JP",
        "日本語 (EUC-JP)",
        "日本語",
        Codec::Rs(encoding_rs::EUC_JP),
    ),
    entry(
        "Shift_JIS-2004",
        "日本語 (Shift_JIS-2004 / JIS X 0213)",
        "日本語",
        Codec::Custom(&legacy::SHIFT_JIS_2004),
    ),
    entry(
        "EUC-JIS-2004",
        "日本語 (EUC-JIS-2004 / JIS X 0213)",
        "日本語",
        Codec::Custom(&legacy::EUC_JIS_2004),
    ),
    entry(
        "ISO-2022-JP",
        "日本語 (ISO-2022-JP / JIS)",
        "日本語",
        Codec::Iso2022Jp {
            old_kanji: false,
            roman: false,
        },
    ),
    entry(
        "ISO-2022-JP (旧JIS)",
        "日本語 (ISO-2022-JP、旧JIS ESC $ @ で保存)",
        "日本語",
        Codec::Iso2022Jp {
            old_kanji: true,
            roman: false,
        },
    ),
    entry(
        "ISO-2022-JP (JISローマ字)",
        "日本語 (ISO-2022-JP、JISローマ字 ESC ( J で保存)",
        "日本語",
        Codec::Iso2022Jp {
            old_kanji: false,
            roman: true,
        },
    ),
    entry(
        "ISO-2022-JP (旧JIS・JISローマ字)",
        "日本語 (ISO-2022-JP、旧JIS・JISローマ字で保存)",
        "日本語",
        Codec::Iso2022Jp {
            old_kanji: true,
            roman: true,
        },
    ),
    entry(
        "ISO-2022-JP-2004",
        "日本語 (ISO-2022-JP-2004 / JIS X 0213)",
        "日本語",
        Codec::Custom(&legacy::ISO_2022_JP_2004),
    ),
    entry(
        "ISO-2022-JP-2",
        "日本語・多言語 (ISO-2022-JP-2)",
        "日本語",
        Codec::Custom(&legacy::ISO_2022_JP_2),
    ),
    entry(
        "MacJapanese",
        "日本語 (Mac 日本語)",
        "日本語",
        Codec::Custom(&legacy::MAC_JAPANESE),
    ),
    entry(
        "IBM930",
        "日本語 EBCDIC (IBM930 カタカナ拡張 + 漢字)",
        "日本語",
        Codec::Custom(&legacy::IBM930),
    ),
    entry(
        "IBM939",
        "日本語 EBCDIC (IBM939 英小文字拡張 + 漢字)",
        "日本語",
        Codec::Custom(&legacy::IBM939),
    ),
    entry(
        "IBM290",
        "日本語 EBCDIC (IBM290 半角カナ)",
        "日本語",
        Codec::Custom(&legacy::IBM290),
    ),
    entry(
        "GB18030",
        "中国語 簡体字 (GB18030)",
        "中国語",
        Codec::Rs(encoding_rs::GB18030),
    ),
    entry(
        "GBK",
        "中国語 簡体字 (GBK)",
        "中国語",
        Codec::Rs(encoding_rs::GBK),
    ),
    entry(
        "Big5",
        "中国語 繁体字 (Big5)",
        "中国語",
        Codec::Rs(encoding_rs::BIG5),
    ),
    entry(
        "EUC-KR",
        "韓国語 (EUC-KR)",
        "韓国語",
        Codec::Rs(encoding_rs::EUC_KR),
    ),
    entry(
        "windows-1252",
        "西欧 (Windows-1252)",
        "西欧",
        Codec::Rs(encoding_rs::WINDOWS_1252),
    ),
    entry("ISO-8859-1", "西欧 (ISO-8859-1)", "西欧", Codec::Latin1),
    entry(
        "ISO-8859-15",
        "西欧 (ISO-8859-15)",
        "西欧",
        Codec::Rs(encoding_rs::ISO_8859_15),
    ),
    entry(
        "ISO-8859-14",
        "ケルト語 (ISO-8859-14)",
        "西欧",
        Codec::Rs(encoding_rs::ISO_8859_14),
    ),
    entry(
        "ISO-8859-10",
        "北欧 (ISO-8859-10)",
        "西欧",
        Codec::Rs(encoding_rs::ISO_8859_10),
    ),
    entry(
        "ISO-8859-3",
        "南欧 (ISO-8859-3)",
        "西欧",
        Codec::Rs(encoding_rs::ISO_8859_3),
    ),
    entry(
        "macintosh",
        "西欧 (Mac Roman)",
        "西欧",
        Codec::Rs(encoding_rs::MACINTOSH),
    ),
    entry(
        "windows-1250",
        "中欧 (Windows-1250)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::WINDOWS_1250),
    ),
    entry(
        "ISO-8859-2",
        "中欧 (ISO-8859-2)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::ISO_8859_2),
    ),
    entry(
        "ISO-8859-16",
        "東南欧 (ISO-8859-16)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::ISO_8859_16),
    ),
    entry(
        "windows-1257",
        "バルト語 (Windows-1257)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::WINDOWS_1257),
    ),
    entry(
        "ISO-8859-4",
        "バルト語 (ISO-8859-4)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::ISO_8859_4),
    ),
    entry(
        "ISO-8859-13",
        "バルト語 (ISO-8859-13)",
        "中欧・東欧",
        Codec::Rs(encoding_rs::ISO_8859_13),
    ),
    entry(
        "windows-1251",
        "キリル (Windows-1251)",
        "キリル",
        Codec::Rs(encoding_rs::WINDOWS_1251),
    ),
    entry(
        "ISO-8859-5",
        "キリル (ISO-8859-5)",
        "キリル",
        Codec::Rs(encoding_rs::ISO_8859_5),
    ),
    entry(
        "KOI8-R",
        "キリル (KOI8-R)",
        "キリル",
        Codec::Rs(encoding_rs::KOI8_R),
    ),
    entry(
        "KOI8-U",
        "キリル (KOI8-U)",
        "キリル",
        Codec::Rs(encoding_rs::KOI8_U),
    ),
    entry(
        "IBM866",
        "キリル (IBM866 / DOS)",
        "キリル",
        Codec::Rs(encoding_rs::IBM866),
    ),
    entry(
        "x-mac-cyrillic",
        "キリル (Mac)",
        "キリル",
        Codec::Rs(encoding_rs::X_MAC_CYRILLIC),
    ),
    entry(
        "windows-1253",
        "ギリシャ語 (Windows-1253)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_1253),
    ),
    entry(
        "ISO-8859-7",
        "ギリシャ語 (ISO-8859-7)",
        "その他",
        Codec::Rs(encoding_rs::ISO_8859_7),
    ),
    entry(
        "windows-1254",
        "トルコ語 (Windows-1254)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_1254),
    ),
    entry(
        "windows-1255",
        "ヘブライ語 (Windows-1255)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_1255),
    ),
    entry(
        "ISO-8859-8",
        "ヘブライ語 (ISO-8859-8 視覚順)",
        "その他",
        Codec::Rs(encoding_rs::ISO_8859_8),
    ),
    entry(
        "ISO-8859-8-I",
        "ヘブライ語 (ISO-8859-8-I 論理順)",
        "その他",
        Codec::Rs(encoding_rs::ISO_8859_8_I),
    ),
    entry(
        "windows-1256",
        "アラビア語 (Windows-1256)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_1256),
    ),
    entry(
        "ISO-8859-6",
        "アラビア語 (ISO-8859-6)",
        "その他",
        Codec::Rs(encoding_rs::ISO_8859_6),
    ),
    entry(
        "windows-874",
        "タイ語 (Windows-874)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_874),
    ),
    entry(
        "windows-1258",
        "ベトナム語 (Windows-1258)",
        "その他",
        Codec::Rs(encoding_rs::WINDOWS_1258),
    ),
    entry(
        "IBM037",
        "EBCDIC 米国・カナダ (IBM037)",
        "EBCDIC",
        Codec::Custom(&legacy::IBM037),
    ),
    entry(
        "IBM500",
        "EBCDIC 国際 (IBM500)",
        "EBCDIC",
        Codec::Custom(&legacy::IBM500),
    ),
    entry(
        "IBM1140",
        "EBCDIC 米国・ユーロ付き (IBM1140)",
        "EBCDIC",
        Codec::Custom(&legacy::IBM1140),
    ),
    entry(
        "IBM273",
        "EBCDIC ドイツ (IBM273)",
        "EBCDIC",
        Codec::Custom(&legacy::IBM273),
    ),
];

fn find_entry(name: &str) -> Option<&'static Entry> {
    TABLE
        .iter()
        .find(|e| e.name == name)
        .or_else(|| TABLE.iter().find(|e| e.name.eq_ignore_ascii_case(name)))
}

/// 文字コード名(大文字小文字は無視)を `supported_encodings` の正規名へ。未対応は None。
pub fn canonical_encoding_name(name: &str) -> Option<&'static str> {
    find_entry(name).map(|e| e.name)
}

/// 指定読み込み・保存で選べる文字コード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextEncodingInfo {
    /// 正規名(`decode` が返す名前と同じ。`decode_as` / `encode` に渡す)
    pub name: &'static str,
    /// 日本語の表示名
    pub display_name: &'static str,
    /// 「Unicode」「日本語」「中国語」「韓国語」「西欧」「中欧・東欧」「キリル」「その他」「EBCDIC」
    pub group: &'static str,
}

/// 対応する文字コードの一覧(表示順)
pub fn supported_encodings() -> Vec<TextEncodingInfo> {
    TABLE
        .iter()
        .map(|e| TextEncodingInfo {
            name: e.name,
            display_name: e.display_name,
            group: e.group,
        })
        .collect()
}

// ==================== 指定読み込み ====================

/// 指定した文字コードで読んだ結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    /// `supported_encodings` の正規名
    pub encoding: &'static str,
    /// 不正なバイト列を U+FFFD に置換したか(true のまま保存すると元のデータを壊す)
    pub had_replacements: bool,
}

/// 指定読み込みの結果(サイズ上限超過だけ区別する。バイナリ判定は行わない)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextLoadAsResult {
    Text(DecodedText),
    TooLarge,
}

/// 文字コードを指定してバイト列を読む。未知の名前は None。
/// 不正なバイト列は U+FFFD に置換して続行する。BOM があれば取り除く。
///
/// 返す `encoding` は実際の BOM の有無を反映する(例: "UTF-8" を指定して BOM 付きなら "UTF-8 (BOM)")。
/// そのため `encode(encoding, text)` は元のバイト列(BOM を含む)へ戻る。
pub fn decode_as(data: &[u8], encoding: &str) -> Option<DecodedText> {
    let entry = find_entry(encoding)?;
    let (text, had_replacements) = decode_lossy(entry, data);
    Some(DecodedText {
        text,
        encoding: resolve_bom_variant(entry, data),
        had_replacements,
    })
}

/// Unicode 系は、データの先頭に BOM があるかどうかで BOM 付き・なしの名前を選び直す。
/// ISO-2022-JP(名前指定が "ISO-2022-JP" のとき)は、使われている指示で変種の名前を選ぶ。
fn resolve_bom_variant(entry: &'static Entry, data: &[u8]) -> &'static str {
    let (with_bom, without_bom, bom): (&'static str, &'static str, &[u8]) = match entry.codec {
        // ISO-2022-JP は、ファイルが使っている旧 JIS・JIS ローマ字の指示を名前に反映する
        Codec::Iso2022Jp { .. } if entry.name == "ISO-2022-JP" => {
            return iso2022jp_variant_name(data)
        }
        Codec::Utf8 => ("UTF-8 (BOM)", "UTF-8", &[0xEF, 0xBB, 0xBF]),
        Codec::Utf16 { be: false } => ("UTF-16 LE", "UTF-16 LE (BOMなし)", &[0xFF, 0xFE]),
        Codec::Utf16 { be: true } => ("UTF-16 BE", "UTF-16 BE (BOMなし)", &[0xFE, 0xFF]),
        Codec::Utf32 { be: false } => ("UTF-32 LE", "UTF-32 LE (BOMなし)", &[0xFF, 0xFE, 0, 0]),
        Codec::Utf32 { be: true } => ("UTF-32 BE", "UTF-32 BE (BOMなし)", &[0, 0, 0xFE, 0xFF]),
        _ => return entry.name,
    };
    if data.starts_with(bom) {
        with_bom
    } else {
        without_bom
    }
}

/// 文字コードを指定してファイルを読む。未知の名前は `InvalidInput` エラー。
pub fn load_as(path: &Path, encoding: &str) -> std::io::Result<TextLoadAsResult> {
    if find_entry(encoding).is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("未対応の文字コードです: {}", encoding),
        ));
    }
    if let Ok(md) = std::fs::metadata(path) {
        if md.len() > MAX_TEXT_BYTES {
            return Ok(TextLoadAsResult::TooLarge);
        }
    }
    let data = std::fs::read(path)?;
    if data.len() as u64 > MAX_TEXT_BYTES {
        return Ok(TextLoadAsResult::TooLarge);
    }
    Ok(TextLoadAsResult::Text(
        decode_as(&data, encoding).expect("名前は検証済み"),
    ))
}

fn decode_lossy(entry: &Entry, data: &[u8]) -> (String, bool) {
    match entry.codec {
        Codec::Utf8 => {
            let body = data.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(data);
            match String::from_utf8_lossy(body) {
                Cow::Borrowed(s) => (s.to_string(), false),
                Cow::Owned(s) => (s, true),
            }
        }
        Codec::Utf16 { be } => {
            let bom: &[u8] = if be { &[0xFE, 0xFF] } else { &[0xFF, 0xFE] };
            let body = data.strip_prefix(bom).unwrap_or(data);
            let mut replaced = !body.len().is_multiple_of(2);
            let units = body.chunks_exact(2).map(|c| {
                if be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            });
            let mut out = String::with_capacity(body.len() / 2);
            for r in char::decode_utf16(units) {
                match r {
                    Ok(c) => out.push(c),
                    Err(_) => {
                        out.push('\u{FFFD}');
                        replaced = true;
                    }
                }
            }
            if !body.len().is_multiple_of(2) {
                out.push('\u{FFFD}');
            }
            (out, replaced)
        }
        Codec::Utf32 { be } => {
            let bom: &[u8] = if be {
                &[0x00, 0x00, 0xFE, 0xFF]
            } else {
                &[0xFF, 0xFE, 0x00, 0x00]
            };
            let body = data.strip_prefix(bom).unwrap_or(data);
            let mut replaced = false;
            let mut out = String::with_capacity(body.len() / 4);
            for c in body.chunks_exact(4) {
                let cp = if be {
                    u32::from_be_bytes([c[0], c[1], c[2], c[3]])
                } else {
                    u32::from_le_bytes([c[0], c[1], c[2], c[3]])
                };
                match char::from_u32(cp) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push('\u{FFFD}');
                        replaced = true;
                    }
                }
            }
            if !body.len().is_multiple_of(4) {
                out.push('\u{FFFD}');
                replaced = true;
            }
            (out, replaced)
        }
        Codec::Latin1 => (data.iter().map(|&b| b as char).collect(), false),
        Codec::Rs(enc) => {
            let (text, had_errors) = enc.decode_without_bom_handling(data);
            (text.into_owned(), had_errors)
        }
        Codec::Custom(c) => (c.decode)(data),
        Codec::Iso2022Jp { .. } => {
            let converted = jis7_to_iso2022jp(data);
            let (text, had_errors) =
                encoding_rs::ISO_2022_JP.decode_without_bom_handling(&converted);
            (text.into_owned(), had_errors)
        }
    }
}

/// データが使う指示(旧 JIS `ESC $ @`、JIS ローマ字 `ESC ( J`)に合わせた ISO-2022-JP の名前
fn iso2022jp_variant_name(data: &[u8]) -> &'static str {
    let has = |needle: &[u8]| data.windows(needle.len()).any(|w| w == needle);
    match (has(b"\x1b$@"), has(b"\x1b(J")) {
        (false, false) => "ISO-2022-JP",
        (true, false) => "ISO-2022-JP (旧JIS)",
        (false, true) => "ISO-2022-JP (JISローマ字)",
        (true, true) => "ISO-2022-JP (旧JIS・JISローマ字)",
    }
}

// ==================== JIS7(SO/SI)====================

/// ESC で始まるエスケープシーケンスの長さ(`ESC ( X`・`ESC $ X`・`ESC $ ( X`。それ以外は ESC 単独)
fn escape_len(rest: &[u8]) -> usize {
    let n = match rest.get(1) {
        Some(b'(') => 3,
        Some(b'$') if rest.get(2) == Some(&b'(') => 4,
        Some(b'$') => 3,
        _ => 1,
    };
    n.min(rest.len())
}

/// SO/SI(0x0E / 0x0F)による 7 ビット半角カナ(JIS7)を、encoding_rs の ISO-2022-JP デコーダが
/// 読める `ESC ( I` / G0 の再指示へ変換する。SO/SI を含まなければそのまま返す。
fn jis7_to_iso2022jp(data: &[u8]) -> Cow<'_, [u8]> {
    if !data.iter().any(|&b| b == 0x0E || b == 0x0F) {
        return Cow::Borrowed(data);
    }
    // WHATWG の ISO-2022-JP デコーダは、文字を挟まずにエスケープシーケンスが続くとエラーにする。
    // SO/SI の変換で連続しやすいため、直前が(文字を挟まない)エスケープなら置き換える。
    fn emit_escape(out: &mut Vec<u8>, last_escape: &mut Option<usize>, seq: &[u8]) {
        if let Some(start) = last_escape.take() {
            out.truncate(start);
        }
        *last_escape = Some(out.len());
        out.extend_from_slice(seq);
    }
    let mut out = Vec::with_capacity(data.len() + 8);
    let mut last_escape: Option<usize> = None;
    let mut g0: Vec<u8> = b"\x1b(B".to_vec();
    let mut shifted = false;
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            0x0E => {
                if !shifted {
                    emit_escape(&mut out, &mut last_escape, b"\x1b(I");
                    shifted = true;
                }
                i += 1;
            }
            0x0F => {
                if shifted {
                    emit_escape(&mut out, &mut last_escape, &g0.clone());
                    shifted = false;
                }
                i += 1;
            }
            0x1B => {
                let n = escape_len(&data[i..]);
                let seq = &data[i..i + n];
                if n > 1 {
                    g0 = seq.to_vec();
                }
                // SO 中の G0 再指示は、SI で戻るときに出す
                if !shifted {
                    emit_escape(&mut out, &mut last_escape, seq);
                }
                i += n;
            }
            b => {
                out.push(b);
                last_escape = None;
                i += 1;
            }
        }
    }
    Cow::Owned(out)
}

// ==================== 自動判定の補助 ====================

/// 制御文字(C0 のうちタブ・改行・復帰・改ページ・ESC 以外、DEL、C1)か
fn is_suspicious_control(c: char) -> bool {
    let u = c as u32;
    (u < 0x20 && !matches!(c, '\t' | '\n' | '\r' | '\x0c' | '\x1b')) || (0x7F..=0x9F).contains(&u)
}

/// 制御文字の割合が 0.1% 未満か
fn control_ratio_ok(text: &str) -> bool {
    let mut total = 0usize;
    let mut ctrl = 0usize;
    for c in text.chars() {
        total += 1;
        if is_suspicious_control(c) {
            ctrl += 1;
        }
    }
    ctrl * 1000 < total.max(1)
}

/// 文章として自然な文字(ASCII の印字文字と空白、ラテン・ギリシャ・キリル・ヘブライ・アラビア・
/// タイ、日本語の約物・かな・漢字・全角、ハングル)か
fn is_text_like(c: char) -> bool {
    let u = c as u32;
    matches!(c, '\t' | '\n' | '\r' | '\x0c')
        || (0x20..0x7F).contains(&u)
        || (0xA0..=0x024F).contains(&u)
        || (0x0370..=0x06FF).contains(&u)
        || (0x0E00..=0x0E7F).contains(&u)
        || (0x2000..=0x206F).contains(&u)
        || (0x3000..=0x30FF).contains(&u)
        || (0x3400..=0x4DBF).contains(&u)
        || (0x4E00..=0x9FFF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xFF00..=0xFFEF).contains(&u)
}

/// かな・日本語の約物・全角・ASCII(向きの判定用)
fn is_japanese_or_ascii(c: char) -> bool {
    let u = c as u32;
    matches!(c, '\t' | '\n' | '\r')
        || (0x20..0x7F).contains(&u)
        || (0x3000..=0x30FF).contains(&u)
        || (0xFF00..=0xFFEF).contains(&u)
}

fn text_like_ratio_ok(text: &str) -> bool {
    let mut total = 0usize;
    let mut like = 0usize;
    for c in text.chars() {
        total += 1;
        if is_text_like(c) {
            like += 1;
        }
    }
    total > 0 && like * 100 >= total * 95
}

/// BOM なし UTF-32 / UTF-16(NUL が一方の位置に強く偏る、ASCII 主体のテキストの形)。
/// 偶数長(UTF-32 は 4 の倍数)、エラーなくデコードでき、制御文字がほぼ無く、文章らしい文字が
/// 大半のときだけ採用する。NUL によるバイナリ判定より前に呼ぶ。
fn detect_bomless_unicode_with_nul(data: &[u8]) -> Option<(String, &'static str)> {
    if data.len() < 4 || !data.contains(&0) {
        return None;
    }
    let accept = |t: &str| control_ratio_ok(t) && !t.contains('\0') && text_like_ratio_ok(t);
    if data.len().is_multiple_of(4) {
        let nul = data.iter().filter(|&&b| b == 0).count();
        if nul * 100 >= data.len() * 45 {
            for (be, name) in [
                (false, "UTF-32 LE (BOMなし)"),
                (true, "UTF-32 BE (BOMなし)"),
            ] {
                if let Some(t) = decode_utf32(data, be) {
                    if accept(&t) {
                        return Some((t, name));
                    }
                }
            }
        }
    }
    if data.len().is_multiple_of(2) {
        let n = data.len() / 2;
        let (mut even, mut odd) = (0usize, 0usize);
        for (i, &b) in data.iter().enumerate() {
            if b == 0 {
                if i % 2 == 0 {
                    even += 1;
                } else {
                    odd += 1;
                }
            }
        }
        // 奇数位置に偏る = 上位バイトが 0 = LE、偶数位置に偏る = BE。偏りの強い方だけを試す
        let (dominant, other, be) = if odd >= even {
            (odd, even, false)
        } else {
            (even, odd, true)
        };
        // 短い入力は ASCII 主体(NUL が多い)のときだけ。十分長ければ、改行などによる少数の
        // NUL でも、日本語主体の UTF-16 として読めるか確かめる(デコードと文字種の検査が本体)
        let enough_nul = if n >= 16 {
            dominant >= 1
        } else {
            dominant * 2 >= n
        };
        if enough_nul && other * 4 <= dominant {
            if let Some(t) = decode_utf16(data, be) {
                if accept(&t) {
                    let name = if be {
                        "UTF-16 BE (BOMなし)"
                    } else {
                        "UTF-16 LE (BOMなし)"
                    };
                    return Some((t, name));
                }
            }
        }
    }
    None
}

/// NUL を含まない BOM なし UTF-16(日本語主体)。かな・全角・ASCII の割合が高い向きを採る。
/// 呼び出し側で UTF-8 / Shift_JIS / EUC-JP が失敗した後に使う。
fn detect_bomless_utf16_japanese(data: &[u8]) -> Option<(String, &'static str)> {
    if data.len() < 8 || !data.len().is_multiple_of(2) {
        return None;
    }
    let mut best: Option<(usize, String, &'static str)> = None;
    let mut second_score = 0usize;
    for (be, name) in [
        (false, "UTF-16 LE (BOMなし)"),
        (true, "UTF-16 BE (BOMなし)"),
    ] {
        let Some(t) = decode_utf16(data, be) else {
            continue;
        };
        if !control_ratio_ok(&t) || t.contains('\0') || !text_like_ratio_ok(&t) {
            continue;
        }
        let total = t.chars().count();
        let score = t.chars().filter(|&c| is_japanese_or_ascii(c)).count();
        if score * 100 < total * 30 {
            continue;
        }
        match &best {
            Some((s, _, _)) if *s >= score => second_score = second_score.max(score),
            Some((s, _, _)) => {
                second_score = *s;
                best = Some((score, t, name));
            }
            None => best = Some((score, t, name)),
        }
    }
    let (score, text, name) = best?;
    // 逆向きでも成り立つ(曖昧な)ときは採用しない
    if second_score * 2 > score {
        return None;
    }
    Some((text, name))
}

/// chardetng による推定を使う最小サイズ(バイト)。単バイト系は短い乱数でも制御文字なしで
/// デコードできてしまうことがあるため、短い入力では推定しない(従来どおりバイナリ)。
const CHARDET_MIN_BYTES: usize = 64;

fn has_kana(text: &str) -> bool {
    text.chars().any(|c| matches!(c as u32, 0x3040..=0x30FF))
}

fn chardetng_guess(data: &[u8]) -> &'static Encoding {
    const SAMPLE: usize = 512 * 1024;
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    let sample = &data[..data.len().min(SAMPLE)];
    detector.feed(sample, sample.len() == data.len());
    detector.guess(None, Utf8Detection::Deny)
}

/// 日本語(Shift_JIS / EUC-JP)としてデコードできたが、ひらがな・カタカナを含まず、非 ASCII が
/// 多いとき(中国語・韓国語・キリル文字などを誤って日本語として読んだ可能性がある)に、
/// chardetng が日本語以外の文字コードと推定し、エラーなくデコードできるならそちらを採る。
fn detect_non_japanese_override(
    data: &[u8],
    japanese_text: &str,
) -> Option<(String, &'static str)> {
    if has_kana(japanese_text) {
        return None;
    }
    let non_ascii = data.iter().filter(|&&b| b >= 0x80).count();
    if non_ascii * 100 < data.len() * 30 {
        return None;
    }
    // 16〜63 バイトは、短い CJK テキストの救済条件(下)で判定する
    if data.len() < CHARDET_MIN_BYTES {
        return detect_short_cjk(data);
    }
    let guessed = chardetng_guess(data);
    if [
        encoding_rs::SHIFT_JIS,
        encoding_rs::EUC_JP,
        encoding_rs::ISO_2022_JP,
    ]
    .iter()
    .any(|e| std::ptr::eq(*e, guessed))
    {
        return None;
    }
    decode_with_guess(data, guessed)
}

/// chardetng による推定。推定した文字コードでエラーなくデコードでき、制御文字の割合が
/// 0.1% 未満のときだけテキストとして採用する。
fn detect_with_chardetng(data: &[u8]) -> Option<(String, &'static str)> {
    if data.len() < CHARDET_MIN_BYTES {
        return None;
    }
    decode_with_guess(data, chardetng_guess(data))
}

fn decode_with_guess(data: &[u8], guessed: &'static Encoding) -> Option<(String, &'static str)> {
    let entry = TABLE
        .iter()
        .find(|e| matches!(e.codec, Codec::Rs(enc) if std::ptr::eq(enc, guessed)))?;
    let Codec::Rs(enc) = entry.codec else {
        return None;
    };
    let text = enc.decode_without_bom_handling_and_without_replacement(data)?;
    if !control_ratio_ok(&text) {
        return None;
    }
    Some((text.into_owned(), entry.name))
}

// ==================== 自動判定の追加(JIS X 0213・ISO-2022-JP 拡張・西欧語・短い単バイト) ====================

/// ひらがな・カタカナ(半角カナを含む)を 1 文字でも含むか
fn has_any_kana(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c as u32, 0x3040..=0x30FF | 0xFF61..=0xFF9F))
}

/// Shift_JIS(WHATWG)/ EUC-JP が失敗したときだけ、Shift_JIS-2004 → EUC-JIS-2004 を厳密に試す。
/// EUC-JIS-2004 は GBK / Big5 / EUC-KR の多くも読めてしまうため、デコード結果に
/// かな(半角カナを含む)が 1 文字以上あるときだけ採用する(JIS X 0213 の字だけでかなを含まない
/// 日本語ファイルは、手動指定で開く)。
fn decode_jis2004_legacy(data: &[u8]) -> Option<(String, &'static str)> {
    for (name, decode) in [
        ("Shift_JIS-2004", legacy::SHIFT_JIS_2004.decode),
        ("EUC-JIS-2004", legacy::EUC_JIS_2004.decode),
    ] {
        let (text, bad) = decode(data);
        // Shift_JIS 系では 0xA1-0xDF の 1 バイトがそのまま半角カナになるため、短い乱数でも
        // 「半角カナを含む」を満たしやすい。Shift_JIS-2004 は全角かなを必須にする。
        // EUC 系の半角カナは 0x8E の前置きが要るので、半角カナだけでも採用する。
        let kana_ok = if name == "Shift_JIS-2004" {
            has_kana(&text)
        } else {
            has_any_kana(&text)
        };
        if !bad && kana_ok {
            return Some((text, name));
        }
    }
    None
}

/// ISO-2022-JP として読めない 7 ビットデータのうち、ISO-2022-JP-2004 / -2 の指示を含むもの
fn detect_iso2022jp_extensions(data: &[u8]) -> Option<(String, &'static str)> {
    let has = |needle: &[u8]| data.windows(needle.len()).any(|w| w == needle);
    if has(b"\x1b$(O") || has(b"\x1b$(Q") || has(b"\x1b$(P") {
        let (text, bad) = (legacy::ISO_2022_JP_2004.decode)(&jis7_to_iso2022jp(data));
        if !bad {
            return Some((text, "ISO-2022-JP-2004"));
        }
    }
    if has(b"\x1b$(D")
        || has(b"\x1b$A")
        || has(b"\x1b$(A")
        || has(b"\x1b$(C")
        || has(b"\x1b.A")
        || has(b"\x1b.F")
    {
        let (text, bad) = (legacy::ISO_2022_JP_2.decode)(&jis7_to_iso2022jp(data));
        if !bad {
            return Some((text, "ISO-2022-JP-2"));
        }
    }
    None
}

/// 西欧・中欧の単バイト系か
fn is_western_encoding(enc: &'static Encoding) -> bool {
    TABLE.iter().any(|e| {
        matches!(e.codec, Codec::Rs(x) if std::ptr::eq(x, enc))
            && (e.group == "西欧" || e.group == "中欧・東欧")
    })
}

/// Shift_JIS / EUC-JP / Shift_JIS-2004 としてデコードできた結果が、かな・全角記号・句読点を 1 つも含まず、
/// 元のバイト列の高位バイトがすべて「孤立した 1 バイトで、直後が ASCII の英字」(日本語の漢字は必ず
/// 高位バイトが 2 つ続くので、アクセント付きの 1 バイト文字だけの列に限られる)で、かつ chardetng が
/// 西欧・中欧の単バイト系と推定するときは、その単バイト系を採る(64 バイト未満は短い単バイト系の判定条件)。
fn detect_western_override(data: &[u8], japanese_text: &str) -> Option<(String, &'static str)> {
    if data.len() < 4 {
        return None;
    }
    if japanese_text
        .chars()
        .any(|c| matches!(c as u32, 0x3000..=0x30FF | 0xFF00..=0xFFEF))
    {
        return None;
    }
    let mut any_high = false;
    for (i, &b) in data.iter().enumerate() {
        if b < 0x80 {
            continue;
        }
        any_high = true;
        let prev_high = i > 0 && data[i - 1] >= 0x80;
        let next_letter = data.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic());
        if prev_high || !next_letter {
            return None;
        }
    }
    if !any_high {
        return None;
    }
    if data.len() < CHARDET_MIN_BYTES {
        return detect_short_single_byte(data);
    }
    let guessed = chardetng_guess(data);
    if !is_western_encoding(guessed) {
        return None;
    }
    decode_with_guess(data, guessed)
}

/// 4〜63 バイトの短い単バイト系テキスト(UTF-8 / Shift_JIS / EUC-JP が全部失敗した後の最終段)。
/// (a) タブ・改行・復帰以外の制御文字が 1 つも無く、(b) 高位バイトが 30% 未満で、(c) ASCII の英字が
/// 半分以上のときだけ、chardetng の推定(厳密にデコードできなければ windows-1252)でテキストにする。
fn detect_short_single_byte(data: &[u8]) -> Option<(String, &'static str)> {
    if data.len() < 4 || data.len() >= CHARDET_MIN_BYTES {
        return None;
    }
    if data
        .iter()
        .any(|&b| (b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r')) || b == 0x7F)
    {
        return None;
    }
    let high = data.iter().filter(|&&b| b >= 0x80).count();
    let letters = data.iter().filter(|b| b.is_ascii_alphabetic()).count();
    if high * 100 >= data.len() * 30 || letters * 2 < data.len() {
        return None;
    }
    let try_decode = |enc: &'static Encoding| {
        let entry = TABLE
            .iter()
            .find(|e| matches!(e.codec, Codec::Rs(x) if std::ptr::eq(x, enc)))?;
        let text = enc.decode_without_bom_handling_and_without_replacement(data)?;
        // C1 を含む(未定義バイトの置き換え)ものは不可
        if text.chars().any(is_suspicious_control) {
            return None;
        }
        Some((text.into_owned(), entry.name))
    };
    try_decode(chardetng_guess(data)).or_else(|| try_decode(encoding_rs::WINDOWS_1252))
}

/// CJK 統合漢字・ハングル・かな・CJK の句読点と全角記号か(ASCII 以外の文字の許容範囲)
fn is_cjk_text_char(c: char) -> bool {
    matches!(
        c as u32,
        0x3000..=0x30FF       // CJK の句読点・かな
        | 0x3130..=0x318F     // ハングル互換字母
        | 0x3400..=0x4DBF     // CJK 統合漢字拡張 A
        | 0x4E00..=0x9FFF     // CJK 統合漢字
        | 0xAC00..=0xD7A3     // ハングル音節
        | 0xFF00..=0xFFEF     // 全角形・半角形
    )
}

/// 16〜63 バイトの短い CJK テキスト(GBK / GB18030 / Big5 / EUC-KR)の救済。chardetng がそれらと推定し、
/// 厳密にデコードでき、制御文字が無く、ASCII 以外の文字がすべて CJK 統合漢字・ハングル・かな・
/// CJK の句読点・全角記号のときだけ採用する(さらに、ASCII は英数字・空白・基本的な句読点のみ、
/// 非 ASCII が 6 字以上で全体の 6 割以上、CJK の句読点・全角記号か空白・改行を含むこと)。
fn detect_short_cjk(data: &[u8]) -> Option<(String, &'static str)> {
    if data.len() < 16 || data.len() >= CHARDET_MIN_BYTES {
        return None;
    }
    let guessed = chardetng_guess(data);
    let cjk = [
        encoding_rs::GBK,
        encoding_rs::GB18030,
        encoding_rs::BIG5,
        encoding_rs::EUC_KR,
    ];
    if !cjk.iter().any(|e| std::ptr::eq(*e, guessed)) {
        return None;
    }
    // 2 バイト文字の先頭バイトが、その文字コードの「よく使う字」の範囲に 85% 以上入っていること
    // (GBK: 句読点 A1-A3 と 1 級漢字 B0-D7、Big5: A1-A3 と常用字 A4-C6、EUC-KR: A1-A3 とハングル B0-C8)。
    // 乱数のバイト列がたまたま読める場合を除く。
    let common: &dyn Fn(u8) -> bool = if std::ptr::eq(guessed, encoding_rs::BIG5) {
        &|b| matches!(b, 0xA1..=0xC6)
    } else if std::ptr::eq(guessed, encoding_rs::EUC_KR) {
        &|b| matches!(b, 0xA1..=0xA3 | 0xB0..=0xC8)
    } else {
        &|b| matches!(b, 0xA1..=0xA3 | 0xB0..=0xD7)
    };
    let (mut pairs, mut common_pairs, mut i) = (0usize, 0usize, 0usize);
    while i < data.len() {
        if data[i] >= 0x80 {
            pairs += 1;
            common_pairs += common(data[i]) as usize;
            i += 2;
        } else {
            i += 1;
        }
    }
    if pairs == 0 || common_pairs * 100 < pairs * 85 {
        return None;
    }
    let (text, name) = decode_with_guess(data, guessed)?;
    if text.chars().any(is_suspicious_control) {
        return None;
    }
    if !text.chars().all(|c| c.is_ascii() || is_cjk_text_char(c)) {
        return None;
    }
    // 文として自然なものだけ(乱数の誤採用を避ける): ASCII は英数字・空白・基本的な句読点のみ、
    // 非 ASCII の文字が 6 字以上で、CJK の句読点・全角記号、空白、改行のいずれかを含む。
    let ascii_ok = text.chars().all(|c| {
        !c.is_ascii()
            || c.is_ascii_alphanumeric()
            || matches!(
                c,
                ' ' | '\t'
                    | '\r'
                    | '\n'
                    | '.'
                    | ','
                    | ':'
                    | ';'
                    | '!'
                    | '?'
                    | '-'
                    | '('
                    | ')'
                    | '\''
                    | '"'
            )
    });
    let non_ascii = text.chars().filter(|c| !c.is_ascii()).count();
    let sentence_like = text.chars().any(|c| {
        matches!(c as u32, 0x3000..=0x303F | 0xFF00..=0xFFEF) || matches!(c, ' ' | '\r' | '\n')
    });
    let total = text.chars().count();
    if !ascii_ok || non_ascii < 6 || non_ascii * 100 < total * 60 || !sentence_like {
        return None;
    }
    Some((text, name))
}
