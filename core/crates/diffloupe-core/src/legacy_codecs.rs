//! 追加の文字コード(表・状態機械による自前実装)。
//!
//! Shift_JIS-2004 / EUC-JIS-2004 / ISO-2022-JP-2004(JIS X 0213:2004)、ISO-2022-JP-2、MacJapanese、
//! EBCDIC(IBM037 / IBM500 / IBM1140 / IBM273 / IBM290 / IBM930 / IBM939)、UTF-7。
//! 復号は `(テキスト, エラーがあったか)`(不正な部分は U+FFFD)、符号化は
//! `Err((表せない文字, そのバイト位置))`。表は `legacy_tables.rs` を参照。
//!
//! 動作設計:
//! - Shift_JIS-2004 の 0x00-0x7F は ASCII のまま読む(0x5C は円記号ではなく `\`)。Python 等の
//!   JIS ローマ字風の対応(0x5C = U+00A5)は採らない。符号化できない文字は U+00A5 / U+203E を含め Err。
//! - ISO-2022-JP-2004 は ESC $ ( O / ESC $ ( Q(面 1。2000 版との差の 10 字は同じ表で読む)、
//!   ESC $ ( P(面 2)、ESC $ B / ESC $ @(JIS X 0208。面 1 の表で読む)、ESC ( I(半角カナ)を読む。
//!   符号化は JIS X 0208 の字は ESC $ B、それ以外の面 1 は ESC $ ( Q、面 2 は ESC $ ( P。
//! - ISO-2022-JP-2 は JIS X 0208 / JIS X 0212 / GB 2312 / KS X 1001 と、G2 の ISO-8859-1・-7 上位
//!   (ESC . A / ESC . F + ESC N)を読む。GB 2312 の指示は ESC $ A と ESC $ ( A の両方を読み、
//!   書くのは ESC $ A。G2 の指示は改行で解除する。符号化の優先順は JIS X 0208、JIS X 0212、
//!   GB 2312、KS X 1001、ISO-8859-1、ISO-8859-7。半角カナは表せない。
//! - EBCDIC は NEL(0x15)と LF(0x25)をどちらも LF として読む。符号化する LF は 0x25(NEL は U+0085)。
//!   IBM930 / IBM939 は SO(0x0E)/ SI(0x0F)で 2 バイト文字に切り替える。
//! - UTF-7 は RFC 2152。符号化で直接書くのは集合 D・集合 O と空白類、`+` は `+-`。

use crate::legacy_tables::{self as tables, Record};
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) type DecodeFn = fn(&[u8]) -> (String, bool);
pub(crate) type EncodeFn = fn(&str) -> Result<Vec<u8>, (char, usize)>;

pub(crate) struct CustomCodec {
    pub decode: DecodeFn,
    pub encode: EncodeFn,
}

const FFFD: char = '\u{FFFD}';

// ==================== JIS X 0213 ====================

const SECOND: [u32; 6] = [0, 0x309A, 0x0300, 0x0301, 0x02E5, 0x02E9];

/// 面(0 = 面 1、1 = 面 2)・区・点(各 1〜94)→ 文字(結合文字を伴う場合は 2 つ)
fn jis0213_cell(plane: usize, row: usize, cell: usize) -> Option<(char, Option<char>)> {
    if !(1..=94).contains(&row) || !(1..=94).contains(&cell) {
        return None;
    }
    let idx = plane * 8836 + (row - 1) * 94 + (cell - 1);
    let b = &tables::JISX0213[idx * 4..idx * 4 + 4];
    let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    if v == 0 {
        return None;
    }
    let first = char::from_u32(v & 0x00FF_FFFF)?;
    let second = SECOND.get((v >> 24) as usize).copied().unwrap_or(0);
    Some((first, char::from_u32(second).filter(|_| second != 0)))
}

type CellEntry = (Option<char>, u8, u8, u8);

fn jis0213_reverse() -> &'static HashMap<char, Vec<CellEntry>> {
    static MAP: OnceLock<HashMap<char, Vec<CellEntry>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: HashMap<char, Vec<CellEntry>> = HashMap::new();
        for plane in 0..2usize {
            for row in 1..=94usize {
                for cell in 1..=94usize {
                    if let Some((a, b)) = jis0213_cell(plane, row, cell) {
                        map.entry(a)
                            .or_default()
                            .push((b, plane as u8, row as u8, cell as u8));
                    }
                }
            }
        }
        map
    })
}

/// 文字列の `i` 文字目から始まる、JIS X 0213 の区点(結合文字を伴う 2 文字の写像を優先)と消費した文字数
fn jis0213_find(chars: &[(usize, char)], i: usize) -> Option<(u8, u8, u8, usize)> {
    let entries = jis0213_reverse().get(&chars[i].1)?;
    if let Some(next) = chars.get(i + 1).map(|c| c.1) {
        if let Some(e) = entries.iter().find(|e| e.0 == Some(next)) {
            return Some((e.1, e.2, e.3, 2));
        }
    }
    entries
        .iter()
        .find(|e| e.0.is_none())
        .map(|e| (e.1, e.2, e.3, 1))
}

