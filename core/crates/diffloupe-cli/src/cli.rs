//! diffloupe CLI(Swift `CLILib` 相当): 引数解析・実行制御・出力・終了コード

use diffloupe_core::cancel::CancellationToken;
use diffloupe_core::comparator::{CompareError, CompareOptions, FolderComparator};
use diffloupe_core::filter::filter_preset;
use diffloupe_core::models::{ComparisonProgress, ComparisonResult, DiffStatus};
use diffloupe_core::options::ContentComparison;
use diffloupe_core::paths::path_extension;
use diffloupe_core::report::{ComparisonReport, ReportFormat};
use diffloupe_core::structured::StructuredComparisonMode;
use std::io::Write;
use std::path::Path;

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_DIFFERENCES: i32 = 1;
pub const EXIT_USAGE_ERROR: i32 = 2;
pub const EXIT_FAILURE: i32 = 3;

pub fn winmerge_code(code: i32) -> i32 {
    match code {
        EXIT_SUCCESS => 0,
        EXIT_DIFFERENCES => 1,
        _ => 2,
    }
}

pub fn exit_for_summary(s: &diffloupe_core::models::ComparisonSummary) -> i32 {
    if s.different_files == 0
        && s.left_only_files == 0
        && s.right_only_files == 0
        && s.error_files == 0
    {
        EXIT_SUCCESS
    } else {
        EXIT_DIFFERENCES
    }
}

pub const VERSION_TEXT: &str = "diffloupe 1.1.0 (DiffLoupe CLI)";

pub const USAGE: &str = "使い方: diffloupe [オプション] <左フォルダ> <右フォルダ>

左右のフォルダを比較し、結果を標準出力またはファイルへ出力します。
既定は非再帰比較です(WinMergeで /r を指定しない場合と同じ)。
WinMergeのCLI/CI運用に相当する終了コードを返します:
  0 = 全ファイル一致
  1 = 差異・片側のみ・読み込みエラーあり
  2 = 引数・パス・オプションのエラー
  3 = 比較処理エラーまたはキャンセル

オプション:
  -r, --recursive                   再帰比較を有効化(既定は非再帰)
      --no-recursive                再帰比較を明示的に無効化
  -x, --no-default-excludes         既定除外(.git/.DS_Store/node_modules)を無効化
  -e, --exclude <glob>              除外glob(複数指定可。区切りなし=ファイル名、あり=相対パス)
  -i, --include <glob>              インクルードglob(複数指定可。非空時は一致するファイルのみ対象)
      --preset <name>               組み込みプリセットの除外を追加
                                    (source-control / build-artifacts / web-nodejs / macos-metadata。opt-in)
      --ignore-line-endings         改行コード(CRLF/CR/LF)を無視
      --ignore-trailing-whitespace  行末の空白を無視
      --normalize-whitespace        連続空白を単一スペースに正規化
      --ignore-case                 大文字小文字を無視
      --structured <off|auto|json|xml|csv>
                                    JSON/XML/CSV・TSVの構造化比較
                                    (キー順・属性順・引用・区切りなどの表記揺れを正規化。既定 off)
      --normalize-json              --structured json のエイリアス
      --normalize-xml               --structured xml のエイリアス
      --table-csv                   --structured csv のエイリアス
      --detect-renames              片側のみファイルのリネーム/移動候補を検出
  -f, --format <json|csv|md|html>   レポート形式(-o省略時はstdoutへ出力)
  -o, --output <パス>               レポートをファイルへ書き込む(既存ファイルの上書きは拒否)
  -q, --quiet                       進捗(stderr)を出力しない
  -h, --help                        このヘルプを表示
  -V, --version                     バージョンを表示

WinMerge互換スラッシュスイッチ(絶対POSIXパスと混同しないよう、既知のスイッチ名にのみ反応):
  /r /r-                          再帰比較の有効化(/r)/無効化(/r-)
  /u /ul /um /ur                  MRU履歴の抑制(GUI専用のため受理のみ・無操作)
  /wl /wm /wr                     ペインの読み取り専用(GUI専用のため受理のみ・無操作)
  /minimize /maximize /noninteractive  GUI用スイッチ(受理のみ・無操作)
  /x /xq                          比較後に終了(GUI用。CLIでは -q 相当へマップ)
  /enableexitcode                 WinMerge互換の終了コード(0=一致/1=差異/2=エラー)へ写像
  /dl <ラベル> /dm <ラベル> /dr <ラベル>  表示ラベル(GUI専用のため値は無視)
  /or <パス>                      レポートをファイルへ出力(拡張子不明時はHTML)

