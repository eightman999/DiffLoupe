// ビルドスクリプト: Windows ターゲットではアプリケーションマニフェストを
// link.exe の /MANIFESTINPUT で exe に埋め込む。
// (per-monitor DPI v2 対応と Common Controls v6 の有効化が目的)
// 外部ビルド依存は増やさず、link.exe の標準機能だけを使う。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("diffloupe-win.manifest")
        .canonicalize()
        .expect("diffloupe-win.manifest が見つかりません");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    println!("cargo:rerun-if-changed=diffloupe-win.manifest");
}
