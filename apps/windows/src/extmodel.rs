//! エディション拡張の UI 非依存モデル(追加メニューの記述など)。
//! Win32 に依存しないためホスト OS でテスト可能。実ビューの生成は
//! `win::ext::Win32EditionExtension`(cfg(windows))が担う。

/// 拡張メニューのコマンド ID 帯。組み込みコマンド(1xxx)と衝突しないよう
/// 3000 番台を割り当てる。Pro 側はこの範囲の ID を使うこと。
pub const EXT_CMD_MIN: u16 = 3000;
pub const EXT_CMD_MAX: u16 = 3999;

/// `id` が拡張メニューのコマンド ID 帯に入るか。
pub fn is_ext_command(id: u16) -> bool {
    (EXT_CMD_MIN..=EXT_CMD_MAX).contains(&id)
}

/// 追加するトップレベルメニュー(例: 「プロジェクト」)。
/// メニューバーの「ヘルプ」の手前に挿入される。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtMenu {
    /// メニュー名("プロジェクト(&P)" など)
    pub title: String,
    pub items: Vec<ExtMenuItem>,
}

/// 追加メニューの1項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtMenuItem {
    /// コマンド ID。`EXT_CMD_MIN..=EXT_CMD_MAX` の範囲で割り当てる。
    /// `SEPARATOR_ID` の場合は区切り線、`submenu_id` が `Some` の場合は動的サブメニュー。
    pub id: u16,
    pub title: String,
    /// 動的サブメニューの識別子。`Some(k)` のとき、この項目は空のサブメニューとして作られ、
    /// 開かれる直前に `build_submenu(k)` の結果で中身が組み替えられる(履歴メニュー用)。
    pub submenu_id: Option<u16>,
}

impl ExtMenuItem {
    /// 区切り線用のコマンド ID(実コマンドとしては送られない)
    pub const SEPARATOR_ID: u16 = u16::MAX;

    pub fn item(id: u16, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            submenu_id: None,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: Self::SEPARATOR_ID,
            title: String::new(),
            submenu_id: None,
        }
    }

    pub fn submenu(submenu_id: u16, title: impl Into<String>) -> Self {
        Self {
            id: 0,
            title: title.into(),
            submenu_id: Some(submenu_id),
        }
    }

    pub fn is_separator(&self) -> bool {
        self.id == Self::SEPARATOR_ID && self.submenu_id.is_none()
    }
}

/// 拡張メニュー記述の検証。コマンド ID が帯域内・一意であることを確認する。
/// 違反がある場合は最初の違反内容を返す(起動時の自己診断用)。
pub fn validate_menus(menus: &[ExtMenu]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for m in menus {
        if m.title.is_empty() {
            return Err("拡張メニューのタイトルが空です".into());
        }
        for item in &m.items {
            if item.is_separator() || item.submenu_id.is_some() {
                continue;
            }
            if !is_ext_command(item.id) {
                return Err(format!(
                    "拡張コマンド ID {} が範囲外です({}..={})",
                    item.id, EXT_CMD_MIN, EXT_CMD_MAX
                ));
            }
            if !seen.insert(item.id) {
                return Err(format!("拡張コマンド ID {} が重複しています", item.id));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_range() {
        assert!(is_ext_command(3000));
        assert!(is_ext_command(3999));
        assert!(!is_ext_command(2999));
        assert!(!is_ext_command(4000));
        assert!(!is_ext_command(1001));
    }

    #[test]
    fn validate_ok() {
        let menus = vec![ExtMenu {
            title: "プロジェクト".into(),
            items: vec![
                ExtMenuItem::item(3000, "開く"),
                ExtMenuItem::item(3001, "保存"),
                ExtMenuItem::separator(),
                ExtMenuItem::submenu(1, "最近のプロジェクト"),
                ExtMenuItem::item(3005, "レポート"),
            ],
        }];
        assert!(validate_menus(&menus).is_ok());
    }

    #[test]
    fn validate_rejects_out_of_range_and_dupes() {
        let bad = vec![ExtMenu {
            title: "x".into(),
            items: vec![ExtMenuItem::item(100, "a")],
        }];
        assert!(validate_menus(&bad).unwrap_err().contains("範囲外"));
        let dup = vec![ExtMenu {
            title: "x".into(),
            items: vec![ExtMenuItem::item(3000, "a"), ExtMenuItem::item(3000, "b")],
        }];
        assert!(validate_menus(&dup).unwrap_err().contains("重複"));
    }
}
