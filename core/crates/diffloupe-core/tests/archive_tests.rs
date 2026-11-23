//! zip 一時展開(archive.rs)のテスト。
//! 不正な名前や属性を持つ zip は zip クレートのライターでは作れないため、生バイトで組み立てる。

use diffloupe_core::archive::{self, ArchiveError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "DiffLoupeTests-{}-{}-{}",
        label,
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

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

struct RawEntry {
    name: Vec<u8>,
    data: Vec<u8>,
    /// 上位 16 ビットが Unix モード
    external_attr: u32,
    flags: u16,
    /// MS-DOS (date, time)
    dos: (u16, u16),
    /// extra field(ローカル・セントラル共通)
    extra: Vec<u8>,
}

impl RawEntry {
    fn file(name: &[u8], data: &[u8]) -> Self {
        RawEntry {
            name: name.to_vec(),
            data: data.to_vec(),
            external_attr: 0o100644 << 16,
            flags: 0,
            // 2021-03-04 05:06:08
            dos: (
                ((2021 - 1980) << 9) | (3 << 5) | 4,
                (5 << 11) | (6 << 5) | 4,
            ),
            extra: Vec::new(),
        }
    }
}

/// 無圧縮(stored)の zip を組み立てる
fn build_zip(entries: &[RawEntry]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for e in entries {
        let offset = out.len() as u32;
        let crc = crc32(&e.data);
        let mut local = Vec::new();
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&e.flags.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&e.dos.1.to_le_bytes());
        local.extend_from_slice(&e.dos.0.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        local.extend_from_slice(&(e.extra.len() as u16).to_le_bytes());
        local.extend_from_slice(&e.name);
        local.extend_from_slice(&e.extra);
        local.extend_from_slice(&e.data);
        out.extend_from_slice(&local);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&((3u16 << 8) | 20).to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&e.flags.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&e.dos.1.to_le_bytes());
        central.extend_from_slice(&e.dos.0.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        central.extend_from_slice(&(e.extra.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&e.external_attr.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(&e.name);
        central.extend_from_slice(&e.extra);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn write_zip(dir: &Path, name: &str, entries: &[RawEntry]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, build_zip(entries)).unwrap();
    path
}

fn extract(entries: &[RawEntry]) -> (Result<PathBuf, ArchiveError>, Cleanup) {
    let dir = scratch("zip");
    let zip = write_zip(&dir, "test.zip", entries);
    (archive::materialize_archive(&zip), Cleanup(dir))
}

fn temp_zip_dirs() -> usize {
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("DiffLoupe-zip-")
        })
        .count()
}

#[test]
fn is_archive_path_matches_zip_case_insensitively() {
    assert!(archive::is_archive_path(Path::new("a.zip")));
    assert!(archive::is_archive_path(Path::new("/x/y/A.ZIP")));
    assert!(archive::is_archive_path(Path::new("b.Zip")));
    assert!(!archive::is_archive_path(Path::new("a.zip.txt")));
    assert!(!archive::is_archive_path(Path::new("zip")));
    assert!(!archive::is_archive_path(Path::new("a.7z")));
}

#[test]
fn materialize_extracts_files_and_directories() {
    let mut dir_entry = RawEntry::file(b"docs/", b"");
    dir_entry.external_attr = 0o040755 << 16;
    let (result, _c) = extract(&[
        RawEntry::file(b"hello.txt", b"hello\n"),
        dir_entry,
        RawEntry::file(b"docs/nested/a.txt", b"nested\n"),
        RawEntry::file("日本語/ファイル.txt".as_bytes(), b"utf8\n"),
    ]);
    let out = result.unwrap();
    assert!(out
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("DiffLoupe-zip-"));
    assert!(out.starts_with(std::env::temp_dir()));
    assert_eq!(
        std::fs::read_to_string(out.join("hello.txt")).unwrap(),
        "hello\n"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("docs/nested/a.txt")).unwrap(),
        "nested\n"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("日本語/ファイル.txt")).unwrap(),
        "utf8\n"
    );
    archive::cleanup(&out);
    assert!(!out.exists());
}

fn mtime_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// 拡張タイムスタンプ(0x5455)の extra field。mtime のみ(UTC の epoch 秒)。
fn extended_timestamp(mtime: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&0x5455u16.to_le_bytes());
    v.extend_from_slice(&5u16.to_le_bytes());
    v.push(1);
    v.extend_from_slice(&mtime.to_le_bytes());
    v
}

