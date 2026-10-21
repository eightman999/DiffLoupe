//! パス操作ヘルパ。`URL(fileURLWithPath:)` 相当の正規化を行う。

use std::path::Path;

/// `URL(fileURLWithPath:).path` 相当: 絶対化 + シンボリックリンク解決。
/// 解決できない場合は与えたパスをそのまま返す。
pub fn canonical_display(path: &Path) -> String {
    match std::fs::canonicalize(path) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// `URL.appendingPathComponent` 相当の結合(表示用)
pub fn append_component(root: &str, relative: &str) -> String {
    if root.ends_with('/') {
        format!("{}{}", root, relative)
    } else {
        format!("{}/{}", root, relative)
    }
}

/// NSString `pathExtension` 相当: 最後の `.` 以降(先頭ドットは拡張子なし)
pub fn path_extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some(i) = name.rfind('.') {
        if i > 0 {
            return &name[i + 1..];
        }
    }
    ""
}
