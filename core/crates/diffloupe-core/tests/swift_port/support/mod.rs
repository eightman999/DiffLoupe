//! 移植テスト共通ヘルパー。
#![allow(dead_code)]

use diffloupe_core::comparator::{CompareOptions, FolderComparator};
use diffloupe_core::line_diff::{DiffRow, DiffRowKind};
use diffloupe_core::models::ComparisonResult;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 並行テストでの名前衝突防止カウンタ(clock granularity では衝突し得るため)
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn uniq() -> usize {
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// 一時ディレクトリに left/ right/ を作る。戻り値は (base, left, right)。
pub fn temp_pair(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "diffloupe-sp-{}-{}-{}",
        tag,
        std::process::id(),
        uniq()
    ));
    let l = base.join("left");
    let r = base.join("right");
    std::fs::create_dir_all(&l).unwrap();
    std::fs::create_dir_all(&r).unwrap();
    (base, l, r)
}

pub fn write(dir: &Path, rel: &str, data: &[u8]) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, data).unwrap();
}

pub fn compare(l: &Path, r: &Path, opts: &CompareOptions) -> ComparisonResult {
    FolderComparator::compare(l, r, opts, None, None).unwrap()
}

/// テンポラリファイルにデータを書いてパスを返す。
pub fn write_temp(data: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(format!("diffloupe-sp-tl-{}-{}", std::process::id(), uniq()));
    std::fs::write(&p, data).unwrap();
    p
}

/// DiffRow 構築ヘルパー(Swift の DiffRow(id:kind:leftNumber:leftText:rightNumber:rightText:) 相当)
pub fn row(
    id: usize,
    kind: DiffRowKind,
    ln: Option<usize>,
    lt: Option<&str>,
    rn: Option<usize>,
    rt: Option<&str>,
) -> DiffRow {
    DiffRow {
        id,
        kind,
        left_number: ln,
        left_text: lt.map(String::from),
        right_number: rn,
        right_text: rt.map(String::from),
    }
}
