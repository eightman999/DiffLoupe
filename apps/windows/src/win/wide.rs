//! UTF-16 文字列ヘルパ

use windows::core::PCWSTR;

/// null 終端 UTF-16 へ変換
pub fn wstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// null 終端 UTF-16 の PCWSTR(返した Vec を生かしておくこと)
pub fn pcw(s: &str) -> (Vec<u16>, PCWSTR) {
    let v = wstr(s);
    (v.clone(), PCWSTR(v.as_ptr()))
}

/// PCWSTR → String(安全側: null 終端を読む)
///
/// # Safety
/// `p` は null 終端の UTF-16 文字列を指す有効なポインタか、null であること。
pub unsafe fn from_pcw(p: PCWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *p.0.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len))
    }
}