#[test]
fn materialize_prefers_extended_timestamp_in_utc() {
    // DOS 日時(2021-03-04 05:06:08)とは無関係な UTC 時刻が優先される
    let mut e = RawEntry::file(b"t.txt", b"x");
    e.extra = extended_timestamp(1_700_000_000);
    let (result, _c) = extract(&[e]);
    let out = result.unwrap();
    assert_eq!(mtime_secs(&out.join("t.txt")), 1_700_000_000);
    archive::cleanup(&out);
}

#[test]
fn materialize_uses_ntfs_mtime_when_no_extended_timestamp() {
    // FILETIME: (unix 秒 + 11644473600) * 10^7
    let ticks: u64 = (1_650_000_000u64 + 11_644_473_600) * 10_000_000;
    let mut extra = Vec::new();
    extra.extend_from_slice(&0x000au16.to_le_bytes());
    extra.extend_from_slice(&32u16.to_le_bytes());
    extra.extend_from_slice(&0u32.to_le_bytes());
    extra.extend_from_slice(&1u16.to_le_bytes());
    extra.extend_from_slice(&24u16.to_le_bytes());
    extra.extend_from_slice(&ticks.to_le_bytes());
    extra.extend_from_slice(&ticks.to_le_bytes());
    extra.extend_from_slice(&ticks.to_le_bytes());
    let mut e = RawEntry::file(b"n.txt", b"x");
    e.extra = extra;
    let (result, _c) = extract(&[e]);
    let out = result.unwrap();
    assert_eq!(mtime_secs(&out.join("n.txt")), 1_650_000_000);
    archive::cleanup(&out);
}

/// UT が無ければ MS-DOS 日時をローカル時刻として解釈する(unix)。
/// TZ に依存しないよう、復元結果を localtime_r でローカル時刻に直して DOS 日時と照合する。
#[cfg(unix)]
#[test]
fn materialize_interprets_dos_time_as_local_time() {
    let (result, _c) = extract(&[RawEntry::file(b"t.txt", b"x")]);
    let out = result.unwrap();
    let secs = mtime_secs(&out.join("t.txt")) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    // 2021-03-04 05:06:08(DOS 日時は 2 秒単位。入力が偶数秒なので一致する)
    assert_eq!(
        (
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        ),
        (2021, 3, 4, 5, 6, 8)
    );
    archive::cleanup(&out);
}

/// Windows では ローカル時刻変換をせず、MS-DOS 日時を UTC とみなす
#[cfg(not(unix))]
#[test]
fn materialize_treats_dos_time_as_utc_on_non_unix() {
    let (result, _c) = extract(&[RawEntry::file(b"t.txt", b"x")]);
    let out = result.unwrap();
    // 2021-03-04 05:06:08 UTC
    assert_eq!(mtime_secs(&out.join("t.txt")), 1_614_834_368);
    archive::cleanup(&out);
}

#[cfg(unix)]
#[test]
fn materialize_keeps_executable_bit() {
    use std::os::unix::fs::PermissionsExt;
    let mut exe = RawEntry::file(b"run.sh", b"#!/bin/sh\n");
    exe.external_attr = 0o100755 << 16;
    let (result, _c) = extract(&[exe, RawEntry::file(b"plain.txt", b"p")]);
    let out = result.unwrap();
    let mode = |n: &str| std::fs::metadata(out.join(n)).unwrap().permissions().mode();
    assert_ne!(mode("run.sh") & 0o100, 0);
    assert_eq!(mode("plain.txt") & 0o100, 0);
    archive::cleanup(&out);
}

#[test]
fn rejects_parent_directory_entries() {
    let before = temp_zip_dirs();
    for name in [
        &b"../evil.txt"[..],
        b"a/../../evil.txt",
        b"a/b/../../../evil.txt",
        b"..\\evil.txt",
    ] {
        let (result, _c) = extract(&[RawEntry::file(b"ok.txt", b"ok"), RawEntry::file(name, b"x")]);
        let e = result.expect_err("zip-slip は拒否されるべき");
        assert!(e.0.contains("展開先の外"), "{}", e.0);
    }
    // 失敗時は作りかけを残さない(並行テストの影響を避けるため減っていないことだけ見る)
    assert!(temp_zip_dirs() <= before + 8);
}

#[test]
fn rejects_absolute_paths() {
    for name in [&b"/etc/evil.txt"[..], b"C:/evil.txt", b"c:\\evil.txt"] {
        let (result, _c) = extract(&[RawEntry::file(name, b"x")]);
        let e = result.expect_err("絶対パスは拒否されるべき");
        assert!(e.0.contains("絶対パス"), "{}", e.0);
    }
}