例:
  diffloupe left/ right/
  diffloupe -r left/ right/
  diffloupe --format json left/ right/ > report.json
  diffloupe /r /or report.html left/ right/";

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownOption(String),
    MissingValue(String),
    MissingPaths,
    ExtraArguments(Vec<String>),
    InvalidFormat(String),
    InvalidStructuredMode(String),
    OutputExtensionUnknown(String),
    UnknownPreset(String),
}

fn error_message(e: &ParseError) -> String {
    match e {
        ParseError::UnknownOption(o) => format!("未知のオプションです: {}", o),
        ParseError::MissingValue(o) => format!("オプションに値が必要です: {}", o),
        ParseError::MissingPaths => "左右のフォルダパスを2つ指定してください。".into(),
        ParseError::ExtraArguments(x) => format!("想定外の引数です: {}", x.join(" ")),
        ParseError::InvalidFormat(f) => format!("不明なレポート形式です: {} (json / csv / md / html)", f),
        ParseError::InvalidStructuredMode(m) => format!("不明な構造化比較モードです: {} (off / auto / json / xml / csv)", m),
        ParseError::OutputExtensionUnknown(p) => format!("出力ファイルの拡張子から形式を判別できません: {} (--formatを指定してください)", p),
        ParseError::UnknownPreset(p) => format!("不明なプリセットです: {} (source-control / build-artifacts / web-nodejs / macos-metadata)", p),
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct CliOptions {
    pub left_path: Option<String>,
    pub right_path: Option<String>,
    pub recursive: bool,
    pub use_default_excludes: bool,
    pub user_exclude_patterns: Vec<String>,
    pub user_include_patterns: Vec<String>,
    pub filter_presets: Vec<String>,
    pub content_comparison: ContentComparison,
    pub structured_mode: StructuredComparisonMode,
    pub detect_renames: bool,
    pub report_format: Option<ReportFormat>,
    pub output_path: Option<String>,
    pub enable_exit_code: bool,
    pub output_from_winmerge: bool,
    pub quiet: bool,
    pub show_help: bool,
    pub show_version: bool,
}

impl CliOptions {
    fn new() -> Self {
        Self {
            use_default_excludes: true,
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub enum ParseResult {
    Options(CliOptions),
    Help,
    Version,
    Error(ParseError),
}

#[derive(Debug, PartialEq)]
pub enum ReportTarget {
    None,
    Stdout(ReportFormat),
    File(String, ReportFormat),
}

pub fn parse_format(raw: &str) -> Option<ReportFormat> {
    match raw.to_lowercase().as_str() {
        "json" => Some(ReportFormat::Json),
        "csv" => Some(ReportFormat::Csv),
        "md" | "markdown" => Some(ReportFormat::Markdown),
        "html" => Some(ReportFormat::Html),
        _ => None,
    }
}

pub fn parse_structured_mode(raw: &str) -> Option<StructuredComparisonMode> {
    match raw.to_lowercase().as_str() {
        "off" | "disabled" => Some(StructuredComparisonMode::Disabled),
        "auto" | "automatic" => Some(StructuredComparisonMode::Automatic),
        "json" => Some(StructuredComparisonMode::Json),
        "xml" => Some(StructuredComparisonMode::Xml),
        "csv" => Some(StructuredComparisonMode::Csv),
        _ => None,
    }
}

pub fn format_for_output_path(path: &str) -> Option<ReportFormat> {
    match path_extension(path).to_lowercase().as_str() {
        "json" => Some(ReportFormat::Json),
        "csv" => Some(ReportFormat::Csv),
        "md" | "markdown" => Some(ReportFormat::Markdown),
        "html" => Some(ReportFormat::Html),
        _ => None,
    }
}

pub fn report_target(options: &CliOptions) -> Result<ReportTarget, ParseError> {
    match (&options.report_format, &options.output_path) {
        (None, None) => Ok(ReportTarget::None),
        (None, Some(out)) => {
            if let Some(f) = format_for_output_path(out) {
                Ok(ReportTarget::File(out.clone(), f))
            } else if options.output_from_winmerge {
                Ok(ReportTarget::File(out.clone(), ReportFormat::Html))
            } else {
                Err(ParseError::OutputExtensionUnknown(out.clone()))
            }
        }
        (Some(f), None) => Ok(ReportTarget::Stdout(*f)),
        (Some(f), Some(out)) => Ok(ReportTarget::File(out.clone(), *f)),
    }
}

fn is_slash_switch(arg: &str) -> bool {
    matches!(
        arg.to_lowercase().as_str(),
        "/r" | "/r-"
            | "/u"
            | "/ul"
            | "/um"
            | "/ur"
            | "/wl"
            | "/wm"
            | "/wr"
            | "/minimize"
            | "/maximize"
            | "/noninteractive"
            | "/x"
            | "/xq"
            | "/enableexitcode"
            | "/dl"
            | "/dm"
            | "/dr"
            | "/or"
    )
}

pub fn parse(args: &[String]) -> ParseResult {
    let mut options = CliOptions::new();
    let mut positional: Vec<String> = Vec::new();
    let mut options_ended = false;
    let mut i = 0usize;
    let next = |i: &mut usize| -> Option<String> {
        let v = args.get(*i).cloned();
        *i += 1;
        v
    };
    while i < args.len() {
        let arg = args[i].clone();
        i += 1;
        if options_ended {
            positional.push(arg);
            continue;
        }
        if arg == "--" {
            options_ended = true;
            continue;
        }
        if let Some(body) = arg.strip_prefix("--") {
            let mut parts = body.splitn(2, '=');
            let name = parts.next().unwrap();
            let inline = parts.next().map(|s| s.to_string());
            macro_rules! value {
                () => {
                    match inline.clone().or_else(|| next(&mut i)) {
                        Some(v) => v,
                        None => {
                            return ParseResult::Error(ParseError::MissingValue(format!(
                                "--{}",
                                name
                            )))
                        }
                    }
                };
            }
            match name {
                "help" => options.show_help = true,
                "version" => options.show_version = true,
                "recursive" => options.recursive = true,
                "no-recursive" => options.recursive = false,
                "no-default-excludes" => options.use_default_excludes = false,
                "exclude" => options.user_exclude_patterns.push(value!()),
                "include" => options.user_include_patterns.push(value!()),
                "preset" => {
                    let v = value!();
                    if filter_preset(&v).is_none() {
                        return ParseResult::Error(ParseError::UnknownPreset(v));
                    }
                    options.filter_presets.push(v);
                }
                "ignore-line-endings" => options.content_comparison.ignore_line_endings = true,
                "ignore-trailing-whitespace" => {
                    options.content_comparison.ignore_trailing_whitespace = true
                }
                "normalize-whitespace" => options.content_comparison.normalize_whitespace = true,
                "ignore-case" => options.content_comparison.ignore_case = true,
                "structured" => {
                    let v = value!();
                    match parse_structured_mode(&v) {
                        Some(m) => options.structured_mode = m,
                        None => return ParseResult::Error(ParseError::InvalidStructuredMode(v)),
                    }
                }
                "normalize-json" => options.structured_mode = StructuredComparisonMode::Json,
                "normalize-xml" => options.structured_mode = StructuredComparisonMode::Xml,
                "table-csv" => options.structured_mode = StructuredComparisonMode::Csv,
                "detect-renames" => options.detect_renames = true,
                "format" => {
                    let v = value!();
                    match parse_format(&v) {
                        Some(f) => options.report_format = Some(f),
                        None => return ParseResult::Error(ParseError::InvalidFormat(v)),
                    }
                }
                "output" => options.output_path = Some(value!()),
                "quiet" => options.quiet = true,
                _ => return ParseResult::Error(ParseError::UnknownOption(format!("--{}", name))),
            }
        } else if arg.starts_with('-') && arg != "-" {
            let flag = &arg[1..];
            if flag.chars().count() != 1 {
                return ParseResult::Error(ParseError::UnknownOption(arg.clone()));
            }
            macro_rules! svalue {
                () => {
                    match next(&mut i) {
                        Some(v) => v,
                        None => return ParseResult::Error(ParseError::MissingValue(arg.clone())),
                    }
                };
            }
            match flag {
                "h" => options.show_help = true,
                "V" => options.show_version = true,
                "r" => options.recursive = true,
                "x" => options.use_default_excludes = false,
                "e" => options.user_exclude_patterns.push(svalue!()),
                "i" => options.user_include_patterns.push(svalue!()),
                "f" => {
                    let v = svalue!();
                    match parse_format(&v) {
                        Some(f) => options.report_format = Some(f),
                        None => return ParseResult::Error(ParseError::InvalidFormat(v)),
                    }
                }
                "o" => options.output_path = Some(svalue!()),
                "q" => options.quiet = true,
                _ => return ParseResult::Error(ParseError::UnknownOption(arg.clone())),
            }
        } else if arg.starts_with('/') && is_slash_switch(&arg) {
            match arg.to_lowercase().as_str() {
                "/r" => options.recursive = true,
                "/r-" => options.recursive = false,
                "/u" | "/ul" | "/um" | "/ur" | "/wl" | "/wm" | "/wr" | "/minimize"
                | "/maximize" | "/noninteractive" => {}
                "/x" | "/xq" => options.quiet = true,
                "/enableexitcode" => options.enable_exit_code = true,
                "/dl" | "/dm" | "/dr" => {
                    if next(&mut i).is_none() {
                        return ParseResult::Error(ParseError::MissingValue(arg.clone()));
                    }
                }
                "/or" => match next(&mut i) {
                    Some(v) => {
                        options.output_path = Some(v);
                        options.output_from_winmerge = true;
                    }
                    None => return ParseResult::Error(ParseError::MissingValue(arg.clone())),
                },
                _ => return ParseResult::Error(ParseError::UnknownOption(arg.clone())),
            }
        } else {
            positional.push(arg);
        }
    }
    if options.show_help {
        return ParseResult::Help;
    }
    if options.show_version {
        return ParseResult::Version;
    }
    if positional.len() < 2 {
        return ParseResult::Error(ParseError::MissingPaths);
    }
    if positional.len() > 2 {
        return ParseResult::Error(ParseError::ExtraArguments(positional.split_off(2)));
    }
    options.left_path = Some(positional.remove(0));
    options.right_path = Some(positional.remove(0));
    ParseResult::Options(options)
}

fn out(text: &str) {
    let _ = writeln!(std::io::stdout(), "{}", text);
}
fn err(text: &str) {
    let _ = writeln!(std::io::stderr(), "{}", text);
}

fn progress_print(p: ComparisonProgress) {
    match p {
        ComparisonProgress::Scanning { found } => err(&format!("走査中: {} エントリ", found)),
        ComparisonProgress::Comparing { done, total } => {
            err(&format!("内容比較: {}/{}", done, total))
        }
        ComparisonProgress::Renames { done, total } => {
            err(&format!("リネーム候補検出: {}/{}", done, total))
        }
    }
}

pub fn comparator_options(options: &CliOptions) -> CompareOptions {
    let mut exclude = options.user_exclude_patterns.clone();
    for id in &options.filter_presets {
        if let Some(p) = filter_preset(id) {
            exclude.extend(p.exclude_patterns.iter().map(|s| s.to_string()));
        }
    }
    let mut cc = options.content_comparison;
    cc.structured_mode = options.structured_mode;
    CompareOptions {
        recursive: options.recursive,
        use_default_excludes: options.use_default_excludes,
        max_entries: None,
        user_exclude_patterns: exclude,
        user_include_patterns: options.user_include_patterns.clone(),
        content_comparison: cc,
        detect_renames: options.detect_renames,
        ..Default::default()
    }
}

pub fn validate_output(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let parent = p.parent().map(|x| {
        if x.as_os_str().is_empty() {
            Path::new(".")
        } else {
            x
        }
    });
    let parent_ok = parent.map(|x| x.is_dir()).unwrap_or(false);
    if !parent_ok {
        return Err(format!("出力先の親ディレクトリが存在しません: {}", path));
    }
    if p.exists() {
        return Err(format!(
            "出力先が既に存在します(上書きは拒否されます): {}",
            path
        ));
    }
    Ok(())
}

pub fn write_report_file(text: &str, path: &str) -> std::io::Result<()> {
    validate_output(path).map_err(std::io::Error::other)?;
    // atomically 相当: 同じディレクトリの一時ファイルへ書いて rename
    let tmp = format!("{}.diffloupe-tmp", path);
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn human_summary(result: &ComparisonResult) -> String {
    let s = &result.summary;
    let mut lines = Vec::new();
    lines.push("DiffLoupe比較結果".to_string());
    lines.push(format!("左: {}", result.left_root));
    lines.push(format!("右: {}", result.right_root));
    lines.push(format!(
        "一致: {}  相違: {}  左のみ: {}  右のみ: {}  エラー: {}  合計: {}",
        s.identical_files,
        s.different_files,
        s.left_only_files,
        s.right_only_files,
        s.error_files,
        s.total_files()
    ));
    let differing: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.status != DiffStatus::Identical && !e.is_directory)
        .collect();
    if !differing.is_empty() {
        lines.push(String::new());
        for (status, label) in [
            (DiffStatus::Different, "相違"),
            (DiffStatus::LeftOnly, "左のみ"),
            (DiffStatus::RightOnly, "右のみ"),
            (DiffStatus::Error, "エラー"),
        ] {
            let group: Vec<_> = differing.iter().filter(|e| e.status == status).collect();
            if group.is_empty() {
                continue;
            }
            lines.push(format!("{}:", label));
            for e in group {
                if status == DiffStatus::Error {
                    if let Some(m) = &e.error_message {
                        lines.push(format!("  {}  [{}]", e.relative_path, m));
                        continue;
                    }
                }
                lines.push(format!("  {}", e.relative_path));
            }
        }
    }
    lines.join("\n")
}

fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}

pub fn run(arguments: &[String], cancellation: &CancellationToken) -> i32 {
    match parse(arguments) {
        ParseResult::Help => {
            out(USAGE);
            EXIT_SUCCESS
        }
        ParseResult::Version => {
            out(VERSION_TEXT);
            EXIT_SUCCESS
        }
        ParseResult::Error(e) => {
            err(&format!("diffloupe: {}", error_message(&e)));
            err("`diffloupe --help` で使い方を確認してください。");
            EXIT_USAGE_ERROR
        }
        ParseResult::Options(options) => {
            let code = run_comparison(&options, cancellation);
            if options.enable_exit_code {
                winmerge_code(code)
            } else {
                code
            }
        }
    }
}

fn run_comparison(options: &CliOptions, cancellation: &CancellationToken) -> i32 {
    let (Some(left), Some(right)) = (&options.left_path, &options.right_path) else {
        err("diffloupe: 左右のフォルダパスが指定されていません。");
        return EXIT_USAGE_ERROR;
    };
    if !is_directory(left) {
        err(&format!(
            "diffloupe: 左パスはディレクトリではありません: {}",
            left
        ));
        return EXIT_USAGE_ERROR;
    }
    if !is_directory(right) {
        err(&format!(
            "diffloupe: 右パスはディレクトリではありません: {}",
            right
        ));
        return EXIT_USAGE_ERROR;
    }
    let target = match report_target(options) {
        Ok(t) => t,
        Err(e) => {
            err(&format!("diffloupe: {}", error_message(&e)));
            return EXIT_USAGE_ERROR;
        }
    };
    if let ReportTarget::File(path, _) = &target {
        if let Err(msg) = validate_output(path) {
            err(&format!("diffloupe: {}", msg));
            return EXIT_USAGE_ERROR;
        }
    }

    let copts = comparator_options(options);
    let quiet = options.quiet;
    let progress = move |p: ComparisonProgress| {
        if !quiet {
            progress_print(p);
        }
    };
    let result = FolderComparator::compare(
        Path::new(left),
        Path::new(right),
        &copts,
        Some(&progress),
        Some(cancellation),
    );
    let result = match result {
        Ok(r) => r,
        Err(CompareError::Cancelled) => {
            err("diffloupe: 比較がキャンセルされました。");
            return EXIT_FAILURE;
        }
        Err(CompareError::EntryLimitExceeded(e)) => {
            err(&format!("diffloupe: {}", e));
            return EXIT_FAILURE;
        }
        Err(CompareError::Io(e)) => {
            err(&format!("diffloupe: 比較に失敗しました: {}", e));
            return EXIT_FAILURE;
        }
    };
    emit(&result, &target)
}

fn emit(result: &ComparisonResult, target: &ReportTarget) -> i32 {
    let exit_code = exit_for_summary(&result.summary);
    let summary = human_summary(result);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    match target {
        ReportTarget::None => out(&summary),
        ReportTarget::Stdout(format) => {
            err(&summary);
            let report = ComparisonReport::new(result, now);
            out(&report.render(*format));
        }
        ReportTarget::File(path, format) => {
            let report = ComparisonReport::new(result, now);
            match write_report_file(&report.render(*format), path) {
                Ok(()) => {
                    out(&summary);
                    out(&format!("レポートを書き込みました: {}", path));
                }
                Err(e) => {
                    err(&format!(
                        "diffloupe: レポートの書き込みに失敗しました: {}",
                        e
                    ));
                    return EXIT_FAILURE;
                }
            }
        }
    }
    exit_code
}