fn push_cell(out: &mut String, c: (char, Option<char>)) {
    out.push(c.0);
    if let Some(s) = c.1 {
        out.push(s);
    }
}

/// JIS X 0208 に含まれる区点か(ISO-2022-JP-2004 で ESC $ B を使えるか)。encoding_rs の EUC-JP で調べる。
fn is_jis0208_cell(row: u8, cell: u8) -> bool {
    static CACHE: OnceLock<Vec<bool>> = OnceLock::new();
    let table = CACHE.get_or_init(|| {
        let mut v = vec![false; 94 * 94];
        // JIS X 0208 の区は 1〜8 と 16〜84(encoding_rs の EUC-JP は NEC 13 区・IBM 拡張も復号するため除く)
        for r in (1..=8usize).chain(16..=84usize) {
            for c in 1..=94usize {
                let raw = [(0xA0 + r) as u8, (0xA0 + c) as u8];
                v[(r - 1) * 94 + (c - 1)] = encoding_rs::EUC_JP
                    .decode_without_bom_handling_and_without_replacement(&raw)
                    .is_some();
            }
        }
        v
    });
    table[(row as usize - 1) * 94 + (cell as usize - 1)]
}

// ---------- Shift_JIS-2004 ----------

fn sjis_to_cell(s1: u8, s2: u8) -> Option<(usize, usize, usize)> {
    if !matches!(s2, 0x40..=0x7E | 0x80..=0xFC) {
        return None;
    }
    let second_half = s2 >= 0x9F;
    let h = second_half as usize;
    let cell = if second_half {
        s2 as usize - 0x9E
    } else if s2 < 0x7F {
        s2 as usize - 0x3F
    } else {
        s2 as usize - 0x40
    };
    let (plane, row) = match s1 {
        0x81..=0x9F => (0, 2 * (s1 as usize - 0x81) + 1 + h),
        0xE0..=0xEF => (0, 2 * (s1 as usize - 0xE0) + 63 + h),
        0xF0 => (1, [1, 8][h]),
        0xF1 => (1, [3, 4][h]),
        0xF2 => (1, [5, 12][h]),
        0xF3 => (1, [13, 14][h]),
        0xF4 => (1, [15, 78][h]),
        0xF5..=0xFC => (1, 79 + 2 * (s1 as usize - 0xF5) + h),
        _ => return None,
    };
    Some((plane, row, cell))
}

fn cell_to_sjis(plane: u8, row: u8, cell: u8) -> Option<[u8; 2]> {
    let (s1, second) = if plane == 0 {
        if row <= 62 {
            (0x81 + (row - 1) / 2, row.is_multiple_of(2))
        } else {
            (0xE0 + (row - 63) / 2, row.is_multiple_of(2))
        }
    } else {
        match row {
            1 => (0xF0, false),
            8 => (0xF0, true),
            3 => (0xF1, false),
            4 => (0xF1, true),
            5 => (0xF2, false),
            12 => (0xF2, true),
            13 => (0xF3, false),
            14 => (0xF3, true),
            15 => (0xF4, false),
            78 => (0xF4, true),
            79..=94 => (0xF5 + (row - 79) / 2, row.is_multiple_of(2)),
            _ => return None,
        }
    };
    let s2 = if second {
        cell + 0x9E
    } else if cell <= 63 {
        cell + 0x3F
    } else {
        cell + 0x40
    };
    Some([s1, s2])
}

fn decode_sjis2004(data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0x00..=0x7F => {
                out.push(b as char);
                i += 1;
            }
            0xA1..=0xDF => {
                out.push(char::from_u32(0xFF61 + (b as u32 - 0xA1)).unwrap_or(FFFD));
                i += 1;
            }
            0x81..=0x9F | 0xE0..=0xFC => {
                let cell = data
                    .get(i + 1)
                    .and_then(|&t| sjis_to_cell(b, t))
                    .and_then(|(p, r, c)| jis0213_cell(p, r, c));
                match cell {
                    Some(c) => {
                        push_cell(&mut out, c);
                        i += 2;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        // 次のバイトが ASCII ならそれは別の文字として読み直す
                        i += if data.get(i + 1).is_some_and(|&t| t >= 0x40 && t != 0x7F) {
                            2
                        } else {
                            1
                        };
                    }
                }
            }
            _ => {
                out.push(FFFD);
                bad = true;
                i += 1;
            }
        }
    }
    (out, bad)
}

