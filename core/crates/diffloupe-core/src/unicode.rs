//! Unicode ヘルパ: NFC 正規化・大文字小文字変換・grapheme 分割・
//! localizedStandardCompare 近似。生成テーブル(unicode_tables.rs)を使う。

use crate::unicode_tables::{CCC, COMPOSE, DECOMP, DECOMP_DATA, EXTEND_RANGES, LOWER, LOWER_DATA};

fn lookup_ccc(cp: u32) -> u32 {
    CCC.binary_search_by_key(&cp, |e| e.0)
        .map(|i| CCC[i].1 as u32)
        .unwrap_or(0)
}

fn decompose_into(cp: u32, out: &mut Vec<u32>) {
    if let Ok(i) = DECOMP.binary_search_by_key(&cp, |e| e.0) {
        let (_, off, len) = DECOMP[i];
        for &d in &DECOMP_DATA[off as usize..off as usize + len as usize] {
            decompose_into(d, out);
        }
    } else {
        out.push(cp);
    }
}

fn compose_lookup(a: u32, b: u32) -> Option<u32> {
    COMPOSE
        .binary_search_by_key(&(a, b), |e| (e.0, e.1))
        .map(|i| COMPOSE[i].2)
        .ok()
}

/// NFC 正規化(precomposedStringWithCanonicalMapping 相当)
pub fn nfc(s: &str) -> String {
    // 速い経路: 分解対象・結合文字を含まなければそのまま
    if !s
        .chars()
        .any(|c| (c as u32) >= 0x80 && (ccc_or_decomp(c as u32)))
    {
        return s.to_string();
    }
    // 1. 正準分解 + 正準並べ替え
    let mut seq: Vec<u32> = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose_into(c as u32, &mut seq);
    }
    // canonical ordering: 安定ソート(ccc>0 のみ移動)
    let mut i = 1;
    while i < seq.len() {
        let c = lookup_ccc(seq[i]);
        if c != 0 {
            let mut j = i;
            while j > 0 && lookup_ccc(seq[j - 1]) > c {
                seq.swap(j, j - 1);
                j -= 1;
            }
        }
        i += 1;
    }
    // 2. 再合成
    let mut out: Vec<u32> = Vec::with_capacity(seq.len());
    let mut starter: Option<usize> = None;
    let mut last_cc = 0u32;
    for &cp in &seq {
        let cc = lookup_ccc(cp);
        if let Some(si) = starter {
            // blocked: 間に ccc >= 対象ccc の文字がある(対象の ccc が 0 でない限り、
            // ccc が単調増加なので last_cc < cc で合成可能)
            let blocked = last_cc != 0 && last_cc >= cc;
            if !blocked || cc == 0 {
                // cc==0 でも合成があれば試す(例: 一部の合成)
            }
            if !blocked {
                if let Some(c) = compose_lookup(out[si], cp) {
                    out[si] = c;
                    continue;
                }
            }
        }
        if cc == 0 {
            starter = Some(out.len());
            last_cc = 0;
        } else {
            last_cc = cc;
        }
        out.push(cp);
    }
    out.iter()
        .map(|&cp| char::from_u32(cp).unwrap_or('\u{FFFD}'))
        .collect()
}

fn ccc_or_decomp(cp: u32) -> bool {
    lookup_ccc(cp) != 0 || DECOMP.binary_search_by_key(&cp, |e| e.0).is_ok()
}

