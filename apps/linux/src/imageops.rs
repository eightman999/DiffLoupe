//! 画像比較のロジック。Swift `ImageDiffEngine` の移植。
//! 画像デコードは gdk-pixbuf(`image` クレートは使わない方針)。
//!
//! - 距離: 4チャンネル(α乗算済み RGBA)の絶対差和(Manhattan)。`distance > threshold` で差分画素。
//! - キャンバス: max(左w,右w) × max(左h,右h)。16M 画素超は `sqrt(16M/pixels)` で縮小。
//! - 差分画像: 一致画素は左画素を白に合成、差分画素は赤(255,0,0, a=0.68)ブレンド。

use std::path::Path;

use gtk4::gdk_pixbuf::Pixbuf;

pub const MAX_COMPARISON_PIXELS: usize = 16_000_000;
pub const DEFAULT_THRESHOLD: i32 = 30;
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "gif", "bmp", "tif", "tiff", "heic", "heif", "webp",
];

pub fn is_image_path(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// α乗算済み RGBA 画素列 + 寸法。
#[derive(Clone)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    /// premultiplied RGBA、4バイト/画素
    pub data: Vec<u8>,
}

impl RgbaImage {
    /// gdk-pixbuf でファイルを読み込み、α乗算済み RGBA に変換する。
    /// (EXIF 回転の自動適用は gdk-pixbuf ローダー依存。代表的な JPEG/PNG はそのまま扱う)
    pub fn load(path: &Path) -> Result<Self, String> {
        let pix = Pixbuf::from_file(path).map_err(|e| e.to_string())?;
        Ok(Self::from_pixbuf(&pix))
    }

    pub fn from_pixbuf(pix: &Pixbuf) -> Self {
        let w = pix.width() as usize;
        let h = pix.height() as usize;
        let bytes = pix.read_pixel_bytes();
        let src = bytes.as_ref();
        let nch = pix.n_channels() as usize;
        let mut data = Vec::with_capacity(w * h * 4);
        for px in src.chunks_exact(nch).take(w * h) {
            let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
            let a = if nch >= 4 { px[3] as u32 } else { 255 };
            // premultiply
            data.push((r * a / 255) as u8);
            data.push((g * a / 255) as u8);
            data.push((b * a / 255) as u8);
            data.push(a as u8);
        }
        Self {
            width: w,
            height: h,
            data,
        }
    }