fn encode_sjis2004(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let (pos, c) = chars[i];
        let u = c as u32;
        if u < 0x80 {
            out.push(u as u8);
            i += 1;
        } else if (0xFF61..=0xFF9F).contains(&u) {
            out.push((u - 0xFF61 + 0xA1) as u8);
            i += 1;
        } else if let Some((p, r, cl, n)) = jis0213_find(&chars, i) {
            let bytes = cell_to_sjis(p, r, cl).ok_or((c, pos))?;
            out.extend_from_slice(&bytes);
            i += n;
        } else {
            return Err((c, pos));
        }
    }
    Ok(out)
}

// ---------- EUC-JIS-2004 ----------

fn decode_eucjis2004(data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0x00..=0x7F => {
                out.push(b as char);
                i += 1;
            }
            0x8E => match data.get(i + 1) {
                Some(&k) if (0xA1..=0xDF).contains(&k) => {
                    out.push(char::from_u32(0xFF61 + (k as u32 - 0xA1)).unwrap_or(FFFD));
                    i += 2;
                }
                _ => {
                    out.push(FFFD);
                    bad = true;
                    i += 1;
                }
            },
            0x8F => {
                let cell = match (data.get(i + 1), data.get(i + 2)) {
                    (Some(&a), Some(&c)) if a >= 0xA1 && c >= 0xA1 => {
                        jis0213_cell(1, a as usize - 0xA0, c as usize - 0xA0)
                    }
                    _ => None,
                };
                match cell {
                    Some(c) => {
                        push_cell(&mut out, c);
                        i += 3;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        i += 1;
                    }
                }
            }
            0xA1..=0xFE => {
                let cell = match data.get(i + 1) {
                    Some(&c) if c >= 0xA1 => jis0213_cell(0, b as usize - 0xA0, c as usize - 0xA0),
                    _ => None,
                };
                match cell {
                    Some(c) => {
                        push_cell(&mut out, c);
                        i += 2;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(FFFD);
                bad = true;
                i += 1;
            }
        }
    }
    (out, bad)
}

fn encode_eucjis2004(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let (pos, c) = chars[i];
        let u = c as u32;
        if u < 0x80 {
            out.push(u as u8);
            i += 1;
        } else if (0xFF61..=0xFF9F).contains(&u) {
            out.extend_from_slice(&[0x8E, (u - 0xFF61 + 0xA1) as u8]);
            i += 1;
        } else if let Some((p, r, cl, n)) = jis0213_find(&chars, i) {
            if p == 1 {
                out.push(0x8F);
            }
            out.extend_from_slice(&[0xA0 + r, 0xA0 + cl]);
            i += n;
        } else {
            return Err((c, pos));
        }
    }
    Ok(out)
}

// ==================== ISO-2022-JP 系の共通部 ====================

/// ESC で始まる指示の長さ(`ESC X Y`、`ESC $ ( Y` は 4)。1 を返すときは単独の ESC。
fn esc_seq_len(rest: &[u8]) -> usize {
    match (rest.get(1), rest.get(2)) {
        (Some(b'$'), Some(b'(')) => 4.min(rest.len()),
        (Some(_), Some(_)) => 3,
        _ => 1,
    }
}

// ---------- ISO-2022-JP-2004 ----------

#[derive(Clone, Copy, PartialEq)]
enum G0Jis2004 {
    Ascii,
    Roman,
    Kana,
    Plane1,
    Plane2,
}

fn decode_iso2022jp2004(data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut mode = G0Jis2004::Ascii;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1B {
            let n = esc_seq_len(&data[i..]);
            let seq = &data[i..i + n];
            let new_mode = match seq {
                b"\x1b(B" => Some(G0Jis2004::Ascii),
                b"\x1b(J" => Some(G0Jis2004::Roman),
                b"\x1b(I" => Some(G0Jis2004::Kana),
                b"\x1b$@" | b"\x1b$B" | b"\x1b$(O" | b"\x1b$(Q" => Some(G0Jis2004::Plane1),
                b"\x1b$(P" => Some(G0Jis2004::Plane2),
                _ => None,
            };
            match new_mode {
                Some(m) => {
                    mode = m;
                    i += n;
                }
                None => {
                    out.push(FFFD);
                    bad = true;
                    i += 1;
                }
            }
            continue;
        }
        if b >= 0x80 {
            out.push(FFFD);
            bad = true;
            i += 1;
            continue;
        }
        // 制御文字(改行など)はどのモードでもそのまま
        if b < 0x21 || b == 0x7F {
            out.push(b as char);
            i += 1;
            continue;
        }
        match mode {
            G0Jis2004::Ascii => {
                out.push(b as char);
                i += 1;
            }
            G0Jis2004::Roman => {
                out.push(match b {
                    0x5C => '\u{A5}',
                    0x7E => '\u{203E}',
                    _ => b as char,
                });
                i += 1;
            }
            G0Jis2004::Kana => {
                if (0x21..=0x5F).contains(&b) {
                    out.push(char::from_u32(0xFF61 + (b as u32 - 0x21)).unwrap_or(FFFD));
                } else {
                    out.push(FFFD);
                    bad = true;
                }
                i += 1;
            }
            G0Jis2004::Plane1 | G0Jis2004::Plane2 => {
                let plane = (mode == G0Jis2004::Plane2) as usize;
                let cell = data
                    .get(i + 1)
                    .filter(|&&c| (0x21..=0x7E).contains(&c))
                    .and_then(|&c| jis0213_cell(plane, b as usize - 0x20, c as usize - 0x20));
                match cell {
                    Some(c) => {
                        push_cell(&mut out, c);
                        i += 2;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        i += if data.get(i + 1).is_some_and(|&c| (0x21..=0x7E).contains(&c)) {
                            2
                        } else {
                            1
                        };
                    }
                }
            }
        }
    }
    (out, bad)
}

