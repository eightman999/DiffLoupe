//! 比較レポート(JSON / CSV / Markdown / HTML)。Swift `ComparisonReport` 相当。
//! JSON は JSONSerialization(.prettyPrinted + .sortedKeys) のバイト互換出力。

use crate::models::{ComparedEntry, ComparisonResult, DiffStatus, RenameCandidate};
use crate::structured::json_escape_string;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Html,
    Csv,
    Json,
    Markdown,
}

impl ReportFormat {
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }
}

/// Foundation `ISO8601DateFormatter`(.withInternetDateTime・UTC)の観測規則:
/// Date(秒 f64)を最近接ミリ秒へ丸め(同値は偶数丸めの lrint 相当)たあと、
/// 秒未満を切り捨てて表示する。したがって小数部が .9995 以上のときだけ
/// 表示秒が1つ繰り上がり、.5/.5001 では繰り上がらない。
/// (例: 100.5001→:00, 100.9995→:01, -0.0005→1970-01-01T00:00:00Z,
///  -1.0005→1969-12-31T23:59:59Z。実機の Swift CLI で確認済み)
pub fn iso8601_date(t: f64) -> String {
    let ms = (t * 1000.0).round_ties_even();
    iso8601((ms / 1000.0).floor() as i64)
}

/// `"yyyy-MM-ddTHH:mm:ssZ"`(任意で小数秒)を epoch 秒へ。UTC の `Z` 終端のみ受理。不正なら None。
/// `iso8601_date` の逆変換(Foundation の `.iso8601` 戦略相当)。
pub fn parse_iso8601(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |lo: usize, hi: usize| -> Option<i64> {
        let part = &b[lo..hi];
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(part).ok()?.parse::<i64>().ok()
    };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut frac = 0.0f64;
    let mut idx = 19usize;
    if b.get(idx) == Some(&b'.') {
        idx += 1;
        let start = idx;
        let mut scale = 0.1f64;
        while let Some(&c) = b.get(idx) {
            if !c.is_ascii_digit() {
                break;
            }
            frac += ((c - b'0') as f64) * scale;
            scale /= 10.0;
            idx += 1;
        }
        if idx == start {
            return None;
        }
    }
    if b.get(idx) != Some(&b'Z') || idx + 1 != b.len() {
        return None;
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    Some(days as f64 * 86400.0 + (h * 3600 + mi * 60 + sec) as f64 + frac)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// epoch 秒 → ISO8601 UTC "YYYY-MM-DDTHH:MM:SSZ"
pub fn iso8601(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

pub struct ComparisonReport<'a> {
    pub result: &'a ComparisonResult,
    /// epoch 秒
    pub generated_at: i64,
    pub left_git_ref: Option<String>,
    pub right_git_ref: Option<String>,
}

const FORMAT_NAME: &str = "DiffLoupe比較レポート";

fn status_label(s: DiffStatus) -> &'static str {
    match s {
        DiffStatus::Identical => "一致",
        DiffStatus::Different => "相違",
        DiffStatus::LeftOnly => "左のみ",
        DiffStatus::RightOnly => "右のみ",
        DiffStatus::Error => "エラー",
    }
}

fn size_text(s: i64) -> String {
    if s >= 0 {
        s.to_string()
    } else {
        "-".into()
    }
}

fn kind_text(d: bool) -> &'static str {
    if d {
        "ディレクトリ"
    } else {
        "ファイル"
    }
}

fn rename_text(c: &Option<RenameCandidate>) -> String {
    match c {
        None => String::new(),
        Some(c) if c.is_unique_match => c.counterpart_path.clone(),
        Some(c) => format!("{}（候補{}件）", c.counterpart_path, c.total_matches),
    }
}

fn side_meta(path: &str, git_ref: &Option<String>) -> String {
    match git_ref {
        Some(r) => format!("{} @ {}", path, r),
        None => path.to_string(),
    }
}

const ENTRY_HEADER: [&str; 9] = [
    "相対パス",
    "状態",
    "種別",
    "左サイズ",
    "右サイズ",
    "左更新日時",
    "右更新日時",
    "リネーム候補",
    "エラー",
];

fn entry_row(e: &ComparedEntry) -> Vec<String> {
    vec![
        e.relative_path.clone(),
        status_label(e.status).into(),
        kind_text(e.is_directory).into(),
        size_text(e.left_size),
        size_text(e.right_size),
        e.left_modification_date
            .map(iso8601_date)
            .unwrap_or_default(),
        e.right_modification_date
            .map(iso8601_date)
            .unwrap_or_default(),
        rename_text(&e.rename_candidate),
        e.error_message.clone().unwrap_or_default(),
    ]
}

impl<'a> ComparisonReport<'a> {
    pub fn new(result: &'a ComparisonResult, generated_at: i64) -> Self {
        Self {
            result,
            generated_at,
            left_git_ref: None,
            right_git_ref: None,
        }
    }

