//! zip アーカイブの一時展開(フォルダ比較へ zip の中身を渡すための土台)
//!
//! 動作設計:
//! - zip を一時ディレクトリ(`DiffLoupe-zip-…`)へ展開し、そのパスを返す。展開後は通常の
//!   フォルダ比較として扱える。後始末は呼び出し側が `cleanup` で行う。失敗時は作りかけを削除する。
//! - 外部プロセスは使わない(純 Rust の `zip` クレート、deflate のみ有効)。
//! - 安全策: zip-slip を防ぐため、絶対パス・`..` を含むエントリは拒否する。シンボリックリンクの
//!   エントリはリンクを作らずスキップする。エントリ数と展開後の合計バイト数に上限を設ける
//!   (zip bomb 対策。申告サイズではなく実際に書き込んだバイト数で数える)。パスワード付き zip は
//!   エラーにする。
//! - ファイル名は UTF-8 として正しければそのまま使い、そうでなければ Shift_JIS として
//!   デコードする(Windows で作られた日本語 zip 対策)。`__MACOSX/` 以下と `.DS_Store` は展開しない。
//! - 展開物を git 作業ツリーへ同期しても hooks / config を上書きされないよう、パス要素が
//!   `.git` の変種(末尾のドット・空白、大文字小文字、HFS+ が無視する文字を挟んだもの、
//!   NTFS 短縮名 `git~1`)になるエントリは、その配下も含めて黙ってスキップする。
//! - Windows で扱えない名前(`:` を含む、予約名 CON / NUL / COM1 等、末尾がドットまたは空白)は、
//!   どの OS でも「展開先の外を指すエントリ」と同じくエラーにする。
//! - ファイルは `create_new` で作る。同名(大文字小文字違い・NFC/NFD 違いで同じ実体になる場合を
//!   含む)のエントリが既にあれば、後勝ちで黙って片方を隠さずエラーにする。
//! - `cleanup` は、一時ディレクトリ直下の `DiffLoupe-zip-` で始まるディレクトリだけを削除する。
//! - 更新日時の復元: 拡張タイムスタンプ(extra field 0x5455。UTC の mtime)があれば最優先、
//!   次に NTFS extra field(0x000a)の mtime(UTC)を使う。どちらも無いときだけ MS-DOS 日時を使う。
//!   MS-DOS 日時は作成側のローカル時刻で記録されるのが慣習のため、unix では実行環境の
//!   ローカル時刻として解釈する(`mktime`、夏時間は自動判定)。Windows では追加依存を避けるため
//!   UTC とみなす(ローカル時刻への変換は行わない)。

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// 展開を許すエントリ数の上限
pub const MAX_ENTRIES: usize = 200_000;
/// 展開後の合計バイト数の上限(8 GiB)
pub const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ArchiveError(pub String);
impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ArchiveError {}

fn err(message: impl Into<String>) -> ArchiveError {
    ArchiveError(message.into())
}

/// 拡張子が zip(大文字小文字無視)か
pub fn is_archive_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// zip を一時ディレクトリへ展開し、そのディレクトリを返す
pub fn materialize_archive(path: &Path) -> Result<PathBuf, ArchiveError> {
    let temp = create_unique_temp_dir("DiffLoupe-zip-")
        .map_err(|e| err(format!("一時ディレクトリの作成に失敗しました: {}", e)))?;
    match extract_all(path, &temp) {
        Ok(()) => Ok(temp),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp);
            Err(e)
        }
    }
}

/// 展開先ディレクトリの削除
pub fn cleanup(path: &Path) {
    if is_owned_temp_dir(path, "DiffLoupe-zip-") {
        let _ = std::fs::remove_dir_all(path);
    }
}

/// `<temp_dir>/<prefix>…` の直下ディレクトリ(シンボリックリンクでない)か。
/// `cleanup` が誤って無関係なディレクトリを再帰削除しないための多重防御。
pub(crate) fn is_owned_temp_dir(path: &Path, prefix: &str) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !name.starts_with(prefix) {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    // 相対パスの親は空になるため、カレントディレクトリ基準で解決する
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let (Ok(parent), Ok(temp)) = (
        std::fs::canonicalize(parent),
        std::fs::canonicalize(std::env::temp_dir()),
    ) else {
        return false;
    };
    if parent != temp {
        return false;
    }
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// HFS+ が無視する文字(これを挟んだ `.git` も `.git` として扱われる)
fn is_hfs_ignorable(c: char) -> bool {
    matches!(c,
        '\u{200C}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{206A}'..='\u{206F}'
        | '\u{FEFF}')
}

/// パス要素が `.git` の変種か(展開物を git 作業ツリーへ同期すると .git を上書きし得るもの)
pub(crate) fn is_dot_git_variant(part: &str) -> bool {
    fn is_dot_git(s: &str) -> bool {
        s.trim_end_matches(['.', ' ']).eq_ignore_ascii_case(".git")
    }
    if is_dot_git(part) || part.eq_ignore_ascii_case("git~1") {
        return true;
    }
    if part.chars().any(is_hfs_ignorable) {
        let stripped: String = part.chars().filter(|c| !is_hfs_ignorable(*c)).collect();
        return is_dot_git(&stripped) || stripped.eq_ignore_ascii_case("git~1");
    }
    false
}

/// 展開物として作らない名前の理由。Windows でだけ適用する。
/// `:`(ドライブ指定・代替データストリーム)、予約名、末尾のドット・空白は Windows でのみ
/// 展開先の外への書き込みや別ファイルへの化けにつながる。macOS / Linux では正当な名前
/// (例: `aux.c`、時刻入りの `12:30.png`)なので、拒否すると普通の zip やリポジトリを比較できなくなる。
pub(crate) fn unsafe_name_reason(part: &str) -> Option<&'static str> {
    if cfg!(windows) {
        windows_unsafe_name_reason(part)
    } else {
        None
    }
}

