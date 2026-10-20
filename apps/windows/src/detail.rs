//! 詳細差分ビューのモデル(Swift `DiffDetailView` 相当)。
//! 選択エントリ → 表示内容(メッセージ/テキスト差分/Hex/画像)の決定、
//! 差分行ナビゲーション・検索・コンテキスト折りたたみの状態機械を保持する。
//! すべて純粋ロジックなのでホスト OS でテスト可能。

use diffloupe_core::diff_display as dd;
use diffloupe_core::inline_diff::{self, InlineHighlight};
use diffloupe_core::line_diff::{self, DiffRow};
use diffloupe_core::models::{ComparedEntry, DiffStatus};
use diffloupe_core::options::ContentComparison;
use diffloupe_core::structured;
use diffloupe_core::text_loader::{self, TextLoadResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 画像比較に回す拡張子(Swift `ImageFileDetector.supportedExtensions` 相当)。
/// デコードは OS の画像デコーダ(Windows: WIC)が担い、未対応形式は読み込みエラーになる。
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "gif", "bmp", "tif", "tiff", "heic", "heif", "webp",
];

pub fn is_supported_image_file(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    SUPPORTED_IMAGE_EXTENSIONS.contains(&ext.as_str())
}

/// 構造化比較の表示情報(Swift `StructuredInfo` 相当)
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredInfo {
    pub format: Option<structured::StructuredFormat>,
    pub did_normalize: bool,
    pub left_notice: Option<String>,
    pub right_notice: Option<String>,
}

impl StructuredInfo {
    pub fn fallback_reason_text(&self) -> String {
        let mut parts = Vec::new();
        if let Some(n) = &self.left_notice {
            if !n.is_empty() {
                parts.push(format!("左: {}", n));
            }
        }
        if let Some(n) = &self.right_notice {
            if !n.is_empty() {
                parts.push(format!("右: {}", n));
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(" / "))
        }
    }

    /// バッジ表示文字列(Swift `StructuredBadge.badgeText` 相当)
    pub fn badge_text(&self) -> String {
        if self.did_normalize {
            format!(
                "{}正規化で比較",
                self.format.map(|f| f.display_name()).unwrap_or("構造化")
            )
        } else {
            format!("構造化正規化に失敗{}", self.fallback_reason_text())
        }
    }
}

/// テキスト差分の表示用データ
#[derive(Debug)]
pub struct DiffDetail {
    pub rows: Vec<DiffRow>,
    pub highlights: std::collections::HashMap<usize, InlineHighlight>,
    pub left_encoding: &'static str,
    pub right_encoding: &'static str,
    pub structured_info: Option<StructuredInfo>,
}

/// 詳細ペインの表示内容(Swift `DiffDetailView.DetailState` 相当)
#[derive(Debug)]
pub enum DetailContent {
    /// 未選択
    Empty,
    /// 説明・エラーメッセージのみ表示
    Message(String),
    /// テキスト行差分
    Diff(Box<DiffDetail>),
    /// Hex バイナリ比較(左右ファイルパス)
    Hex { left: PathBuf, right: PathBuf },
    /// 画像比較(左右ファイルパス)
    Image { left: PathBuf, right: PathBuf },
}

/// 選択エントリから表示内容を決定する(Swift `DiffDetailView.load` 相当)。
/// ファイル読み込み・差分計算を含むため重い — UI 層はワーカースレッドから呼ぶこと。
pub fn compute_detail(entry: &ComparedEntry, cc: &ContentComparison) -> DetailContent {
    use crate::strings::*;
    if entry.is_directory {
        return DetailContent::Message(DETAIL_IS_DIR.into());
    }
    match entry.status {
        DiffStatus::LeftOnly => return DetailContent::Message(DETAIL_LEFT_ONLY.into()),
        DiffStatus::RightOnly => return DetailContent::Message(DETAIL_RIGHT_ONLY.into()),
        DiffStatus::Error => {
            return DetailContent::Message(format!(
                "読み込みエラー: {}",
                entry.error_message.as_deref().unwrap_or("不明")
            ))
        }
        _ => {}
    }
    let (Some(left), Some(right)) = (entry.left_path.clone(), entry.right_path.clone()) else {
        return DetailContent::Message(DETAIL_NO_TARGET.into());
    };

    if is_supported_image_file(&left) && is_supported_image_file(&right) {
        return DetailContent::Image { left, right };
    }
    if entry.status == DiffStatus::Identical {
        return DetailContent::Message(DETAIL_IDENTICAL.into());
    }
    compute_text_or_hex(&left, &right, cc)
}

