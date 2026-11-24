//! 表計算ファイル(xlsx / xlsm / xls / ods)のテキスト抽出
//!
//! 動作設計:
//! - `calamine`(純 Rust)で全シートを読み、差分比較用のテキストにする。
//! - 出力形式: シートごとに `## <シート名>` の行、続けて各行をタブ区切りで出す。
//!   セル内の改行・タブは空白に置換し、行末の空セルと末尾の空行は削る。シート間は空行 1 つ。
//! - 数値は整数値なら小数点なし、日付・時刻は ISO 形式(`YYYY-MM-DD` / `YYYY-MM-DDTHH:MM:SS` /
//!   `HH:MM:SS`)で出す。数式セルは保存されている計算結果の値を使う(再計算はしない)。
//! - 暗号化されたファイルや壊れたファイルは読めずエラーになる。ファイルサイズは 64 MiB まで。

use calamine::{open_workbook_auto, Data, Reader};
use std::path::Path;

/// 読み込みを許すファイルサイズの上限(64 MiB)
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct DocumentError(pub String);
impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for DocumentError {}

/// 拡張子が xlsx / xlsm / xls / ods(大文字小文字無視)か
pub fn is_spreadsheet_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["xlsx", "xlsm", "xls", "ods"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// 全シートをテキストへ変換する
pub fn extract_spreadsheet_text(path: &Path) -> Result<String, DocumentError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| DocumentError(format!("ファイルを読み込めませんでした: {}", e)))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(DocumentError(format!(
            "ファイルが大きすぎます(上限 {} MiB)",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| DocumentError(format!("表計算ファイルを開けませんでした: {}", e)))?;
    let sheets = workbook.worksheets();

    let mut blocks: Vec<String> = Vec::new();
    for (name, range) in sheets {
        let mut lines: Vec<String> = Vec::new();
        for row in range.rows() {
            let mut cells: Vec<String> = row.iter().map(cell_text).collect();
            while cells.last().is_some_and(|c| c.is_empty()) {
                cells.pop();
            }
            lines.push(cells.join("\t"));
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        let mut block = format!("## {}", flatten(&name));
        for line in lines {
            block.push('\n');
            block.push_str(&line);
        }
        blocks.push(block);
    }
    let mut text = blocks.join("\n\n");
    if !text.is_empty() {
        text.push('\n');
    }
    Ok(text)
}

/// セル内の改行・タブを空白に置き換える
fn flatten(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\r', '\n', '\t'], " ")
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => flatten(s),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => float_text(*f),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::DateTime(dt) => {
            let value = dt.as_f64();
            if dt.is_duration() {
                duration_text(value)
            } else if value.abs() < 1.0 {
                let (_, _, _, h, mi, s, _) = dt.to_ymd_hms_milli();
                format!("{:02}:{:02}:{:02}", h, mi, s)
            } else {
                let (y, mo, d, h, mi, s, _) = dt.to_ymd_hms_milli();
                if h == 0 && mi == 0 && s == 0 {
                    format!("{:04}-{:02}-{:02}", y, mo, d)
                } else {
                    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, mo, d, h, mi, s)
                }
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => flatten(s),
        Data::Error(e) => e.to_string(),
    }
}

fn float_text(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

/// 経過時間(日単位の値)を `[h]:mm:ss` 形式へ
fn duration_text(days: f64) -> String {
    let total = (days * 86_400.0).round() as i64;
    let sign = if total < 0 { "-" } else { "" };
    let t = total.abs();
    format!("{}{}:{:02}:{:02}", sign, t / 3600, (t / 60) % 60, t % 60)
}
