//! ユーザー定義glob(`*` `?` `**`)。Swift `GlobPattern` 相当。
//! 照合は grapheme(拡張書記素クラスタ)単位 + NFC 等価。

use crate::unicode::{graphemes, nfc};

#[derive(Debug, Clone)]
pub struct GlobPattern {
    pub pattern: String,
    has_separator: bool,
    tokens: Vec<String>,
}

impl GlobPattern {
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            has_separator: pattern.contains('/'),
            tokens: pattern
                .split('/')
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect(),
        }
    }

    /// name: 最終パス成分 / relative_path: "/"区切り(NFC正規化済み契約)
    pub fn matches(&self, name: &str, relative_path: &str) -> bool {
        let target = if self.has_separator {
            relative_path
        } else {
            name
        };
        let path: Vec<String> = target
            .split('/')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();
        Self::match_tokens(&self.tokens, &path)
    }

    fn match_tokens(tokens: &[String], path: &[String]) -> bool {
        if tokens.is_empty() {
            return path.is_empty();
        }
        if path.is_empty() {
            return tokens.iter().all(|t| t == "**");
        }
        let token = &tokens[0];
        if token == "**" {
            return Self::match_tokens(&tokens[1..], path)
                || Self::match_tokens(tokens, &path[1..]);
        }
        if token.contains('*') || token.contains('?') {
            if !single_token_match(token, &path[0]) {
                return false;
            }
        } else if nfc(token) != path[0] {
            // リテラルトークンは NFC 等価で比較(Swift Character 等価)
            return false;
        }
        Self::match_tokens(&tokens[1..], &path[1..])
    }
}

/// `*`(区切りをまたがない任意列)/`?`(1 grapheme)の貪欲+バックトラック照合
fn single_token_match(token: &str, candidate: &str) -> bool {
    let pattern: Vec<String> = graphemes(token).iter().map(|s| s.to_string()).collect();
    let string: Vec<String> = graphemes(candidate).iter().map(|s| nfc(s)).collect();
    let pat_nfc: Vec<String> = pattern.iter().map(|s| nfc(s)).collect();
    let (mut si, mut pi) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while si < string.len() {
        if pi < pat_nfc.len() && (pat_nfc[pi] == "?" || pat_nfc[pi] == string[si]) {
            si += 1;
            pi += 1;
        } else if pi < pat_nfc.len() && pat_nfc[pi] == "*" {
            star = Some((pi, si));
            pi += 1;
        } else if let Some((sp, ss)) = star {
            star = Some((sp, ss + 1));
            si = ss + 1;
            pi = sp + 1;
        } else {
            return false;
        }
    }
    while pi < pat_nfc.len() && pat_nfc[pi] == "*" {
        pi += 1;
    }
    pi == pat_nfc.len()
}