fn compute_text_or_hex(left: &Path, right: &Path, cc: &ContentComparison) -> DetailContent {
    use crate::strings::*;
    let l = match text_loader::load(left) {
        Ok(v) => v,
        Err(e) => return DetailContent::Message(format!("読み込みエラー: {}", e)),
    };
    let r = match text_loader::load(right) {
        Ok(v) => v,
        Err(e) => return DetailContent::Message(format!("読み込みエラー: {}", e)),
    };
    match (l, r) {
        (TextLoadResult::Binary, _) | (_, TextLoadResult::Binary) => DetailContent::Hex {
            left: left.to_path_buf(),
            right: right.to_path_buf(),
        },
        (TextLoadResult::TooLarge, _) | (_, TextLoadResult::TooLarge) => {
            DetailContent::Message(DETAIL_TOO_LARGE.into())
        }
        (TextLoadResult::Text(lt, lenc), TextLoadResult::Text(rt, renc)) => {
            // 構造化モード: 両側とも正規化に成功した場合のみ正規化テキストを比較・表示する。
            if cc.structured_mode != structured::StructuredComparisonMode::Disabled {
                let lres = structured::normalize(
                    lt.as_bytes(),
                    Some(&left.to_string_lossy()),
                    cc.structured_mode,
                );
                let rres = structured::normalize(
                    rt.as_bytes(),
                    Some(&right.to_string_lossy()),
                    cc.structured_mode,
                );
                if lres.did_normalize && rres.did_normalize {
                    let nl = lres.normalized_text.clone().unwrap();
                    let nr = rres.normalized_text.clone().unwrap();
                    if let Some(rows) = line_diff::rows(&nl, &nr, cc) {
                        let highlights = inline_diff::highlights_for_rows(&rows, cc.ignore_case);
                        return DetailContent::Diff(Box::new(DiffDetail {
                            rows,
                            highlights,
                            left_encoding: lenc,
                            right_encoding: renc,
                            structured_info: Some(StructuredInfo {
                                format: lres.format.or(rres.format),
                                did_normalize: true,
                                left_notice: None,
                                right_notice: None,
                            }),
                        }));
                    }
                    return DetailContent::Message(DETAIL_TOO_MANY_LINES.into());
                }
                // フォールバック: 元テキストの比較 + 警告情報
                let info = StructuredInfo {
                    format: lres.format.or(rres.format),
                    did_normalize: false,
                    left_notice: lres.notice.clone(),
                    right_notice: rres.notice.clone(),
                };
                if let Some(rows) = line_diff::rows(&lt, &rt, cc) {
                    let highlights = inline_diff::highlights_for_rows(&rows, cc.ignore_case);
                    return DetailContent::Diff(Box::new(DiffDetail {
                        rows,
                        highlights,
                        left_encoding: lenc,
                        right_encoding: renc,
                        structured_info: Some(info),
                    }));
                }
                return DetailContent::Message(DETAIL_TOO_MANY_LINES.into());
            }
            if let Some(rows) = line_diff::rows(&lt, &rt, cc) {
                let highlights = inline_diff::highlights_for_rows(&rows, cc.ignore_case);
                DetailContent::Diff(Box::new(DiffDetail {
                    rows,
                    highlights,
                    left_encoding: lenc,
                    right_encoding: renc,
                    structured_info: None,
                }))
            } else {
                DetailContent::Message(DETAIL_TOO_MANY_LINES.into())
            }
        }
    }
}

/// 差分行ナビゲーション・検索・折りたたみの状態(Swift `DiffDetailView` の @State 相当)
#[derive(Debug, Default)]
pub struct DiffNav {
    /// 差分行リスト内のインデックス(None = 未選択)
    pub diff_index: Option<usize>,
    pub search_text: String,
    pub search_case_sensitive: bool,
    pub current_search_hit: Option<usize>,
    /// 展開済み折りたたみグループID。既定は折りたたみ
    pub expanded_groups: HashSet<usize>,
    /// 折りたたみを無効にして全行表示
    pub show_all_context_rows: bool,
}

impl DiffNav {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn difference_row_ids(rows: &[DiffRow]) -> Vec<usize> {
        dd::difference_row_ids(rows)
    }