#[test]
fn failed_extraction_removes_partial_directory() {
    let dir = scratch("zip-fail");
    let _c = Cleanup(dir.clone());
    let marker = format!("fail-marker-{}", std::process::id());
    let zip = write_zip(
        &dir,
        "bad.zip",
        &[
            RawEntry::file(marker.as_bytes(), b"first"),
            RawEntry::file(b"../evil.txt", b"x"),
        ],
    );
    assert!(archive::materialize_archive(&zip).is_err());
    // 展開済みだった marker を含むディレクトリは残っていない
    let leftover = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("DiffLoupe-zip-")
                && e.path().join(&marker).exists()
        });
    assert!(!leftover);
}

#[test]
fn skips_symlink_entries() {
    let mut link = RawEntry::file(b"link", b"/etc/passwd");
    link.external_attr = 0o120777 << 16;
    let (result, _c) = extract(&[link, RawEntry::file(b"real.txt", b"real")]);
    let out = result.unwrap();
    assert!(std::fs::symlink_metadata(out.join("link")).is_err());
    assert!(out.join("real.txt").exists());
    archive::cleanup(&out);
}

#[test]
fn rejects_encrypted_zip_with_japanese_message() {
    let mut enc = RawEntry::file(b"secret.txt", b"xxxxxxxxxxxxxxxx");
    enc.flags = 1;
    let (result, _c) = extract(&[enc]);
    let e = result.expect_err("暗号化 zip はエラー");
    assert!(
        e.0.contains("パスワード付き zip には対応していません"),
        "{}",
        e.0
    );
}

#[test]
fn decodes_shift_jis_names_without_utf8_flag() {
    let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("日本語/テスト.txt");
    assert!(std::str::from_utf8(&sjis).is_err());
    let (result, _c) = extract(&[RawEntry::file(&sjis, b"sjis\n")]);
    let out = result.unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("日本語/テスト.txt")).unwrap(),
        "sjis\n"
    );
    archive::cleanup(&out);
}

#[test]
fn skips_macosx_and_ds_store() {
    let (result, _c) = extract(&[
        RawEntry::file(b"keep.txt", b"k"),
        RawEntry::file(b"__MACOSX/._keep.txt", b"junk"),
        RawEntry::file(b".DS_Store", b"junk"),
        RawEntry::file(b"sub/.DS_Store", b"junk"),
        RawEntry::file(b"sub/keep2.txt", b"k2"),
    ]);
    let out = result.unwrap();
    assert!(out.join("keep.txt").exists());
    assert!(out.join("sub/keep2.txt").exists());
    assert!(!out.join("__MACOSX").exists());
    assert!(!out.join(".DS_Store").exists());
    assert!(!out.join("sub/.DS_Store").exists());
    archive::cleanup(&out);
}

#[test]
fn reports_invalid_zip_file() {
    let dir = scratch("zip-bad");
    let _c = Cleanup(dir.clone());
    let path = dir.join("not.zip");
    std::fs::write(&path, b"this is not a zip").unwrap();
    assert!(archive::materialize_archive(&path).is_err());
    assert!(archive::materialize_archive(&dir.join("missing.zip")).is_err());
}

#[test]
fn limits_are_defined_as_specified() {
    assert_eq!(archive::MAX_ENTRIES, 200_000);
    assert_eq!(archive::MAX_TOTAL_BYTES, 8 * 1024 * 1024 * 1024);
}

#[test]
fn deflate_compressed_zip_roundtrips() {
    use std::io::Write;
    let dir = scratch("zip-deflate");
    let _c = Cleanup(dir.clone());
    let path = dir.join("d.zip");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("big.txt", opts).unwrap();
    let body = "abc\n".repeat(50_000);
    w.write_all(body.as_bytes()).unwrap();
    w.finish().unwrap();
    let out = archive::materialize_archive(&path).unwrap();
    assert_eq!(std::fs::read_to_string(out.join("big.txt")).unwrap(), body);
    archive::cleanup(&out);
}

#[test]
fn skips_dot_git_variants_silently() {
    let (result, _c) = extract(&[
        RawEntry::file(b"keep.txt", b"k"),
        RawEntry::file(b".GIT/hooks/x", b"evil"),
        RawEntry::file(b".git./x", b"evil"),
        RawEntry::file(b".git /x", b"evil"),
        RawEntry::file(".g\u{200c}it/x".as_bytes(), b"evil"),
        RawEntry::file(".git\u{feff}/config".as_bytes(), b"evil"),
        RawEntry::file(b"GIT~1/x", b"evil"),
        RawEntry::file(b"git~1/y", b"evil"),
        RawEntry::file(b"sub/.git/config", b"evil"),
        RawEntry::file(b"sub/ok.txt", b"ok"),
        RawEntry::file(b".gitignore", b"allowed"),
    ]);
    let out = result.unwrap();
    assert!(out.join("keep.txt").exists());
    assert!(out.join("sub/ok.txt").exists());
    assert!(out.join(".gitignore").exists());
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec![".gitignore", "keep.txt", "sub"]);
    assert_eq!(std::fs::read_dir(out.join("sub")).unwrap().count(), 1);
    archive::cleanup(&out);
}

