//! WIC による画像デコード(外部画像クレート不使用)。
//! 出力は RGBA8 バッファ(`imagemodel::RgbaImage`)。WIC の PBGRA を
//! デコードして R/B チャンネルを入れ替えて RGBA 化する。

use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGR,
    GUID_WICPixelFormat32bppPBGRA, IWICBitmapFrameEncode, IWICFormatConverter, IWICImagingFactory,
    WICBitmapDitherType, WICBitmapEncoderNoCache, WICBitmapPaletteType, WICDecodeOptions,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
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
        // COM オブジェクトは関数末尾で Drop されるため CoUninitialize は呼ばない
        // (先に呼ぶと解放時にアクセス違反になる)。スレッド終了時に OS が片付ける。
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
        for px in buf.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
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

/// 32bpp BGRA(top-down, アルファは無視)を PNG として保存する(スモークのキャプチャ用)
pub fn encode_png(path: &Path, bgra: &[u8], w: i32, h: i32) -> Result<(), String> {
    let e = |what: &str, err: windows::core::Error| format!("PNG {}: {}", what, err);
    unsafe {
        // COM オブジェクトは関数末尾で Drop されるため CoUninitialize は呼ばない
        // (先に呼ぶと解放時にアクセス違反になる)。スレッド終了時に OS が片付ける。
        let _init = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_ALL)
                .map_err(|x| e("factory", x))?;
        let stream = factory.CreateStream().map_err(|x| e("stream", x))?;
        let file = wstr(&path.to_string_lossy());
        stream
            .InitializeFromFilename(
                PCWSTR(file.as_ptr()),
                windows::Win32::Foundation::GENERIC_WRITE.0,
            )
            .map_err(|x| e("open", x))?;
        let encoder = factory
            .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
            .map_err(|x| e("encoder", x))?;
        encoder
            .Initialize(&stream, WICBitmapEncoderNoCache)
            .map_err(|x| e("init", x))?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        let mut props = None;
        encoder
            .CreateNewFrame(&mut frame, &mut props)
            .map_err(|x| e("frame", x))?;
        let frame = frame.ok_or_else(|| "PNG: frame が null".to_string())?;
        frame
            .Initialize(props.as_ref())
            .map_err(|x| e("frame init", x))?;
        frame
            .SetSize(w as u32, h as u32)
            .map_err(|x| e("size", x))?;
        let mut fmt = GUID_WICPixelFormat32bppBGR;
        frame.SetPixelFormat(&mut fmt).map_err(|x| e("format", x))?;
        frame
            .WritePixels(h as u32, (w * 4) as u32, bgra)
            .map_err(|x| e("write", x))?;
        frame.Commit().map_err(|x| e("frame commit", x))?;
        encoder.Commit().map_err(|x| e("commit", x))?;
    }
    Ok(())
}