fn encode_iso2022jp2004(text: &str) -> Result<Vec<u8>, (char, usize)> {
    const ASCII: &[u8] = b"\x1b(B";
    const KANA: &[u8] = b"\x1b(I";
    const P1_0208: &[u8] = b"\x1b$B";
    const P1_0213: &[u8] = b"\x1b$(Q";
    const P2: &[u8] = b"\x1b$(P";
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut cur: &[u8] = ASCII;
    let mut i = 0;
    while i < chars.len() {
        let (pos, c) = chars[i];
        let u = c as u32;
        let mut consumed = 1;
        let payload: [u8; 2];
        let len;
        let want: &[u8];
        if u < 0x80 {
            want = ASCII;
            payload = [u as u8, 0];
            len = 1;
        } else if (0xFF61..=0xFF9F).contains(&u) {
            want = KANA;
            payload = [(u - 0xFF61 + 0x21) as u8, 0];
            len = 1;
        } else if let Some((p, r, cl, n)) = jis0213_find(&chars, i) {
            consumed = n;
            payload = [0x20 + r, 0x20 + cl];
            len = 2;
            want = if p == 1 {
                P2
            } else if n == 1 && is_jis0208_cell(r, cl) {
                P1_0208
            } else {
                P1_0213
            };
        } else {
            return Err((c, pos));
        }
        // ESC $ ( Q は JIS X 0208 の字も含むため、その中では ESC $ B へ戻さない
        let satisfied = cur == want || (cur == P1_0213 && want == P1_0208);
        if !satisfied {
            out.extend_from_slice(want);
            cur = want;
        }
        out.extend_from_slice(&payload[..len]);
        i += consumed;
    }
    if cur != ASCII {
        out.extend_from_slice(ASCII);
    }
    Ok(out)
}

// ---------- ISO-2022-JP-2 ----------

#[derive(Clone, Copy, PartialEq)]
enum G0Jis2 {
    Ascii,
    Roman,
    Jis0208,
    Jis0212,
    Gb2312,
    Ksc5601,
}

#[derive(Clone, Copy, PartialEq)]
enum G2Set {
    None,
    Latin1,
    Greek,
}

fn two_byte_euc(enc: &'static encoding_rs::Encoding, prefix: &[u8], a: u8, b: u8) -> Option<char> {
    let mut raw = prefix.to_vec();
    raw.extend_from_slice(&[a | 0x80, b | 0x80]);
    let s = enc.decode_without_bom_handling_and_without_replacement(&raw)?;
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

fn decode_iso2022jp2(data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut g0 = G0Jis2::Ascii;
    let mut g2 = G2Set::None;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x1B {
            let n = esc_seq_len(&data[i..]);
            let seq = &data[i..i + n];
            // ESC N(SS2): 次の 1 バイトを G2 で読む
            if seq.starts_with(b"\x1bN") {
                let ch = data
                    .get(i + 2)
                    .copied()
                    .filter(|c| (0x20..=0x7F).contains(c));
                let decoded = match (g2, ch) {
                    (G2Set::Latin1, Some(c)) => char::from_u32(c as u32 | 0x80),
                    (G2Set::Greek, Some(c)) => encoding_rs::ISO_8859_7
                        .decode_without_bom_handling_and_without_replacement(&[c | 0x80])
                        .and_then(|s| s.chars().next()),
                    _ => None,
                };
                match decoded {
                    Some(c) => {
                        out.push(c);
                        i += 3;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        i += if ch.is_some() { 3 } else { 2 };
                    }
                }
                continue;
            }
            let (new_g0, new_g2) = match seq {
                b"\x1b(B" => (Some(G0Jis2::Ascii), None),
                b"\x1b(J" => (Some(G0Jis2::Roman), None),
                b"\x1b$@" | b"\x1b$B" => (Some(G0Jis2::Jis0208), None),
                b"\x1b$A" | b"\x1b$(A" => (Some(G0Jis2::Gb2312), None),
                b"\x1b$(C" => (Some(G0Jis2::Ksc5601), None),
                b"\x1b$(D" => (Some(G0Jis2::Jis0212), None),
                b"\x1b.A" => (None, Some(G2Set::Latin1)),
                b"\x1b.F" => (None, Some(G2Set::Greek)),
                _ => (None, None),
            };
            if new_g0.is_none() && new_g2.is_none() {
                out.push(FFFD);
                bad = true;
                i += 1;
                continue;
            }
            if let Some(m) = new_g0 {
                g0 = m;
            }
            if let Some(m) = new_g2 {
                g2 = m;
            }
            i += n;
            continue;
        }
        if b >= 0x80 {
            out.push(FFFD);
            bad = true;
            i += 1;
            continue;
        }
        if b < 0x21 || b == 0x7F {
            out.push(b as char);
            if b == b'\n' {
                g2 = G2Set::None;
            }
            i += 1;
            continue;
        }
        match g0 {
            G0Jis2::Ascii => {
                out.push(b as char);
                i += 1;
            }
            G0Jis2::Roman => {
                out.push(match b {
                    0x5C => '\u{A5}',
                    0x7E => '\u{203E}',
                    _ => b as char,
                });
                i += 1;
            }
            _ => {
                let trail = data
                    .get(i + 1)
                    .copied()
                    .filter(|c| (0x21..=0x7E).contains(c));
                let ch = trail.and_then(|c| match g0 {
                    G0Jis2::Jis0208 => two_byte_euc(encoding_rs::EUC_JP, &[], b, c),
                    G0Jis2::Jis0212 => two_byte_euc(encoding_rs::EUC_JP, &[0x8F], b, c),
                    G0Jis2::Gb2312 => two_byte_euc(encoding_rs::GBK, &[], b, c),
                    G0Jis2::Ksc5601 => two_byte_euc(encoding_rs::EUC_KR, &[], b, c),
                    _ => None,
                });
                match ch {
                    Some(c) => {
                        out.push(c);
                        i += 2;
                    }
                    None => {
                        out.push(FFFD);
                        bad = true;
                        i += if trail.is_some() { 2 } else { 1 };
                    }
                }
            }
        }
    }
    (out, bad)
}