#[test]
fn rejects_windows_unsafe_names_only_on_windows() {
    for name in [
        &b"ab:c.txt"[..],
        b"dir/x:y",
        b"CON",
        b"con.txt",
        b"sub/NUL.log",
        b"aux",
        b"COM1",
        b"lpt9.txt",
        b"trailing.",
        b"trailing ",
        b"dir./x",
    ] {
        let (result, _c) = extract(&[RawEntry::file(name, b"x")]);
        // コロン・予約名・末尾ドット/空白が危険なのは Windows だけ。他の OS では正当な名前として展開する。
        if cfg!(windows) {
            let e = result.expect_err(&format!(
                "拒否されるべき: {}",
                String::from_utf8_lossy(name)
            ));
            assert!(
                e.0.contains("展開できない名前"),
                "{}: {}",
                String::from_utf8_lossy(name),
                e.0
            );
        } else {
            let out = result.unwrap_or_else(|e| panic!("{}: {}", String::from_utf8_lossy(name), e));
            archive::cleanup(&out);
        }
    }
    // 誤検出しない名前
    for name in [
        &b"console.txt"[..],
        b"COM10",
        b"auxiliary",
        b"a.b.c",
        b"my.git",
    ] {
        let (result, _c) = extract(&[RawEntry::file(name, b"x")]);
        let out = result.unwrap_or_else(|e| panic!("{}: {}", String::from_utf8_lossy(name), e));
        archive::cleanup(&out);
    }
}

#[test]
fn duplicate_file_names_are_reported_as_conflict() {
    let (result, _c) = extract(&[
        RawEntry::file(b"same.txt", b"first"),
        RawEntry::file(b"same.txt", b"second"),
    ]);
    let e = result.expect_err("同名の重複はエラー");
    assert!(
        e.0.contains("zip 内でファイル名が衝突しています"),
        "{}",
        e.0
    );
}

#[test]
fn case_variant_names_conflict_on_case_insensitive_filesystems() {
    // 大文字小文字だけが違う名前は、大文字小文字を区別しない FS では同じ実体になる。
    // その場合は create_new が失敗し、衝突エラーになる(区別する FS では両方展開される)。
    let probe = scratch("case-probe");
    std::fs::write(probe.join("a"), "").unwrap();
    let insensitive = probe.join("A").exists();
    let _ = std::fs::remove_dir_all(&probe);
    let (result, _c) = extract(&[
        RawEntry::file(b"Case.txt", b"first"),
        RawEntry::file(b"case.txt", b"second"),
    ]);
    if insensitive {
        let e = result.expect_err("大文字小文字違いの衝突はエラー");
        assert!(
            e.0.contains("zip 内でファイル名が衝突しています: case.txt"),
            "{}",
            e.0
        );
    } else {
        archive::cleanup(&result.unwrap());
    }
}

#[test]
fn cleanup_only_removes_owned_temp_directories() {
    // temp 外: 接頭辞が正しくても消さない
    let outside = std::env::current_dir()
        .unwrap()
        .join(format!("DiffLoupe-zip-outside-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "keep").unwrap();
    archive::cleanup(&outside);
    assert!(outside.join("keep.txt").exists());
    let _ = std::fs::remove_dir_all(&outside);

    // temp 内でも接頭辞が違えば消さない
    let other = scratch("not-owned");
    std::fs::write(other.join("keep.txt"), "keep").unwrap();
    archive::cleanup(&other);
    assert!(other.join("keep.txt").exists());
    let _ = std::fs::remove_dir_all(&other);

    // temp の孫は消さない
    let parent = scratch("parent");
    let nested = parent.join("DiffLoupe-zip-nested");
    std::fs::create_dir_all(&nested).unwrap();
    archive::cleanup(&nested);
    assert!(nested.exists());
    let _ = std::fs::remove_dir_all(&parent);

    // 正規の展開先は消える
    let (result, _c) = extract(&[RawEntry::file(b"a.txt", b"a")]);
    let out = result.unwrap();
    archive::cleanup(&out);
    assert!(!out.exists());
}
