//! 画像差分モデル(Swift `ImageDiffEngine` のピクセル処理部分相当)。
//! デコードは OS 側(Windows では WIC)が RGBA8 バッファを供給し、
//! ここではデコード非依存の処理(共通キャンバス・リサイズ・差分生成・
//! ビューポート計算)だけを行う。ホスト OS でテスト可能。

/// 既定の色差閾値(Swift `ImageDiffEngine.defaultThreshold`)
pub const DEFAULT_THRESHOLD: i32 = 30;
/// 差分計算の最大ピクセル数(超過時はダウンサンプリング)
pub const MAX_COMPARISON_PIXELS: usize = 16_000_000;
/// ズーム範囲(Swift `ImageViewport`)
pub const MIN_SCALE: f64 = 0.25;
pub const MAX_SCALE: f64 = 4.0;

/// RGBA8(ストレート→実用上は premultiplied 相当でもよい。チャンネル順は R,G,B,A)
#[derive(Debug, Clone, PartialEq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// RGBA の 4 バイト/ピクセル
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(pixels.len(), (width * height * 4) as usize);
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for px in pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        Self::new(width, height, pixels)
    }

    /// バイリニア補間で (w,h) にリサイズして新しい画像を返す。
    /// CGContext(.high) による縮小と同等の視覚品質を目指す近似実装。
    pub fn resized(&self, w: u32, h: u32) -> RgbaImage {
        if self.width == w && self.height == h {
            return self.clone();
        }
        let (sw, sh) = (self.width as f64, self.height as f64);
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            // 出力ピクセル中心をソース座標へ写像
            let sy = ((y as f64 + 0.5) * sh / h as f64 - 0.5).clamp(0.0, sh - 1.0);
            let y0 = sy.floor() as u32;
            let y1 = (y0 + 1).min(self.height - 1);
            let fy = sy - y0 as f64;
            for x in 0..w {
                let sx = ((x as f64 + 0.5) * sw / w as f64 - 0.5).clamp(0.0, sw - 1.0);
                let x0 = sx.floor() as u32;
                let x1 = (x0 + 1).min(self.width - 1);
                let fx = sx - x0 as f64;
                let di = ((y * w + x) * 4) as usize;
                for c in 0..4 {
                    let p00 = self.pixels[((y0 * self.width + x0) * 4) as usize + c] as f64;
                    let p10 = self.pixels[((y0 * self.width + x1) * 4) as usize + c] as f64;
                    let p01 = self.pixels[((y1 * self.width + x0) * 4) as usize + c] as f64;
                    let p11 = self.pixels[((y1 * self.width + x1) * 4) as usize + c] as f64;
                    let v = p00 * (1.0 - fx) * (1.0 - fy)
                        + p10 * fx * (1.0 - fy)
                        + p01 * (1.0 - fx) * fy
                        + p11 * fx * fy;
                    out[di + c] = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        RgbaImage::new(w, h, out)
    }
}

/// 比較キャンバス(両画像を共通サイズへ正規化。超過時は縮小)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonCanvas {
    pub width: u32,
    pub height: u32,
    pub downsampled: bool,
}