    pub fn render(&self, format: ReportFormat) -> String {
        match format {
            ReportFormat::Json => self.render_json(),
            ReportFormat::Csv => self.render_csv(),
            ReportFormat::Markdown => self.render_markdown(),
            ReportFormat::Html => self.render_html(),
        }
    }

    fn summary_rows(&self) -> [(&'static str, usize); 5] {
        [
            ("一致", self.result.summary.identical_files),
            ("相違", self.result.summary.different_files),
            ("左のみ", self.result.summary.left_only_files),
            ("右のみ", self.result.summary.right_only_files),
            ("エラー", self.result.summary.error_files),
        ]
    }

    // MARK: - JSON(JSONSerialization pretty + sortedKeys バイト互換)

    fn render_json(&self) -> String {
        let r = self.result;
        let mut out = String::new();
        out.push_str("{\n");
        // sortedKeys(collation)順: entries, format, generatedAt, left, right, summary, version
        out.push_str("  \"entries\" : [");
        if r.entries.is_empty() {
            out.push_str("\n\n  ]");
        } else {
            out.push('\n');
            let items: Vec<String> = r
                .entries
                .iter()
                .map(|e| format!("{}{}", "  ".repeat(2), entry_json(e, 2)))
                .collect();
            out.push_str(&items.join(",\n"));
            out.push_str("\n  ]");
        }
        out.push_str(",\n");
        out.push_str(&format!(
            "  \"format\" : {},\n",
            json_escape_string("DiffLoupeComparisonReport")
        ));
        out.push_str(&format!(
            "  \"generatedAt\" : {},\n",
            json_escape_string(&iso8601(self.generated_at))
        ));
        for (key, path, git_ref) in [
            ("left", &r.left_root, &self.left_git_ref),
            ("right", &r.right_root, &self.right_git_ref),
        ] {
            out.push_str(&format!("  \"{}\" : {{\n", key));
            out.push_str(&format!(
                "    \"gitRef\" : {},\n",
                git_ref
                    .as_deref()
                    .map(json_escape_string)
                    .unwrap_or_else(|| "null".into())
            ));
            out.push_str(&format!("    \"path\" : {}\n", json_escape_string(path)));
            out.push_str("  },\n");
        }
        let s = &r.summary;
        out.push_str("  \"summary\" : {\n");
        out.push_str(&format!(
            "    \"differentFiles\" : {},\n    \"errorFiles\" : {},\n    \"identicalFiles\" : {},\n    \"leftOnlyFiles\" : {},\n    \"rightOnlyFiles\" : {},\n    \"totalFiles\" : {}\n",
            s.different_files, s.error_files, s.identical_files, s.left_only_files,
            s.right_only_files, s.total_files()));
        out.push_str("  },\n");
        out.push_str("  \"version\" : 1\n");
        out.push('}');
        out
    }

    // MARK: - CSV

    #[allow(clippy::vec_init_then_push)]
    fn render_csv(&self) -> String {
        let r = self.result;
        let mut lines: Vec<String> = Vec::new();
        lines.push(csv_row(&["項目", "値"]));
        lines.push(csv_row(&["レポート種別", FORMAT_NAME]));
        lines.push(csv_row(&[
            "左",
            &side_meta(&r.left_root, &self.left_git_ref),
        ]));
        lines.push(csv_row(&[
            "右",
            &side_meta(&r.right_root, &self.right_git_ref),
        ]));
        lines.push(csv_row(&["日時", &iso8601(self.generated_at)]));
        for (label, count) in self.summary_rows() {
            lines.push(csv_row(&[label, &count.to_string()]));
        }
        lines.push(csv_row(&["合計", &r.summary.total_files().to_string()]));
        lines.push(String::new());
        lines.push(csv_row(&ENTRY_HEADER));
        for e in &r.entries {
            lines.push(csv_row(
                &entry_row(e).iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ));
        }
        lines.join("\n")
    }

    // MARK: - Markdown

    fn render_markdown(&self) -> String {
        let r = self.result;
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("# {}", FORMAT_NAME));
        lines.push(String::new());
        lines.push(format!(
            "- 左: {}",
            md_escape(&side_meta(&r.left_root, &self.left_git_ref))
        ));
        lines.push(format!(
            "- 右: {}",
            md_escape(&side_meta(&r.right_root, &self.right_git_ref))
        ));
        lines.push(format!("- 日時: {}", iso8601(self.generated_at)));
        lines.push(String::new());
        lines.push("## サマリ".into());
        lines.push(String::new());
        lines.push(md_row(&["項目", "件数"]));
        lines.push(md_separator(2));
        for (label, count) in self.summary_rows() {
            lines.push(md_row(&[label, &count.to_string()]));
        }
        lines.push(md_row(&["合計", &r.summary.total_files().to_string()]));
        lines.push(String::new());
        lines.push("## 詳細".into());
        lines.push(String::new());
        lines.push(md_row(&ENTRY_HEADER));
        lines.push(md_separator(ENTRY_HEADER.len()));
        for e in &r.entries {
            let cells: Vec<String> = entry_row(e).iter().map(|s| md_escape(s)).collect();
            lines.push(md_row(
                &cells.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ));
        }
        lines.join("\n")
    }

