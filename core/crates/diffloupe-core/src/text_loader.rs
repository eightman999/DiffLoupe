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
        .chunks_exact(2)
        .map(|c| {
            if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
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
    for c in data.chunks_exact(4) {
        let cp = if big_endian {
            u32::from_be_bytes([c[0], c[1], c[2], c[3]])
        } else {
            u32::from_le_bytes([c[0], c[1], c[2], c[3]])
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