/// 1 文字を符号化できる G0 の 2 バイト集合を優先順に調べる
fn encode_iso2022jp2(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut g0 = G0Jis2::Ascii;
    let mut g2 = G2Set::None;
    let mut buf = [0u8; 4];
    for (pos, c) in text.char_indices() {
        let u = c as u32;
        if u < 0x80 {
            if g0 != G0Jis2::Ascii {
                out.extend_from_slice(b"\x1b(B");
                g0 = G0Jis2::Ascii;
            }
            out.push(u as u8);
            if c == '\n' {
                g2 = G2Set::None;
            }
            continue;
        }
        let s: &str = c.encode_utf8(&mut buf);
        // JIS X 0208
        let (e, _, bad) = encoding_rs::EUC_JP.encode(s);
        if !bad && e.len() == 2 && e[0] >= 0xA1 && e[1] >= 0xA1 {
            if g0 != G0Jis2::Jis0208 {
                out.extend_from_slice(b"\x1b$B");
                g0 = G0Jis2::Jis0208;
            }
            out.extend_from_slice(&[e[0] & 0x7F, e[1] & 0x7F]);
            continue;
        }
        // JIS X 0212
        if let Some(seq) = crate::text_loader::jis0212_lookup(c) {
            if g0 != G0Jis2::Jis0212 {
                out.extend_from_slice(b"\x1b$(D");
                g0 = G0Jis2::Jis0212;
            }
            out.extend_from_slice(&[seq[1] & 0x7F, seq[2] & 0x7F]);
            continue;
        }
        // GB 2312(GBK の符号化のうち 0xA1-0xF7 の 2 バイト)
        let (e, _, bad) = encoding_rs::GBK.encode(s);
        if !bad && e.len() == 2 && (0xA1..=0xF7).contains(&e[0]) && e[1] >= 0xA1 {
            if g0 != G0Jis2::Gb2312 {
                out.extend_from_slice(b"\x1b$A");
                g0 = G0Jis2::Gb2312;
            }
            out.extend_from_slice(&[e[0] & 0x7F, e[1] & 0x7F]);
            continue;
        }
        // KS X 1001(EUC-KR の 0xA1-0xFD の 2 バイト)
        let (e, _, bad) = encoding_rs::EUC_KR.encode(s);
        if !bad && e.len() == 2 && (0xA1..=0xFD).contains(&e[0]) && e[1] >= 0xA1 {
            if g0 != G0Jis2::Ksc5601 {
                out.extend_from_slice(b"\x1b$(C");
                g0 = G0Jis2::Ksc5601;
            }
            out.extend_from_slice(&[e[0] & 0x7F, e[1] & 0x7F]);
            continue;
        }
        // G2: ISO-8859-1 / ISO-8859-7 の上位(ESC N + 1 バイト)。ASCII へ戻してから書く
        let g2_byte = if (0xA0..=0xFF).contains(&u) {
            Some((G2Set::Latin1, u as u8 & 0x7F))
        } else {
            let (e, _, bad) = encoding_rs::ISO_8859_7.encode(s);
            (!bad && e.len() == 1 && e[0] >= 0xA0).then(|| (G2Set::Greek, e[0] & 0x7F))
        };
        if let Some((set, byte)) = g2_byte {
            if g0 != G0Jis2::Ascii {
                out.extend_from_slice(b"\x1b(B");
                g0 = G0Jis2::Ascii;
            }
            if g2 != set {
                out.extend_from_slice(if set == G2Set::Latin1 {
                    b"\x1b.A"
                } else {
                    b"\x1b.F"
                });
                g2 = set;
            }
            out.extend_from_slice(&[0x1B, b'N', byte]);
            continue;
        }
        return Err((c, pos));
    }
    if g0 != G0Jis2::Ascii {
        out.extend_from_slice(b"\x1b(B");
    }
    Ok(out)
}