    // MARK: - HTML

    fn render_html(&self) -> String {
        let r = self.result;
        let summary_html = self
            .summary_rows()
            .iter()
            .map(|(label, count)| {
                format!(
                    "<tr><td>{}</td><td class=\"num\">{}</td></tr>",
                    html_escape(label),
                    count
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let entries_html = r
            .entries
            .iter()
            .map(|e| {
                let cells = entry_row(e)
                    .iter()
                    .map(|c| format!("<td>{}</td>", html_escape(c)))
                    .collect::<String>();
                format!(
                    "<tr class=\"{}\">{}</tr>",
                    status_row_class(e.status),
                    cells
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let header_cells = ENTRY_HEADER
            .iter()
            .map(|h| format!("<th>{}</th>", html_escape(h)))
            .collect::<String>();
        let left_meta = html_escape(&side_meta(&r.left_root, &self.left_git_ref));
        let right_meta = html_escape(&side_meta(&r.right_root, &self.right_git_ref));
        format!(
            r##"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{0}</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 24px; color: #1a1a1a; }}
h1 {{ font-size: 1.3rem; margin: 0 0 8px; }}
h2 {{ font-size: 1.1rem; margin: 24px 0 8px; }}
.meta {{ color: #555; font-size: 0.9rem; margin: 0 0 16px; }}
table {{ border-collapse: collapse; width: 100%; font-size: 0.85rem; }}
th, td {{ border: 1px solid #ccc; padding: 4px 8px; text-align: left; vertical-align: top; }}
th {{ background: #f2f2f2; }}
td.num {{ text-align: right; white-space: nowrap; }}
tr.identical {{ color: #666; }}
tr.leftonly td:first-child, tr.rightonly td:first-child {{ background: #fff8e6; }}
tr.error td {{ color: #b00020; }}
</style>
</head>
<body>
<h1>{0}</h1>
<p class="meta">左: {1}<br>右: {2}<br>日時: {3}</p>
<h2>サマリ</h2>
<table>
<tr><th>項目</th><th>件数</th></tr>
{4}
<tr><td>合計</td><td class="num">{5}</td></tr>
</table>
<h2>詳細</h2>
<table>
<thead><tr>{6}</tr></thead>
<tbody>
{7}
</tbody>
</table>
</body>
</html>"##,
            html_escape(FORMAT_NAME),
            left_meta,
            right_meta,
            iso8601(self.generated_at),
            summary_html,
            r.summary.total_files(),
            header_cells,
            entries_html
        )
    }
}

fn status_row_class(s: DiffStatus) -> &'static str {
    match s {
        DiffStatus::Identical => "identical",
        DiffStatus::Different => "different",
        DiffStatus::LeftOnly => "leftonly",
        DiffStatus::RightOnly => "rightonly",
        DiffStatus::Error => "error",
    }
}

fn entry_json(e: &ComparedEntry, depth: usize) -> String {
    let ind = "  ".repeat(depth + 1);
    let indc = "  ".repeat(depth);
    let opt = |v: &Option<f64>| {
        v.map(iso8601_date)
            .map(|s| json_escape_string(&s))
            .unwrap_or_else(|| "null".into())
    };
    let rename = match &e.rename_candidate {
        None => "null".to_string(),
        Some(c) => format!(
            "{{\n{}\"counterpartPath\" : {},\n{}\"isUniqueMatch\" : {},\n{}\"totalMatches\" : {}\n{}}}",
            "  ".repeat(depth + 2), json_escape_string(&c.counterpart_path),
            "  ".repeat(depth + 2), c.is_unique_match,
            "  ".repeat(depth + 2), c.total_matches,
            "  ".repeat(depth + 1)),
    };
    format!(
        "{{\n{i}\"errorMessage\" : {em},\n{i}\"isDirectory\" : {id},\n{i}\"leftModificationDate\" : {lm},\n{i}\"leftSize\" : {ls},\n{i}\"relativePath\" : {rp},\n{i}\"renameCandidate\" : {rc},\n{i}\"rightModificationDate\" : {rm},\n{i}\"rightSize\" : {rs},\n{i}\"status\" : {st}\n{ic}}}",
        i = ind,
        em = e.error_message.as_deref().map(json_escape_string).unwrap_or_else(|| "null".into()),
        id = e.is_directory,
        lm = opt(&e.left_modification_date),
        ls = e.left_size,
        rp = json_escape_string(&e.relative_path),
        rc = rename,
        rm = opt(&e.right_modification_date),
        rs = e.right_size,
        st = json_escape_string(e.status.raw_value()),
        ic = indc,
    )
}

fn csv_row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_field(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') || v.contains('\r') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn md_escape(t: &str) -> String {
    t.replace('|', "\\|").replace('\r', "").replace('\n', " ")
}

fn md_row(fields: &[&str]) -> String {
    format!("| {} |", fields.join(" | "))
}

fn md_separator(n: usize) -> String {
    format!("| {} |", vec!["---"; n].join(" | "))
}

fn html_escape(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