pub fn comparison_canvas(lw: u32, lh: u32, rw: u32, rh: u32) -> ComparisonCanvas {
    let mut w = lw.max(rw);
    let mut h = lh.max(rh);
    let pixels = (w as usize) * (h as usize);
    if pixels <= MAX_COMPARISON_PIXELS {
        return ComparisonCanvas {
            width: w,
            height: h,
            downsampled: false,
        };
    }
    let scale = (MAX_COMPARISON_PIXELS as f64 / pixels as f64).sqrt();
    w = ((w as f64) * scale).max(1.0) as u32;
    h = ((h as f64) * scale).max(1.0) as u32;
    ComparisonCanvas {
        width: w,
        height: h,
        downsampled: true,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageFileMetadata {
    pub width: u32,
    pub height: u32,
    pub file_size: i64,
    pub format: String,
}

impl ImageFileMetadata {
    pub fn dimensions_text(&self) -> String {
        if self.width == 0 || self.height == 0 {
            return crate::strings::IMG_UNKNOWN.into();
        }
        format!("{} x {} px", self.width, self.height)
    }
    pub fn file_size_text(&self) -> String {
        crate::viewmodel::byte_count_text(self.file_size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDiffStats {
    pub compared_width: u32,
    pub compared_height: u32,
    pub diff_pixel_count: usize,
    pub diff_percentage: f64,
    pub threshold: i32,
    pub used_downsampled_comparison: bool,
}

impl ImageDiffStats {
    pub fn compared_dimensions_text(&self) -> String {
        format!("{} x {} px", self.compared_width, self.compared_height)
    }
}

pub struct ImageComparisonResult {
    pub left: RgbaImage,
    pub right: RgbaImage,
    pub diff_image: RgbaImage,
    pub left_metadata: ImageFileMetadata,
    pub right_metadata: ImageFileMetadata,
    pub stats: ImageDiffStats,
}

impl ImageComparisonResult {
    pub fn has_different_dimensions(&self) -> bool {
        self.left_metadata.width != self.right_metadata.width
            || self.left_metadata.height != self.right_metadata.height
    }
}

fn composite_on_white(r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8) {
    let alpha = a as f64 / 255.0;
    (
        (r as f64 * alpha + 255.0 * (1.0 - alpha)).round() as u8,
        (g as f64 * alpha + 255.0 * (1.0 - alpha)).round() as u8,
        (b as f64 * alpha + 255.0 * (1.0 - alpha)).round() as u8,
    )
}

fn blend(base: u8, overlay: u8, alpha: f64) -> u8 {
    (base as f64 * (1.0 - alpha) + overlay as f64 * alpha).round() as u8
}

/// 差分画像の生成(Swift `generateDiffImage` 相当)。
/// 左右はすでに共通キャンバスサイズへ正規化済みとする。
pub fn generate_diff_image(
    left: &RgbaImage,
    right: &RgbaImage,
    threshold: i32,
) -> (RgbaImage, usize) {
    debug_assert_eq!(left.width, right.width);
    debug_assert_eq!(left.height, right.height);
    let n = (left.width * left.height * 4) as usize;
    let mut out = vec![255u8; n];
    let mut diff_count = 0usize;
    for i in (0..n).step_by(4) {
        let (r, g, b) = composite_on_white(
            left.pixels[i],
            left.pixels[i + 1],
            left.pixels[i + 2],
            left.pixels[i + 3],
        );
        let distance = (left.pixels[i] as i32 - right.pixels[i] as i32).abs()
            + (left.pixels[i + 1] as i32 - right.pixels[i + 1] as i32).abs()
            + (left.pixels[i + 2] as i32 - right.pixels[i + 2] as i32).abs()
            + (left.pixels[i + 3] as i32 - right.pixels[i + 3] as i32).abs();
        if distance > threshold {
            out[i] = blend(r, 255, 0.68);
            out[i + 1] = blend(g, 0, 0.68);
            out[i + 2] = blend(b, 0, 0.68);
            out[i + 3] = 255;
            diff_count += 1;
        } else {
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = 255;
        }
    }
    (RgbaImage::new(left.width, left.height, out), diff_count)
}

/// 左右デコード済み画像とメタデータから差分結果を作る(Swift `ImageDiffEngine.compare` 相当)
pub fn compare(
    left: RgbaImage,
    right: RgbaImage,
    left_meta: ImageFileMetadata,
    right_meta: ImageFileMetadata,
    threshold: i32,
) -> ImageComparisonResult {
    let canvas = comparison_canvas(left.width, left.height, right.width, right.height);
    let left_n = left.resized(canvas.width, canvas.height);
    let right_n = right.resized(canvas.width, canvas.height);
    let (diff_image, diff_count) = generate_diff_image(&left_n, &right_n, threshold);
    let total = (canvas.width as usize * canvas.height as usize).max(1);
    ImageComparisonResult {
        left,
        right,
        diff_image,
        left_metadata: left_meta,
        right_metadata: right_meta,
        stats: ImageDiffStats {
            compared_width: canvas.width,
            compared_height: canvas.height,
            diff_pixel_count: diff_count,
            diff_percentage: diff_count as f64 / total as f64 * 100.0,
            threshold,
            used_downsampled_comparison: canvas.downsampled,
        },
    }
}

/// 共有ビューポート(Swift `ImageViewport` 相当)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageViewport {
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

impl Default for ImageViewport {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

impl ImageViewport {
    pub fn clamped_scale(scale: f64) -> f64 {
        scale.clamp(MIN_SCALE, MAX_SCALE)
    }

    /// 画像をビューに収めたときの表示サイズ(アスペクト比維持)
    pub fn fitted_size(iw: f64, ih: f64, vw: f64, vh: f64) -> (f64, f64) {
        if iw <= 0.0 || ih <= 0.0 || vw <= 0.0 || vh <= 0.0 {
            return (0.0, 0.0);
        }
        let ratio = (vw / iw).min(vh / ih);
        (iw * ratio, ih * ratio)
    }

    /// パンの許容範囲(画像が画面外に完全に消えない最大オフセット)
    pub fn pan_limits(dw: f64, dh: f64, vw: f64, vh: f64) -> (f64, f64) {
        (((dw - vw) / 2.0).max(0.0), ((dh - vh) / 2.0).max(0.0))
    }

    pub fn clamp_offset(&mut self, dw: f64, dh: f64, vw: f64, vh: f64) {
        let (lx, ly) = Self::pan_limits(dw, dh, vw, vh);
        self.offset_x = self.offset_x.clamp(-lx, lx);
        self.offset_y = self.offset_y.clamp(-ly, ly);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_not_downsampled_below_limit() {
        let c = comparison_canvas(100, 100, 200, 50);
        assert_eq!((c.width, c.height, c.downsampled), (200, 100, false));
    }

    #[test]
    fn canvas_downsamples_over_limit() {
        let c = comparison_canvas(8000, 4000, 8000, 4000); // 32MP
        assert!(c.downsampled);
        assert!((c.width as usize * c.height as usize) <= MAX_COMPARISON_PIXELS);
    }

    #[test]
    fn diff_counts_changed_pixels() {
        let left = RgbaImage::solid(4, 4, [0, 0, 0, 255]);
        let mut right = RgbaImage::solid(4, 4, [0, 0, 0, 255]);
        // 1ピクセルだけ大きく変える
        right.pixels[0] = 200;
        right.pixels[1] = 200;
        right.pixels[2] = 200;
        let (img, count) = generate_diff_image(&left, &right, 30);
        assert_eq!(count, 1);
        // 差分ピクセルは赤系になる(ブレンド: R≒232, G≒82, B≒82)
        assert!(img.pixels[0] > img.pixels[1]);
        assert_eq!(img.pixels.len(), 4 * 4 * 4);
    }

    #[test]
    fn diff_threshold_boundary() {
        let left = RgbaImage::solid(1, 1, [0, 0, 0, 255]);
        let right = RgbaImage::solid(1, 1, [10, 10, 10, 255]); // distance = 30
        let (_, count) = generate_diff_image(&left, &right, 30);
        assert_eq!(count, 0); // distance > threshold で差分(等値は一致)
        let (_, count) = generate_diff_image(&left, &right, 29);
        assert_eq!(count, 1);
    }

    #[test]
    fn resize_half() {
        let img = RgbaImage::solid(4, 4, [100, 50, 200, 255]);
        let small = img.resized(2, 2);
        assert_eq!((small.width, small.height), (2, 2));
        assert_eq!(small.pixels[0..4], [100, 50, 200, 255]);
    }

    #[test]
    fn viewport_fit_and_clamp() {
        let (w, h) = ImageViewport::fitted_size(200.0, 100.0, 100.0, 100.0);
        assert_eq!((w, h), (100.0, 50.0));
        let mut vp = ImageViewport {
            scale: 4.0,
            offset_x: 1000.0,
            offset_y: -1000.0,
        };
        vp.clamp_offset(400.0, 400.0, 100.0, 100.0);
        assert_eq!(vp.offset_x, 150.0);
        assert_eq!(vp.offset_y, -150.0);
    }
}