// ==================== 疎な表を使う文字コード ====================

struct SparseTable {
    records: Vec<Record>,
}

impl SparseTable {
    fn get(&self, code: u16) -> Option<&Record> {
        self.records
            .binary_search_by_key(&code, |r| r.code)
            .ok()
            .map(|i| &self.records[i])
    }
}

/// Apple の変換ヒント(私用領域)。2 つ目の符号位置がこれなら無視する。
fn is_apple_hint(cp: u32) -> bool {
    (0xF87A..=0xF87F).contains(&cp)
}

fn record_chars(r: &Record) -> (char, Option<char>) {
    let a = char::from_u32(r.cp1).unwrap_or(FFFD);
    let b = if r.cp2 == 0 || is_apple_hint(r.cp2) {
        None
    } else {
        char::from_u32(r.cp2)
    };
    (a, b)
}

/// 文字(と任意の 2 文字目)→ コード。ヒント付きのものは、他に無い文字のときだけ登録する。
fn build_reverse(table: &SparseTable) -> ReverseMap {
    let mut map = HashMap::new();
    for r in table.records.iter().filter(|r| !is_apple_hint(r.cp2)) {
        map.entry(record_chars(r)).or_insert(r.code);
    }
    for r in table.records.iter().filter(|r| is_apple_hint(r.cp2)) {
        map.entry(record_chars(r)).or_insert(r.code);
    }
    map
}

// ---------- MacJapanese ----------

type ReverseMap = HashMap<(char, Option<char>), u16>;

fn mac_table() -> &'static (SparseTable, ReverseMap) {
    static T: OnceLock<(SparseTable, ReverseMap)> = OnceLock::new();
    T.get_or_init(|| {
        let t = SparseTable {
            records: tables::parse_records(tables::MACJAPANESE),
        };
        let rev = build_reverse(&t);
        (t, rev)
    })
}

fn decode_macjapanese(data: &[u8]) -> (String, bool) {
    let (table, _) = mac_table();
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if matches!(b, 0x81..=0x9F | 0xE0..=0xFC) {
            let rec = data
                .get(i + 1)
                .and_then(|&t| table.get(((b as u16) << 8) | t as u16));
            match rec {
                Some(r) => {
                    push_cell(&mut out, record_chars(r));
                    i += 2;
                }
                None => {
                    out.push(FFFD);
                    bad = true;
                    i += if data.get(i + 1).is_some_and(|&t| t >= 0x40 && t != 0x7F) {
                        2
                    } else {
                        1
                    };
                }
            }
        } else {
            match table.get(b as u16) {
                Some(r) => {
                    push_cell(&mut out, record_chars(r));
                }
                None => {
                    out.push(FFFD);
                    bad = true;
                }
            }
            i += 1;
        }
    }
    (out, bad)
}

fn encode_macjapanese(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let (_, rev) = mac_table();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let (pos, c) = chars[i];
        let pair = chars
            .get(i + 1)
            .and_then(|n| rev.get(&(c, Some(n.1))).map(|code| (*code, 2)));
        let (code, n) = match pair.or_else(|| rev.get(&(c, None)).map(|code| (*code, 1))) {
            Some(x) => x,
            None => return Err((c, pos)),
        };
        if code > 0xFF {
            out.push((code >> 8) as u8);
        }
        out.push(code as u8);
        i += n;
    }
    Ok(out)
}

// ---------- EBCDIC(単バイト) ----------

fn sbcs(idx: usize) -> &'static [u16] {
    static TABLES: OnceLock<Vec<Vec<u16>>> = OnceLock::new();
    let all = TABLES.get_or_init(|| {
        tables::EBCDIC_SBCS
            .chunks_exact(512)
            .map(|t| {
                t.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect()
            })
            .collect()
    });
    &all[idx]
}

