//! WIC による画像デコード(外部画像クレート不使用)。
//! 出力は RGBA8 バッファ(`imagemodel::RgbaImage`)。WIC の PBGRA を
//! デコードして R/B チャンネルを入れ替えて RGBA 化する。

use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICFormatConverter,
    IWICImagingFactory, WICBitmapDitherType, WICBitmapPaletteType, WICDecodeOptions,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use crate::imagemodel::RgbaImage;
use crate::win::wide::wstr;

/// 画像ファイルを RGBA8 へデコードする。失敗時はメッセージを返す。
pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    unsafe {
        let _init = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory =
            match CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_ALL) {
                Ok(f) => f,
                Err(_) => {
                    return Err(format!(
                        "{}: {}",
                        crate::strings::IMG_UNREADABLE_PREFIX,
                        name
                    ))
                }
            };
        let file = wstr(&path.to_string_lossy());
        let decoder = factory
            .CreateDecoderFromFilename(
                PCWSTR(file.as_ptr()),
                None,
                windows::Win32::Foundation::GENERIC_READ,
                WICDecodeOptions(0),
            )
            .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        let frame = decoder
            .GetFrame(0)
            .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        let (mut w, mut h) = (0u32, 0u32);
        frame
            .GetSize(&mut w, &mut h)
            .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        let conv: IWICFormatConverter = factory
            .CreateFormatConverter()
            .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        conv.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherType(0),
            None,
            0.0,
            WICBitmapPaletteType(0),
        )
        .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        let stride = w * 4;
        let mut buf = vec![0u8; (stride * h) as usize];
        conv.CopyPixels(std::ptr::null(), stride, &mut buf)
            .map_err(|_| format!("{}{}", crate::strings::IMG_UNREADABLE_PREFIX, name))?;
        // PBGRA → PRGBA へチャンネル入れ替え(ピクセル単位 B<->R)
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        CoUninitialize();
        Ok(RgbaImage::new(w, h, buf))
    }
}

/// ファイルメタデータ(拡張子から形式名・サイズ)
pub fn metadata(path: &Path, width: u32, height: u32) -> crate::imagemodel::ImageFileMetadata {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| crate::strings::IMG_UNKNOWN.into());
    let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    crate::imagemodel::ImageFileMetadata {
        width,
        height,
        file_size: size,
        format: ext,
    }
}