/// Windows で安全に作れない名前の理由(判定そのもの。OS によらずテストできるよう分離)
pub(crate) fn windows_unsafe_name_reason(part: &str) -> Option<&'static str> {
    if part.contains(':') {
        return Some("コロンを含む名前");
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return Some("末尾がドットまたは空白の名前");
    }
    let stem = part
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Some("Windows の予約名");
    }
    None
}

/// エントリ名(生バイト列)を文字列へ。UTF-8 が不正なら Shift_JIS として読む。
fn decode_name(raw: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }
    match encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(raw) {
        Some(s) => s.into_owned(),
        None => String::from_utf8_lossy(raw).into_owned(),
    }
}

/// 展開先からの相対パスへ変換する。
/// `Ok(None)` は展開しない(`__MACOSX/`、`.DS_Store`、`.git` の変種、空名)、`Err` は不正(zip-slip 等)。
fn sanitize_entry_path(name: &str) -> Result<Option<PathBuf>, ArchiveError> {
    if name.contains('\0') {
        return Err(err(format!(
            "不正なエントリ名です: {}",
            name.escape_debug()
        )));
    }
    // 古い Windows 製 zip はバックスラッシュ区切りのことがある
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(err(format!("絶対パスのエントリは展開できません: {}", name)));
    }
    let mut out = PathBuf::new();
    let mut first = true;
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(err(format!(
                "展開先の外を指すエントリは展開できません: {}",
                name
            )));
        }
        // "C:" のようなドライブ指定は絶対パス扱い
        let bytes = part.as_bytes();
        if first && bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err(err(format!("絶対パスのエントリは展開できません: {}", name)));
        }
        if first && part == "__MACOSX" {
            return Ok(None);
        }
        first = false;
        if is_dot_git_variant(part) {
            return Ok(None);
        }
        if let Some(reason) = unsafe_name_reason(part) {
            return Err(err(format!(
                "展開できない名前のエントリです({}): {}",
                reason, name
            )));
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        return Ok(None);
    }
    // 最終防衛線: 通常のコンポーネントだけで構成されていること
    if !out.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(err(format!(
            "展開先の外を指すエントリは展開できません: {}",
            name
        )));
    }
    if out.file_name().is_some_and(|n| n == ".DS_Store") {
        return Ok(None);
    }
    Ok(Some(out))
}

fn extract_all(archive_path: &Path, dest: &Path) -> Result<(), ArchiveError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| err(format!("zip を開けませんでした: {}", e)))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| err(format!("zip を読み込めませんでした: {}", e)))?;
    if zip.len() > MAX_ENTRIES {
        return Err(err(format!(
            "zip のエントリ数が多すぎます(上限 {} 件)",
            MAX_ENTRIES
        )));
    }

    // zip クレートは同名エントリを 1 つにまとめてしまう(後勝ち)ため、先に検出してエラーにする
    if zip.has_duplicate_names() {
        return Err(err(
            "zip 内でファイル名が衝突しています: 同名のエントリが複数あります",
        ));
    }

    let mut total: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    for index in 0..zip.len() {
        // 展開せずにメタデータだけ先に確認する
        let (name, is_dir, is_symlink, encrypted, mode) = {
            let entry = zip
                .by_index_raw(index)
                .map_err(|e| err(format!("zip のエントリを読み込めませんでした: {}", e)))?;
            (
                decode_name(entry.name_raw()),
                entry.is_dir(),
                entry.is_symlink(),
                entry.encrypted(),
                entry.unix_mode(),
            )
        };
        if encrypted {
            return Err(err("パスワード付き zip には対応していません"));
        }
        let Some(rel) = sanitize_entry_path(&name)? else {
            continue;
        };
        let target = dest.join(&rel);
        if is_symlink {
            continue;
        }
        if is_dir {
            std::fs::create_dir_all(&target)
                .map_err(|e| err(format!("フォルダの作成に失敗しました: {}", e)))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| err(format!("フォルダの作成に失敗しました: {}", e)))?;
        }

        let mut entry = zip.by_index(index).map_err(|e| {
            if is_password_error(&e) {
                err("パスワード付き zip には対応していません")
            } else {
                err(format!(
                    "zip のエントリを展開できませんでした({}): {}",
                    name, e
                ))
            }
        })?;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let exec = mode.is_some_and(|m| m & 0o100 != 0);
            opts.mode(if exec { 0o755 } else { 0o644 });
        }
        #[cfg(not(unix))]
        let _ = mode;
        let mut out = opts.open(&target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                err(format!("zip 内でファイル名が衝突しています: {}", name))
            } else {
                err(format!("ファイルの作成に失敗しました({}): {}", name, e))
            }
        })?;
        loop {
            let n = entry.read(&mut buf).map_err(|e| {
                err(format!(
                    "zip のエントリを展開できませんでした({}): {}",
                    name, e
                ))
            })?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > MAX_TOTAL_BYTES {
                return Err(err(format!(
                    "展開後のサイズが大きすぎます(上限 {} GiB)",
                    MAX_TOTAL_BYTES / (1024 * 1024 * 1024)
                )));
            }
            out.write_all(&buf[..n])
                .map_err(|e| err(format!("ファイルの書き込みに失敗しました({}): {}", name, e)))?;
        }
        if let Some(time) = entry_modified_time(entry.extra_data_fields(), entry.last_modified()) {
            let _ = out.set_modified(time);
        }
    }
    Ok(())
}

