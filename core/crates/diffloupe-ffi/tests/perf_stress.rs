//! 1万ファイル規模フィクスチャでの比較本体と FFI 型変換の所要時間を測る(手動実行用)。
//! `DIFFLOUPE_STRESS_DIR=/tmp/x cargo test --profile release-ffi -p diffloupe-ffi --test perf_stress -- --ignored --nocapture`
use std::path::Path;
use std::time::Instant;

use diffloupe_core::comparator::{CompareOptions, FolderComparator};
use diffloupe_ffi::types::FfiComparisonResult;

#[test]
#[ignore]
fn stress_breakdown() {
    let base = std::env::var("DIFFLOUPE_STRESS_DIR").expect("DIFFLOUPE_STRESS_DIR");
    let left = Path::new(&base).join("left");
    let right = Path::new(&base).join("right");
    let opts = CompareOptions::default();
    for _ in 0..5 {
        let t0 = Instant::now();
        let r = FolderComparator::compare(&left, &right, &opts, None, None).unwrap();
        let t1 = Instant::now();
        let f: FfiComparisonResult = r.into();
        let t2 = Instant::now();
        println!(
            "core_compare={:.3}s into_ffi={:.3}s entries={}",
            (t1 - t0).as_secs_f64(),
            (t2 - t1).as_secs_f64(),
            f.entries.len()
        );
    }
}