/// 結合クラス (Case_Ignorable 判定の補助は行わない簡易版)
fn is_case_ignorable(c: char) -> bool {
    let cp = c as u32;
    if EXTEND_RANGES
        .binary_search_by(|&(a, b)| {
            if cp < a {
                std::cmp::Ordering::Greater
            } else if cp > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
    {
        return true;
    }
    matches!(c, '\'' | '\u{2019}' | '\u{00B7}' | ':' | '.' | '-' | '_')
}

fn is_cased(c: char) -> bool {
    c.is_uppercase() || c.is_lowercase() || c.is_alphabetic()
}

fn lower_of(c: char) -> Option<&'static [u32]> {
    LOWER
        .binary_search_by_key(&(c as u32), |e| e.0)
        .map(|i| {
            let (_, off, len) = LOWER[i];
            &LOWER_DATA[off as usize..off as usize + len as usize]
        })
        .ok()
}

/// Unicode lowercase(String.lowercased() 相当。ギリシャ語 σ/ς の文脈規則を含む)
pub fn lowercased(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == '\u{03A3}' {
            // 前: case-ignorable を飛ばして cased があるか
            let before = chars[..i]
                .iter()
                .rev()
                .find(|&&x| !is_case_ignorable(x))
                .map(|&x| is_cased(x))
                .unwrap_or(false);
            let after = chars[i + 1..]
                .iter()
                .find(|&&x| !is_case_ignorable(x))
                .map(|&x| is_cased(x))
                .unwrap_or(false);
            out.push(if before && !after {
                '\u{03C2}'
            } else {
                '\u{03C3}'
            });
            continue;
        }
        if let Some(m) = lower_of(c) {
            for &cp in m {
                out.push(char::from_u32(cp).unwrap_or(c));
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn is_extend(cp: u32) -> bool {
    EXTEND_RANGES
        .binary_search_by(|&(a, b)| {
            if cp < a {
                std::cmp::Ordering::Greater
            } else if cp > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
        || cp == 0x200D
}

fn is_regional_indicator(cp: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&cp)
}

/// Swift `Character`(拡張書記素クラスタ)の近似分割。
/// 結合文字・VS・ZWJ連結・絵文字修飾・国旗(RI対)に対応する簡易 UAX#29。
pub fn graphemes(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut out = Vec::new();
    let cps: Vec<(usize, char)> = s.char_indices().collect();
    let mut start = 0usize;
    let mut ri_run = 0usize;
    let mut prev_zwj = false;
    let mut prev_cp: Option<u32> = cps.first().map(|(_, c)| *c as u32);
    if is_regional_indicator(cps[0].1 as u32) {
        ri_run = 1;
    }
    for (idx, c) in cps.iter().skip(1) {
        let cp = *c as u32;
        let p = prev_cp.unwrap();
        let boundary = if p == 0x0D && cp == 0x0A {
            false // CRLF は 1 クラスタ
        } else if p == 0x0D || p == 0x0A || cp == 0x0D || cp == 0x0A {
            true
        } else if prev_zwj
            || (is_regional_indicator(cp) && is_regional_indicator(p) && ri_run % 2 == 1)
        {
            // ZWJ の直後、および RI のペアは分割しない
            false
        } else {
            !is_extend(cp) // 非結合文字は新しいクラスタの先頭
        };
        if boundary {
            out.push(&s[start..*idx]);
            start = *idx;
            ri_run = 0;
        }
        if is_regional_indicator(cp) {
            ri_run += 1;
        } else {
            ri_run = 0;
        }
        prev_zwj = cp == 0x200D;
        prev_cp = Some(cp);
    }
    out.push(&s[start..]);
    out
}

/// localizedStandardCompare の近似。
/// 数字連は数値比較・大文字小文字は主に無視(同格は小文字優先)・
/// 句読点は英数字の前に来る簡易照合順。レポートには使われず
/// ツリー表示の並びのみに影響する(実装上の近似)。
pub fn localized_standard_compare(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::{self, *};
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let mut case_tie: Option<Ordering> = None;
    while i < ac.len() && j < bc.len() {
        let (x, y) = (ac[i], bc[j]);
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let mut ii = i;
            while ii < ac.len() && ac[ii].is_ascii_digit() {
                ii += 1;
            }
            let mut jj = j;
            while jj < bc.len() && bc[jj].is_ascii_digit() {
                jj += 1;
            }
            let an: String = ac[i..ii].iter().collect();
            let bn: String = bc[j..jj].iter().collect();
            let at = an.trim_start_matches('0');
            let bt = bn.trim_start_matches('0');
            let ord = at
                .len()
                .cmp(&bt.len())
                .then_with(|| at.cmp(bt))
                .then_with(|| an.len().cmp(&bn.len()));
            if ord != Equal {
                return ord;
            }
            i = ii;
            j = jj;
            continue;
        }
        let (xl, yl) = (
            x.to_lowercase().next().unwrap_or(x),
            y.to_lowercase().next().unwrap_or(y),
        );
        // クラス: 空白/句読点・記号 < 英数字以外の記号 < 文字
        let rank = |c: char| -> u32 {
            if c.is_alphanumeric() {
                2
            } else if c.is_ascii_punctuation() || c == ' ' {
                0
            } else {
                1
            }
        };
        let (ra, rb) = (rank(x), rank(y));
        if ra != rb {
            return ra.cmp(&rb);
        }
        if xl != yl {
            return xl.cmp(&yl);
        }
        if x != y && case_tie.is_none() {
            case_tie = Some(if x.is_uppercase() { Greater } else { Less });
        }
        i += 1;
        j += 1;
    }
    let ord = (ac.len() - i).cmp(&(bc.len() - j));
    if ord != Equal {
        return ord;
    }
    case_tie.unwrap_or(Equal)
}