    pub fn search_hit_row_ids(&self, rows: &[DiffRow]) -> Vec<usize> {
        dd::search_hit_row_ids(rows, &self.search_text, self.search_case_sensitive)
    }

    /// 現在の差分位置の表示文字列("—/N" など)
    pub fn diff_position_text(&self, rows: &[DiffRow]) -> String {
        let n = Self::difference_row_ids(rows).len();
        if n == 0 {
            return String::new();
        }
        format!(
            "{}/{}",
            self.diff_index
                .map(|i| i + 1)
                .map(|i| i.to_string())
                .unwrap_or_else(|| "—".into()),
            n
        )
    }

    /// 前後の差分行へ移動し、対象行IDを返す(なければ None)
    pub fn jump_to_difference(&mut self, rows: &[DiffRow], offset: isize) -> Option<usize> {
        let ids = Self::difference_row_ids(rows);
        if ids.is_empty() {
            return None;
        }
        let current = self
            .diff_index
            .map(|i| i as isize)
            .unwrap_or(if offset > 0 { -1 } else { ids.len() as isize });
        let next = current + offset;
        if next < 0 || next >= ids.len() as isize {
            return None;
        }
        self.diff_index = Some(next as usize);
        Some(ids[next as usize])
    }

    /// 前後の検索ヒットへ移動し、対象行IDを返す
    pub fn jump_to_search_hit(&mut self, rows: &[DiffRow], offset: isize) -> Option<usize> {
        let ids = self.search_hit_row_ids(rows);
        if ids.is_empty() {
            return None;
        }
        let current = self
            .current_search_hit
            .map(|i| i as isize)
            .unwrap_or(if offset > 0 { -1 } else { ids.len() as isize });
        let next = current + offset;
        if next < 0 || next >= ids.len() as isize {
            return None;
        }
        self.current_search_hit = Some(next as usize);
        Some(ids[next as usize])
    }

    /// 行IDへスクロールするための処理。対象行が折りたたみ内なら展開し、
    /// 表示アイテム上のインデックスを返す。
    /// (Swift `requestScrollToRow` 相当)
    pub fn reveal_row(&mut self, rows: &[DiffRow], row_id: usize) -> Option<usize> {
        if let Some(group) = dd::fold_groups(rows, dd::COLLAPSE_THRESHOLD)
            .into_iter()
            .find(|g| g.row_ids.contains(&row_id))
        {
            self.expanded_groups.insert(group.id);
        }
        let items = self.display_items(rows);
        items.iter().position(|item| match item {
            dd::DisplayItem::Row(r) => r.id == row_id,
            _ => false,
        })
    }

    /// 検索ヒットを含む折りたたみグループを展開する(クエリ空なら何もしない)
    /// (Swift `expandFoldGroupsForSearch` 相当)
    pub fn expand_fold_groups_for_search(&mut self, rows: &[DiffRow]) {
        let hits = self.search_hit_row_ids(rows);
        if hits.is_empty() {
            return;
        }
        self.expanded_groups
            .extend(dd::expanded_group_ids(&hits, rows, dd::COLLAPSE_THRESHOLD));
    }

    /// 表示アイテム列(折りたたみ適用後)
    pub fn display_items(&self, rows: &[DiffRow]) -> Vec<dd::DisplayItem> {
        dd::display_items(
            rows,
            &self.expanded_groups,
            self.show_all_context_rows,
            dd::COLLAPSE_THRESHOLD,
        )
    }

    /// 検索語の変更時に呼ぶ(ヒット位置をリセットし、ヒットを含む折りたたみを展開)
    pub fn on_search_changed(&mut self, rows: &[DiffRow]) {
        self.current_search_hit = None;
        self.expand_fold_groups_for_search(rows);
    }
}

/// メタデータヘッダ用の整形(Swift `MetadataBar` 相当)
pub fn size_text(size: i64) -> String {
    if size < 0 {
        "—".into()
    } else {
        crate::viewmodel::byte_count_text(size)
    }
}