    /// キャンバス寸法へ最近傍で合わせる(不足分は透明)。縮小係数は呼び出し側が渡す。
    fn resized_to(&self, w: usize, h: usize) -> RgbaImage {
        if w == self.width && h == self.height {
            return self.clone();
        }
        let mut data = vec![0u8; w * h * 4];
        for y in 0..h {
            let sy = (y * self.height / h).min(self.height - 1);
            for x in 0..w {
                let sx = (x * self.width / w).min(self.width - 1);
                let s = (sy * self.width + sx) * 4;
                let d = (y * w + x) * 4;
                data[d..d + 4].copy_from_slice(&self.data[s..s + 4]);
            }
        }
        RgbaImage {
            width: w,
            height: h,
            data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageFileMeta {
    pub width: usize,
    pub height: usize,
    pub file_size: u64,
    pub format: String, // 拡張子大文字 or 不明
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // threshold は統計として保持(UI の再計算表示用)
pub struct ImageDiffStats {
    pub compared_width: usize,
    pub compared_height: usize,
    pub diff_pixel_count: usize,
    pub diff_percentage: f64,
    pub threshold: i32,
    pub used_downsampled_comparison: bool,
    pub size_mismatch: bool,
}

pub struct ImageDiffResult {
    pub left: RgbaImage, // 元画像(表示用・非乗算合成前の premult RGBA)
    pub right: RgbaImage,
    pub diff: RgbaImage, // 差分ハイライト画像(比較キャンバス寸法)
    pub stats: ImageDiffStats,
    pub left_meta: ImageFileMeta,
    pub right_meta: ImageFileMeta,
}

fn meta_of(path: &Path, img: &RgbaImage) -> ImageFileMeta {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| s_unknown().to_string());
    ImageFileMeta {
        width: img.width,
        height: img.height,
        file_size,
        format,
    }
}

fn s_unknown() -> &'static str {
    "不明"
}

fn pixel_distance(a: &[u8], b: &[u8]) -> i32 {
    (a[0] as i32 - b[0] as i32).abs()
        + (a[1] as i32 - b[1] as i32).abs()
        + (a[2] as i32 - b[2] as i32).abs()
        + (a[3] as i32 - b[3] as i32).abs()
}

/// 一致画素: 左画素を白に合成(a_over)。差分画素: 赤 overlay α=0.68。
fn compose_diff_pixel(left_px: &[u8], is_diff: bool) -> [u8; 4] {
    let (r, g, b, a) = (
        left_px[0] as u32,
        left_px[1] as u32,
        left_px[2] as u32,
        left_px[3] as u32,
    );
    // compositeOnWhite: src*a + 255*(1-a)
    let base = [
        ((r * a + 255 * (255 - a)) / 255) as u8,
        ((g * a + 255 * (255 - a)) / 255) as u8,
        ((b * a + 255 * (255 - a)) / 255) as u8,
    ];
    if !is_diff {
        return [base[0], base[1], base[2], 255];
    }
    // blend(base, overlay red, alpha 0.68) = base*0.32 + red*0.68
    let blend = |c: u8, o: u8| ((c as u32 * 82 + o as u32 * 173) / 255) as u8;
    [
        blend(base[0], 255),
        blend(base[1], 0),
        blend(base[2], 0),
        255,
    ]
}

#[allow(dead_code)] // 経路2(パスから一括計算)。compute_diff が主経路
pub fn compare_images(
    left_path: &Path,
    right_path: &Path,
    threshold: i32,
) -> Result<ImageDiffResult, String> {
    let left = RgbaImage::load(left_path).map_err(|_| {
        crate::strings::img_load_error(
            left_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        )
    })?;
    let right = RgbaImage::load(right_path).map_err(|_| {
        crate::strings::img_load_error(
            right_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        )
    })?;
    Ok(compute_diff(left, right, threshold, left_path, right_path))
}

/// デコード済み画像から差分を計算(閾値スライダーの再計算用に公開)。
pub fn compute_diff(
    left: RgbaImage,
    right: RgbaImage,
    threshold: i32,
    left_path: &Path,
    right_path: &Path,
) -> ImageDiffResult {
    let left_meta = meta_of(left_path, &left);
    let right_meta = meta_of(right_path, &right);
    let mut w = left.width.max(right.width);
    let mut h = left.height.max(right.height);
    let size_mismatch = left.width != right.width || left.height != right.height;
    let mut used_downsampled = false;
    if w * h > MAX_COMPARISON_PIXELS {
        let scale = (MAX_COMPARISON_PIXELS as f64 / (w * h) as f64).sqrt();
        w = ((w as f64) * scale) as usize;
        h = ((h as f64) * scale) as usize;
        used_downsampled = true;
    }
    let l = left.resized_to(w, h);
    let r = right.resized_to(w, h);
    let mut diff_data = vec![0u8; w * h * 4];
    let mut diff_count = 0usize;
    for i in 0..w * h {
        let is_diff =
            pixel_distance(&l.data[i * 4..i * 4 + 4], &r.data[i * 4..i * 4 + 4]) > threshold;
        if is_diff {
            diff_count += 1;
        }
        diff_data[i * 4..i * 4 + 4]
            .copy_from_slice(&compose_diff_pixel(&l.data[i * 4..i * 4 + 4], is_diff));
    }
    let stats = ImageDiffStats {
        compared_width: w,
        compared_height: h,
        diff_pixel_count: diff_count,
        diff_percentage: diff_count as f64 / (w * h) as f64 * 100.0,
        threshold,
        used_downsampled_comparison: used_downsampled,
        size_mismatch,
    };
    ImageDiffResult {
        left,
        right,
        diff: RgbaImage {
            width: w,
            height: h,
            data: diff_data,
        },
        stats,
        left_meta,
        right_meta,
    }
}

/// premultiplied RGBA → cairo::ImageSurface(ARgb32 は LE で B,G,R,A)。
pub fn to_cairo_surface(img: &RgbaImage) -> gtk4::cairo::ImageSurface {
    let mut surf = gtk4::cairo::ImageSurface::create(
        gtk4::cairo::Format::ARgb32,
        img.width as i32,
        img.height as i32,
    )
    .expect("cairo surface");
    {
        let stride = surf.stride() as usize;
        let mut data = surf.data().expect("cairo data");
        for y in 0..img.height {
            for x in 0..img.width {
                let s = (y * img.width + x) * 4;
                let d = y * stride + x * 4;
                if d + 3 < data.len() {
                    data[d] = img.data[s + 2]; // B
                    data[d + 1] = img.data[s + 1]; // G
                    data[d + 2] = img.data[s]; // R
                    data[d + 3] = img.data[s + 3]; // A(乗算済み)
                }
            }
        }
    }
    surf.mark_dirty();
    surf
}

/// ファイルサイズの人間向け表記(Swift 版の ByteCountFormatter 相当の簡易版)。
pub fn human_size(n: u64) -> String {
    const KB: f64 = 1000.0;
    let f = n as f64;
    if f < KB {
        format!("{n} バイト")
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} MB", f / (KB * KB))
    } else {
        format!("{:.1} GB", f / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_distance() {
        assert_eq!(pixel_distance(&[0, 0, 0, 0], &[0, 0, 0, 0]), 0);
        assert_eq!(pixel_distance(&[10, 20, 30, 40], &[0, 0, 0, 40]), 60);
    }

    #[test]
    fn diff_pixel_blend() {
        // 差分なしの不透明画素はそのまま
        assert_eq!(
            compose_diff_pixel(&[200, 100, 50, 255], false),
            [200, 100, 50, 255]
        );
        // 差分ありは赤寄り
        let p = compose_diff_pixel(&[0, 0, 0, 255], true);
        assert!(p[0] > 150 && p[1] < 80 && p[3] == 255);
    }

    #[test]
    fn resize_nearest() {
        let img = RgbaImage {
            width: 2,
            height: 2,
            data: vec![
                255, 0, 0, 255, 0, 255, 0, 255, //
                0, 0, 255, 255, 255, 255, 255, 255,
            ],
        };
        let up = img.resized_to(4, 4);
        assert_eq!(up.width, 4);
        assert_eq!(&up.data[0..4], &[255, 0, 0, 255]);
        assert_eq!(
            &up.data[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4],
            &[255, 255, 255, 255]
        );
    }
}