fn sbcs_reverse(idx: usize) -> &'static HashMap<char, u8> {
    static MAPS: OnceLock<Vec<HashMap<char, u8>>> = OnceLock::new();
    let all = MAPS.get_or_init(|| {
        (0..7)
            .map(|k| {
                let mut m = HashMap::new();
                for (b, &cp) in sbcs(k).iter().enumerate() {
                    if cp != 0xFFFF {
                        if let Some(c) = char::from_u32(cp as u32) {
                            m.entry(c).or_insert(b as u8);
                        }
                    }
                }
                m
            })
            .collect()
    });
    &all[idx]
}

/// EBCDIC の NEL(0x15)は LF として読む(LF 0x25 も LF)
fn ebcdic_byte(idx: usize, b: u8) -> Option<char> {
    if b == 0x15 {
        return Some('\n');
    }
    match sbcs(idx)[b as usize] {
        0xFFFF => None,
        cp => char::from_u32(cp as u32),
    }
}

fn decode_ebcdic(idx: usize, data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    for &b in data {
        match ebcdic_byte(idx, b) {
            Some(c) => out.push(c),
            None => {
                out.push(FFFD);
                bad = true;
            }
        }
    }
    (out, bad)
}

fn encode_ebcdic(idx: usize, text: &str) -> Result<Vec<u8>, (char, usize)> {
    let rev = sbcs_reverse(idx);
    text.char_indices()
        .map(|(pos, c)| rev.get(&c).copied().ok_or((c, pos)))
        .collect()
}

// ---------- 日本語 EBCDIC(IBM930 / IBM939) ----------

struct Dbcs {
    table: SparseTable,
    reverse: HashMap<char, u16>,
}

fn dbcs(which: usize) -> &'static Dbcs {
    static T: OnceLock<[Dbcs; 2]> = OnceLock::new();
    let all = T.get_or_init(|| {
        [tables::IBM930_DBCS, tables::IBM939_DBCS].map(|blob| {
            let table = SparseTable {
                records: tables::parse_records(blob),
            };
            let mut reverse = HashMap::new();
            for r in &table.records {
                if r.cp2 == 0 {
                    if let Some(c) = char::from_u32(r.cp1) {
                        reverse.entry(c).or_insert(r.code);
                    }
                }
            }
            Dbcs { table, reverse }
        })
    });
    &all[which]
}

fn decode_ebcdic_jp(which: usize, sbcs_idx: usize, data: &[u8]) -> (String, bool) {
    let d = dbcs(which);
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut double = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0x0E if !double => {
                double = true;
                i += 1;
            }
            0x0F if double => {
                double = false;
                i += 1;
            }
            _ if double => {
                let rec = data
                    .get(i + 1)
                    .and_then(|&t| d.table.get(((b as u16) << 8) | t as u16));
                if b == 0x40 && data.get(i + 1) == Some(&0x40) {
                    out.push('\u{3000}');
                    i += 2;
                } else if let Some(r) = rec {
                    push_cell(&mut out, record_chars(r));
                    i += 2;
                } else {
                    out.push(FFFD);
                    bad = true;
                    i += if i + 1 < data.len() && data[i + 1] != 0x0F {
                        2
                    } else {
                        1
                    };
                }
            }
            _ => {
                match ebcdic_byte(sbcs_idx, b) {
                    Some(c) => out.push(c),
                    None => {
                        out.push(FFFD);
                        bad = true;
                    }
                }
                i += 1;
            }
        }
    }
    (out, bad)
}

fn encode_ebcdic_jp(which: usize, sbcs_idx: usize, text: &str) -> Result<Vec<u8>, (char, usize)> {
    let d = dbcs(which);
    let rev = sbcs_reverse(sbcs_idx);
    let mut out = Vec::with_capacity(text.len() * 2);
    let mut double = false;
    for (pos, c) in text.char_indices() {
        if let Some(&b) = rev.get(&c) {
            if double {
                out.push(0x0F);
                double = false;
            }
            out.push(b);
            continue;
        }
        let code = if c == '\u{3000}' {
            Some(0x4040)
        } else {
            d.reverse.get(&c).copied()
        };
        match code {
            Some(code) => {
                if !double {
                    out.push(0x0E);
                    double = true;
                }
                out.extend_from_slice(&code.to_be_bytes());
            }
            None => return Err((c, pos)),
        }
    }
    if double {
        out.push(0x0F);
    }
    Ok(out)
}