fn is_password_error(e: &zip::result::ZipError) -> bool {
    let s = e.to_string().to_ascii_lowercase();
    s.contains("password") || s.contains("encrypt")
}

/// 更新日時を決める。拡張タイムスタンプ(UTC)、NTFS(UTC)、MS-DOS 日時(ローカル時刻)の順。
fn entry_modified_time<'a>(
    extra_fields: impl Iterator<Item = &'a zip::extra_fields::ExtraField>,
    dos: Option<zip::DateTime>,
) -> Option<std::time::SystemTime> {
    use zip::extra_fields::ExtraField;
    let mut ntfs: Option<std::time::SystemTime> = None;
    for field in extra_fields {
        match field {
            ExtraField::ExtendedTimestamp(ts) => {
                if let Some(secs) = ts.mod_time() {
                    return Some(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
                    );
                }
            }
            ExtraField::Ntfs(n) => {
                // FILETIME: 1601-01-01 UTC からの 100ns 単位
                const EPOCH_DIFF_SECS: u64 = 11_644_473_600;
                let ticks = n.mtime();
                if let Some(unix_secs) = (ticks / 10_000_000).checked_sub(EPOCH_DIFF_SECS) {
                    ntfs = Some(
                        std::time::UNIX_EPOCH
                            + std::time::Duration::new(
                                unix_secs,
                                ((ticks % 10_000_000) * 100) as u32,
                            ),
                    );
                }
            }
            _ => {}
        }
    }
    ntfs.or_else(|| dos.and_then(dos_to_system_time))
}

/// MS-DOS 日時を SystemTime へ。unix ではローカル時刻として解釈する。
#[cfg(unix)]
fn dos_to_system_time(dt: zip::DateTime) -> Option<std::time::SystemTime> {
    if !dt.is_valid() {
        return None;
    }
    // SAFETY: tm は全フィールドを 0 で初期化してから値を設定し、mktime だけに渡す。
    let secs = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = dt.year() as libc::c_int - 1900;
        tm.tm_mon = dt.month() as libc::c_int - 1;
        tm.tm_mday = dt.day() as libc::c_int;
        tm.tm_hour = dt.hour() as libc::c_int;
        tm.tm_min = dt.minute() as libc::c_int;
        tm.tm_sec = dt.second() as libc::c_int;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm)
    };
    if secs < 0 {
        return None;
    }
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
}

/// MS-DOS 日時を UTC とみなした SystemTime へ(Windows。ローカル時刻変換の依存は増やさない)
#[cfg(not(unix))]
fn dos_to_system_time(dt: zip::DateTime) -> Option<std::time::SystemTime> {
    if !dt.is_valid() {
        return None;
    }
    let (y, m, d) = (dt.year() as i64, dt.month() as i64, dt.day() as i64);
    // Howard Hinnant の days_from_civil
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs =
        days * 86_400 + dt.hour() as i64 * 3600 + dt.minute() as i64 * 60 + dt.second() as i64;
    if secs < 0 {
        return None;
    }
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
}

/// 一意な一時ディレクトリを作る(`<temp>/<prefix><時刻><pid>-<連番>`)。
/// 時刻だけでは同時実行で名前が衝突し得るため、プロセス内の連番を足し、
/// `create_dir`(既存なら失敗)で作れた場合だけ採用する。
pub(crate) fn create_unique_temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base)?;
    let mut last_error = None;
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!(
            "{}{:x}{:x}-{:x}",
            prefix,
            nanos,
            std::process::id(),
            seq
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last_error = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("一時ディレクトリ名が衝突しました")))
}
