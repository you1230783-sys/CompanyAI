//! Python圖片通過WIC尺寸及完整解碼後才交付；不接受SVG、外部資源或任意格式。
use crate::AppResult;
use windows::Win32::{Graphics::Imaging::*, System::Com::*};

pub(in crate::projects) fn validate(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() > super::MAX_OUTPUT || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Python圖片必須是8MiB以內的PNG。".into());
    }
    let _apartment = crate::com_apartment::Apartment::for_imaging().map_err(|e| e.to_string())?;
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.to_string())?;
        let stream = factory.CreateStream().map_err(|e| e.to_string())?;
        stream
            .InitializeFromMemory(bytes)
            .map_err(|e| e.to_string())?;
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(|e| e.to_string())?;
        if decoder.GetContainerFormat().map_err(|e| e.to_string())? != GUID_ContainerFormatPng
            || decoder.GetFrameCount().map_err(|e| e.to_string())? != 1
        {
            return Err("Python圖片只接受單張PNG。".into());
        }
        let frame = decoder.GetFrame(0).map_err(|e| e.to_string())?;
        let (mut width, mut height) = (0, 0);
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|e| e.to_string())?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Python圖片尺寸需為1至4096像素。".into());
        }
        let converter = factory.CreateFormatConverter().map_err(|e| e.to_string())?;
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| e.to_string())?;
        let mut pixels = vec![0; width as usize * height as usize * 4];
        converter
            .CopyPixels(std::ptr::null(), width * 4, &mut pixels)
            .map_err(|e| format!("Python PNG解碼失敗：{e}"))?;
    }
    Ok(())
}