// ==================== UTF-7 ====================

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_value(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some((b - b'A') as u32),
        b'a'..=b'z' => Some((b - b'a') as u32 + 26),
        b'0'..=b'9' => Some((b - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn decode_utf7(data: &[u8]) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut bad = false;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b != b'+' {
            if b >= 0x80 {
                out.push(FFFD);
                bad = true;
            } else {
                out.push(b as char);
            }
            i += 1;
            continue;
        }
        // "+-" は "+"
        if data.get(i + 1) == Some(&b'-') {
            out.push('+');
            i += 2;
            continue;
        }
        i += 1;
        let mut bits: u32 = 0;
        let mut nbits = 0u32;
        let mut units: Vec<u16> = Vec::new();
        let start_len = i;
        while let Some(&c) = data.get(i) {
            match b64_value(c) {
                Some(v) => {
                    bits = (bits << 6) | v;
                    nbits += 6;
                    if nbits >= 16 {
                        nbits -= 16;
                        units.push(((bits >> nbits) & 0xFFFF) as u16);
                        bits &= (1 << nbits) - 1;
                    }
                    i += 1;
                }
                None => break,
            }
        }
        // 残りビットは 0 で、6 ビット未満でなければならない
        if i == start_len || bits != 0 || nbits >= 6 {
            bad = true;
        }
        for r in char::decode_utf16(units) {
            match r {
                Ok(c) => out.push(c),
                Err(_) => {
                    out.push(FFFD);
                    bad = true;
                }
            }
        }
        if data.get(i) == Some(&b'-') {
            i += 1;
        }
    }
    (out, bad)
}

/// 直接書ける文字(集合 D + 集合 O + 空白類。`+`・`\`・`~` は除く)
fn utf7_direct(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '\'' | '('
                | ')'
                | ','
                | '-'
                | '.'
                | '/'
                | ':'
                | '?'
                | ' '
                | '\t'
                | '\r'
                | '\n'
                | '!'
                | '"'
                | '#'
                | '$'
                | '%'
                | '&'
                | '*'
                | ';'
                | '<'
                | '='
                | '>'
                | '@'
                | '['
                | ']'
                | '^'
                | '_'
                | '`'
                | '{'
                | '|'
                | '}'
        )
}

fn encode_utf7(text: &str) -> Result<Vec<u8>, (char, usize)> {
    let mut out = Vec::with_capacity(text.len() * 2);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '+' {
            out.extend_from_slice(b"+-");
            i += 1;
        } else if utf7_direct(c) {
            out.push(c as u8);
            i += 1;
        } else {
            let mut units: Vec<u16> = Vec::new();
            while i < chars.len() && chars[i] != '+' && !utf7_direct(chars[i]) {
                let mut buf = [0u16; 2];
                units.extend_from_slice(chars[i].encode_utf16(&mut buf));
                i += 1;
            }
            out.push(b'+');
            let mut bits: u32 = 0;
            let mut nbits = 0u32;
            for u in units {
                bits = (bits << 16) | u as u32;
                nbits += 16;
                while nbits >= 6 {
                    nbits -= 6;
                    out.push(B64[((bits >> nbits) & 0x3F) as usize]);
                    bits &= (1 << nbits) - 1;
                }
            }
            if nbits > 0 {
                out.push(B64[((bits << (6 - nbits)) & 0x3F) as usize]);
            }
            // 末尾、または次が Base64 の文字か `-` のときは `-` で閉じる
            let needs_terminator = match chars.get(i) {
                None => true,
                Some(&n) => n.is_ascii_alphanumeric() || n == '+' || n == '/' || n == '-',
            };
            if needs_terminator {
                out.push(b'-');
            }
        }
    }
    Ok(out)
}

// ==================== 公開する文字コード ====================

macro_rules! ebcdic_codec {
    ($name:ident, $idx:expr) => {
        pub(crate) static $name: CustomCodec = CustomCodec {
            decode: |d| decode_ebcdic($idx, d),
            encode: |t| encode_ebcdic($idx, t),
        };
    };
}

ebcdic_codec!(IBM037, 0);
ebcdic_codec!(IBM500, 1);
ebcdic_codec!(IBM1140, 2);
ebcdic_codec!(IBM273, 3);
ebcdic_codec!(IBM290, 4);

pub(crate) static IBM930: CustomCodec = CustomCodec {
    decode: |d| decode_ebcdic_jp(0, 5, d),
    encode: |t| encode_ebcdic_jp(0, 5, t),
};
pub(crate) static IBM939: CustomCodec = CustomCodec {
    decode: |d| decode_ebcdic_jp(1, 6, d),
    encode: |t| encode_ebcdic_jp(1, 6, t),
};
pub(crate) static SHIFT_JIS_2004: CustomCodec = CustomCodec {
    decode: decode_sjis2004,
    encode: encode_sjis2004,
};
pub(crate) static EUC_JIS_2004: CustomCodec = CustomCodec {
    decode: decode_eucjis2004,
    encode: encode_eucjis2004,
};
pub(crate) static ISO_2022_JP_2004: CustomCodec = CustomCodec {
    decode: decode_iso2022jp2004,
    encode: encode_iso2022jp2004,
};
pub(crate) static ISO_2022_JP_2: CustomCodec = CustomCodec {
    decode: decode_iso2022jp2,
    encode: encode_iso2022jp2,
};
pub(crate) static MAC_JAPANESE: CustomCodec = CustomCodec {
    decode: decode_macjapanese,
    encode: encode_macjapanese,
};
pub(crate) static UTF_7: CustomCodec = CustomCodec {
    decode: decode_utf7,
    encode: encode_utf7,
};