/// epoch 秒(f64) → "yyyy-MM-dd HH:mm:ss" (UTC)
/// ローカル時刻化は UI 層で行う(ここではテスト可能な UTC 版のみ)。
pub fn date_text_utc(epoch: Option<f64>) -> String {
    let Some(t) = epoch else { return "—".into() };
    let ms = (t * 1000.0).round_ties_even();
    let secs = (ms / 1000.0).floor() as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil_from_days(diffloupe-core の report.rs と同じ変換)
    let (y, mo, d) = {
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z.rem_euclid(146097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        mo,
        d,
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// パーミッション表示(Swift `posixText` 相当)
pub fn posix_text(meta: Option<diffloupe_core::models::PosixBasicMetadata>) -> String {
    match meta.and_then(|m| m.permissions) {
        Some(p) => format!("{:04o}", p),
        None => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffloupe_core::models::ComparedEntry;

    fn entry(status: DiffStatus, is_dir: bool, l: Option<&str>, r: Option<&str>) -> ComparedEntry {
        ComparedEntry {
            relative_path: "a.txt".into(),
            name: "a.txt".into(),
            is_directory: is_dir,
            status,
            left_path: l.map(PathBuf::from),
            right_path: r.map(PathBuf::from),
            left_size: 0,
            right_size: 0,
            error_message: None,
            left_modification_date: None,
            right_modification_date: None,
            left_posix: None,
            right_posix: None,
            rename_candidate: None,
        }
    }

    #[test]
    fn directory_shows_message() {
        let e = entry(DiffStatus::Different, true, Some("/a"), Some("/b"));
        assert!(matches!(
            compute_detail(&e, &ContentComparison::default()),
            DetailContent::Message(_)
        ));
    }

    #[test]
    fn left_only_shows_message() {
        let e = entry(DiffStatus::LeftOnly, false, Some("/a"), None);
        assert!(
            matches!(compute_detail(&e, &ContentComparison::default()), DetailContent::Message(m) if m.contains("左"))
        );
    }

    #[test]
    fn identical_shows_message() {
        let e = entry(DiffStatus::Identical, false, Some("/a"), Some("/b"));
        assert!(
            matches!(compute_detail(&e, &ContentComparison::default()), DetailContent::Message(m) if m.contains("一致"))
        );
    }

    #[test]
    fn nav_jump_unselected_next_goes_first() {
        let rows = line_diff::rows("a\nb\nc", "a\nx\nc", &ContentComparison::default()).unwrap();
        let mut nav = DiffNav::default();
        assert_eq!(nav.jump_to_difference(&rows, 1), Some(1));
        assert_eq!(nav.diff_index, Some(0));
        // 先頭から前は動かない
        assert_eq!(nav.jump_to_difference(&rows, -1), None);
    }

    #[test]
    fn nav_jump_unselected_prev_goes_last() {
        let rows = line_diff::rows("a\nb\nc", "x\nb\ny", &ContentComparison::default()).unwrap();
        let mut nav = DiffNav::default();
        let last = nav.jump_to_difference(&rows, -1);
        assert_eq!(last, Some(2));
    }

    #[test]
    fn reveal_row_expands_fold_group() {
        // context 7行 + 差分1行: 折りたたみ対象
        let l = "d\n1\n2\n3\n4\n5\n6\n7";
        let r = "x\n1\n2\n3\n4\n5\n6\n7";
        let rows = line_diff::rows(l, r, &ContentComparison::default()).unwrap();
        let mut nav = DiffNav::default();
        let items = nav.display_items(&rows);
        // 先頭行が差分なので context 7行が畳まれる
        assert!(items
            .iter()
            .any(|i| matches!(i, dd::DisplayItem::Collapsed(_))));
        let idx = nav.reveal_row(&rows, 5).unwrap();
        assert!(matches!(
            nav.display_items(&rows)[idx],
            dd::DisplayItem::Row(_)
        ));
        assert!(!nav.expanded_groups.is_empty());
    }

    #[test]
    fn search_expands_folded_hits() {
        let l = "d\n1\n2\n3\n4\n5\nneedle\n8";
        let r = "x\n1\n2\n3\n4\n5\nneedle\n8";
        let rows = line_diff::rows(l, r, &ContentComparison::default()).unwrap();
        let mut nav = DiffNav {
            search_text: "needle".into(),
            ..Default::default()
        };
        nav.on_search_changed(&rows);
        assert!(!nav.expanded_groups.is_empty());
    }

    #[test]
    fn date_text_format() {
        assert_eq!(date_text_utc(None), "—");
        assert_eq!(date_text_utc(Some(0.0)), "1970-01-01 00:00:00");
    }
}
