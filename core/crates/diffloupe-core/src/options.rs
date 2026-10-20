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
        if self.structured_mode == StructuredComparisonMode::Disabled {
            flags
        } else {
            format!("{}|{}", flags, self.structured_mode.raw_value())
        }
    }

    pub const BINARY_SNIFF_LENGTH: usize = 8192;

    /// バイト列をテキスト正規化。バイナリ/UTF-8 不能なら None
    pub fn normalized_data(&self, data: &[u8]) -> Option<Vec<u8>> {
        if self.is_strict() {
            return Some(data.to_vec());
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
        let l = structured::normalize(left, left_url, self.structured_mode);
        let r = structured::normalize(right, right_url, self.structured_mode);
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
