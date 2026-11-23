//! 内容比較モード(Swift `ComparisonOptions.swift` / `ContentComparison` 相当)

use crate::structured::{self, StructuredComparisonMode};
use crate::unicode::lowercased;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ContentComparison {
    pub ignore_line_endings: bool,
    pub ignore_trailing_whitespace: bool,
    pub normalize_whitespace: bool,
    pub ignore_case: bool,
    pub structured_mode: StructuredComparisonMode,
    /// テキストとしてデコードする経路(正規化比較・構造化比較)で、自動判定の代わりに使う文字コード
    /// (`text_loader::supported_encodings` の正規名。`text_loader::canonical_encoding_name` で得る)。
    /// 指定時は不正なバイト列を U+FFFD に置換して続行する。None なら従来どおり(UTF-8 のみ)。
    pub text_encoding: Option<&'static str>,
}

impl ContentComparison {
    pub fn is_strict(&self) -> bool {
        !self.ignore_line_endings
            && !self.ignore_trailing_whitespace
            && !self.normalize_whitespace
            && !self.ignore_case
            && self.structured_mode == StructuredComparisonMode::Disabled
    }

    pub fn settings_signature(&self) -> String {
        let flags = format!(
            "{}{}{}{}",
            self.ignore_line_endings as u8,
            self.ignore_trailing_whitespace as u8,
            self.normalize_whitespace as u8,
            self.ignore_case as u8
        );
        let base = if self.structured_mode == StructuredComparisonMode::Disabled {
            flags
        } else {
            format!("{}|{}", flags, self.structured_mode.raw_value())
        };
        match self.text_encoding {
            Some(enc) => format!("{}|enc={}", base, enc),
            None => base,
        }
    }

    /// `text_encoding` 指定時、バイト列をその文字コードで UTF-8 のテキストへ変換する。
    /// 構造化比較のパーサへ渡すため、XML 宣言の `encoding` は UTF-8 に書き換える。
    fn transcoded_for_structured<'a>(&self, data: &'a [u8]) -> std::borrow::Cow<'a, [u8]> {
        let Some(enc) = self.text_encoding else {
            return std::borrow::Cow::Borrowed(data);
        };
        let Some(decoded) = crate::text_loader::decode_as(data, enc) else {
            return std::borrow::Cow::Borrowed(data);
        };
        let mut text = decoded.text;
        neutralize_xml_encoding(&mut text);
        std::borrow::Cow::Owned(text.into_bytes())
    }

    pub const BINARY_SNIFF_LENGTH: usize = 8192;

    /// バイト列をテキスト正規化。バイナリ/UTF-8 不能なら None
    pub fn normalized_data(&self, data: &[u8]) -> Option<Vec<u8>> {
        if self.is_strict() {
            return Some(data.to_vec());
        }
        if let Some(enc) = self.text_encoding {
            // 文字コードの指定があれば、NUL の有無によらずその文字コードで読む(UTF-16 など)
            let decoded = crate::text_loader::decode_as(data, enc)?;
            return Some(self.normalized_text(&decoded.text).into_bytes());
        }
        if data[..data.len().min(Self::BINARY_SNIFF_LENGTH)].contains(&0) {
            return None;
        }
        let text = String::from_utf8(data.to_vec()).ok()?;
        // Foundation の UTF-8 デコードは先頭 BOM(U+FEFF)を除去する
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        Some(self.normalized_text(text).into_bytes())
    }

    /// 左右両側の構造化正規化。両側成功時のみカノニカルペア
    pub fn structured_canonical_pair(
        &self,
        left: &[u8],
        left_url: Option<&str>,
        right: &[u8],
        right_url: Option<&str>,
    ) -> Option<(String, String)> {
        if self.structured_mode == StructuredComparisonMode::Disabled {
            return None;
        }
        let left = self.transcoded_for_structured(left);
        let right = self.transcoded_for_structured(right);
        let l = structured::normalize(&left, left_url, self.structured_mode);
        let r = structured::normalize(&right, right_url, self.structured_mode);
        if l.did_normalize && r.did_normalize {
            Some((l.normalized_text.unwrap(), r.normalized_text.unwrap()))
        } else {
            None
        }
    }

    pub fn normalized_text(&self, text: &str) -> String {
        let mut base = text.to_string();
        if self.ignore_line_endings || self.normalize_whitespace {
            base = base.replace("\r\n", "\n").replace('\r', "\n");
        }
        base.split('\n')
            .map(|l| self.normalized_line(l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn normalized_line(&self, line: &str) -> String {
        let mut result = line.to_string();
        if self.ignore_trailing_whitespace || self.normalize_whitespace {
            // .whitespaces 相当: スペースとタブのみ(CR は残る)
            result = result.trim_end_matches([' ', '\t']).to_string();
        }
        if self.normalize_whitespace {
            result = result.split_whitespace().collect::<Vec<_>>().join(" ");
        }
        if self.ignore_case {
            result = lowercased(&result);
        }
        result
    }
}

/// 先頭の XML 宣言の `encoding="…"` を UTF-8 に書き換える(変換済みのテキストをパーサに渡すため)
fn neutralize_xml_encoding(text: &mut String) -> Option<()> {
    if !text.starts_with("<?xml") {
        return None;
    }
    let end = text.find("?>")?;
    let decl = &text[..end];
    let k = decl.find("encoding")? + "encoding".len();
    let bytes = decl.as_bytes();
    let mut j = k + decl[k..].find('=')? + 1;
    while bytes.get(j).is_some_and(|b| b.is_ascii_whitespace()) {
        j += 1;
    }
    let quote = *bytes.get(j)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let start = j + 1;
    let close = decl[start..].find(quote as char)?;
    text.replace_range(start..start + close, "UTF-8");
    Some(())
}
