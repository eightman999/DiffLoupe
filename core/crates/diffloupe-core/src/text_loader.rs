//! テキスト読み込み・エンコード判定(Swift `TextFileLoader` 相当)

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
/// 判定順: BOM → NUL sniff → ISO-2022-JP → UTF-8 → Shift_JIS/EUC-JP
pub fn decode(data: &[u8]) -> Option<(String, &'static str)> {
    if let Some(r) = decode_by_bom(data) {
        return Some(r);
    }
    if data[..data.len().min(BINARY_SNIFF_LENGTH)].contains(&0) {
        return None;
    }
    if looks_like_iso2022jp(data) {
        if let Some(t) = encoding_rs::ISO_2022_JP
            .decode_without_bom_handling_and_without_replacement(data)
            .map(|c| c.into_owned())
        {
            return Some((t, "ISO-2022-JP"));
        }
    }
    if let Ok(t) = std::str::from_utf8(data) {
        return Some((t.to_string(), "UTF-8"));
    }
    decode_japanese_legacy(data)
}

/// `decode` が返すエンコード名に対応する BOM。BOM を付けない名前は空、未対応の名前は None。
pub fn bom_for(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "UTF-8" | "Shift_JIS" | "EUC-JP" | "ISO-2022-JP" => &[],
        "UTF-8 (BOM)" => &[0xEF, 0xBB, 0xBF],
        "UTF-16 BE" => &[0xFE, 0xFF],
        "UTF-16 LE" => &[0xFF, 0xFE],
        "UTF-32 BE" => &[0x00, 0x00, 0xFE, 0xFF],
        "UTF-32 LE" => &[0xFF, 0xFE, 0x00, 0x00],
        _ => return None,
    })
}

/// `decode` の逆変換: テキストを `name`(`decode` が返す名前)で符号化する(BOM は付けない)。
/// 未対応の名前・表現できない文字があるときは None。
pub fn encode_body(name: &str, text: &str) -> Option<Vec<u8>> {
    match name {
        "UTF-8" | "UTF-8 (BOM)" => Some(text.as_bytes().to_vec()),
        "UTF-16 BE" => Some(text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()),
        "UTF-16 LE" => Some(text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()),
        "UTF-32 BE" => Some(
            text.chars()
                .flat_map(|c| (c as u32).to_be_bytes())
                .collect(),
        ),
        "UTF-32 LE" => Some(
            text.chars()
                .flat_map(|c| (c as u32).to_le_bytes())
                .collect(),
        ),
        "Shift_JIS" => encode_with(encoding_rs::SHIFT_JIS, text),
        "EUC-JP" => encode_with(encoding_rs::EUC_JP, text),
        "ISO-2022-JP" => encode_iso2022jp(text),
        _ => None,
    }
}

/// `decode` の逆変換: BOM(該当する名前のみ)+本文。`decode(&encode(n, t)?)` は `(t, n)` に戻る
/// (ISO-2022-JP / Shift_JIS 等は内容が判定可能な場合)。未対応・表現不可は None。
pub fn encode(name: &str, text: &str) -> Option<Vec<u8>> {
    let bom = bom_for(name)?;
    let mut out = bom.to_vec();
    out.extend(encode_body(name, text)?);
    Some(out)
}

fn encode_with(enc: &'static encoding_rs::Encoding, text: &str) -> Option<Vec<u8>> {
    let (bytes, _, had_unmappable) = enc.encode(text);
    if had_unmappable {
        return None;
    }
    Some(bytes.into_owned())
}

/// ISO-2022-JP(JIS X 0208 + JIS X 0201 カナ)。encoding_rs の ISO-2022-JP は符号化側が UTF-8 に
/// なる(WHATWG 仕様)ため使えない。EUC-JP の符号化結果から 7bit へ変換して組み立てる。
/// Swift(`String.Encoding.iso2022JP`)と同じく、2バイト文字は `ESC $ B`、半角カナは `ESC ( I`、
/// ASCII へ戻るとき `ESC ( B`、末尾は ASCII モードで閉じる。JIS X 0212 は表現不可として扱う。
fn encode_iso2022jp(text: &str) -> Option<Vec<u8>> {
    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
        Ascii,
        Jis0208,
        Kana,
    }
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut mode = Mode::Ascii;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        let want;
        let mut bytes: [u8; 2] = [0, 0];
        let len;
        if (ch as u32) < 0x80 {
            want = Mode::Ascii;
            bytes[0] = ch as u8;
            len = 1;
        } else {
            let s = ch.encode_utf8(&mut buf);
            let (enc, _, bad) = encoding_rs::EUC_JP.encode(s);
            if bad {
                return None;
            }
            match enc.as_ref() {
                [0x8E, k] if (0xA1..=0xDF).contains(k) => {
                    want = Mode::Kana;
                    bytes[0] = k & 0x7F;
                    len = 1;
                }
                [a, b] if (0xA1..=0xFE).contains(a) && (0xA1..=0xFE).contains(b) => {
                    want = Mode::Jis0208;
                    bytes = [a & 0x7F, b & 0x7F];
                    len = 2;
                }
                _ => return None,
            }
        }
        if want != mode {
            out.extend_from_slice(match want {
                Mode::Ascii => b"\x1b(B",
                Mode::Jis0208 => b"\x1b$B",
                Mode::Kana => b"\x1b(I",
            });
            mode = want;
        }
        out.extend_from_slice(&bytes[..len]);
    }
    if mode != Mode::Ascii {
        out.extend_from_slice(b"\x1b(B");
    }
    Some(out)
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
