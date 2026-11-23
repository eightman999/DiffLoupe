//! 表計算テキスト抽出(document.rs)のテスト。最小の xlsx をテスト内で組み立てる。

use diffloupe_core::document;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "DiffLoupeTests-doc-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_xlsx(path: &Path, sheets: &[(&str, &str)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, body: &str| {
        w.start_file(name, opts).unwrap();
        w.write_all(body.as_bytes()).unwrap();
    };
    let mut overrides = String::new();
    let mut wb_sheets = String::new();
    let mut rels = String::new();
    for (i, (name, _)) in sheets.iter().enumerate() {
        let n = i + 1;
        overrides.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
        wb_sheets.push_str(&format!(
            r#"<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>"#
        ));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#
        ));
    }
    let styles_id = sheets.len() + 1;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{styles_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    ));
    put(
        "[Content_Types].xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#
        ),
    );
    put(
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
    );
    put(
        "xl/workbook.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{wb_sheets}</sheets></workbook>"#
        ),
    );
    put(
        "xl/_rels/workbook.xml.rels",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
        ),
    );
    // cellXfs: 0 = 標準, 1 = 日付(numFmtId 14), 2 = 日時(numFmtId 22)
    put(
        "xl/styles.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="22" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs></styleSheet>"#,
    );
    for (i, (_, body)) in sheets.iter().enumerate() {
        put(
            &format!("xl/worksheets/sheet{}.xml", i + 1),
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{body}</sheetData></worksheet>"#
            ),
        );
    }
    w.finish().unwrap();
}

fn text_cell(r: &str, s: &str) -> String {
    format!(r#"<c r="{r}" t="inlineStr"><is><t xml:space="preserve">{s}</t></is></c>"#)
}

#[test]
fn is_spreadsheet_path_checks_extension() {
    for p in ["a.xlsx", "a.XLSM", "a.xls", "b.ODS", "/x/y.Xlsx"] {
        assert!(document::is_spreadsheet_path(Path::new(p)), "{}", p);
    }
    for p in ["a.csv", "a.docx", "xlsx", "a.xlsx.bak"] {
        assert!(!document::is_spreadsheet_path(Path::new(p)), "{}", p);
    }
}

#[test]
fn extracts_all_sheets_with_tab_separated_rows() {
    let dir = scratch();
    let _c = Cleanup(dir.clone());
    let path = dir.join("book.xlsx");
    let sheet1 = format!(
        "<row r=\"1\">{}{}{}</row><row r=\"2\">{}<c r=\"B2\"><v>42</v></c><c r=\"C2\"><v>3.5</v></c><c r=\"D2\" t=\"b\"><v>1</v></c></row><row r=\"3\"><c r=\"A3\"><v>2</v></c><c r=\"B3\"><f>A3+1</f><v>3</v></c></row><row r=\"4\">{}</row>",
        text_cell("A1", "名前"),
        text_cell("B1", "メモ"),
        text_cell("C1", ""),
        text_cell("A2", "山田"),
        text_cell("A4", ""),
    );
    let sheet2 = format!(
        "<row r=\"1\">{}</row><row r=\"2\">{}</row>",
        text_cell("A1", "line1\nline2\ttab"),
        text_cell("A2", "end"),
    );
    write_xlsx(&path, &[("シート1", &sheet1), ("Second", &sheet2)]);
    let text = document::extract_spreadsheet_text(&path).unwrap();
    let expected =
        "## シート1\n名前\tメモ\n山田\t42\t3.5\tTRUE\n2\t3\n\n## Second\nline1 line2 tab\nend\n";
    assert_eq!(text, expected);
}

#[test]
fn formats_dates_as_iso() {
    let dir = scratch();
    let _c = Cleanup(dir.clone());
    let path = dir.join("dates.xlsx");
    // 44259 = 2021-03-04、44259.25 = 2021-03-04 06:00:00
    let sheet =
        r#"<row r="1"><c r="A1" s="1"><v>44259</v></c><c r="B1" s="2"><v>44259.25</v></c></row>"#;
    write_xlsx(&path, &[("D", sheet)]);
    let text = document::extract_spreadsheet_text(&path).unwrap();
    assert_eq!(text, "## D\n2021-03-04\t2021-03-04T06:00:00\n");
}

#[test]
fn trailing_empty_rows_are_trimmed() {
    let dir = scratch();
    let _c = Cleanup(dir.clone());
    let path = dir.join("trim.xlsx");
    let sheet = format!(
        "<row r=\"1\">{}</row><row r=\"2\"><c r=\"A2\"><v>1</v></c></row><row r=\"3\"/><row r=\"4\">{}</row>",
        text_cell("A1", "head"),
        text_cell("A4", "   ")
    );
    write_xlsx(&path, &[("S", &sheet)]);
    let text = document::extract_spreadsheet_text(&path).unwrap();
    // 空白だけのセルは空ではないので残る
    assert_eq!(text, "## S\nhead\n1\n\n   \n");
}

#[test]
fn rejects_oversized_file() {
    let dir = scratch();
    let _c = Cleanup(dir.clone());
    let path = dir.join("huge.xlsx");
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(document::MAX_FILE_BYTES + 1).unwrap();
    let e = document::extract_spreadsheet_text(&path).unwrap_err();
    assert!(e.0.contains("ファイルが大きすぎます"), "{}", e.0);
}

#[test]
fn reports_error_for_non_spreadsheet_and_missing_file() {
    let dir = scratch();
    let _c = Cleanup(dir.clone());
    let path = dir.join("broken.xlsx");
    std::fs::write(&path, b"not a spreadsheet").unwrap();
    assert!(document::extract_spreadsheet_text(&path).is_err());
    assert!(document::extract_spreadsheet_text(&dir.join("none.xlsx")).is_err());
}
