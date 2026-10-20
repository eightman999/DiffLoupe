// UniFFI バインディング生成用のエントリポイント。
// `cargo run -p diffloupe-ffi --bin uniffi-bindgen -- generate --library <lib> --language swift ...`
fn main() {
    uniffi::uniffi_bindgen_main();
}
