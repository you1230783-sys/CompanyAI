//! Windows 內建 WIC 轉檔；不安裝套件、不覆寫來源、不把原始圖片附到後續上下文。
use crate::AppResult;
use windows::{
    core::{w, PWSTR},
    Win32::{
        Foundation::{HGLOBAL, RPC_E_CHANGED_MODE},
        Graphics::Imaging::*,
        System::{
            Com::{StructuredStorage::*, *},
            Variant::VARIANT,
        },
    },
};

pub struct Jpeg {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
}

struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn convert(bytes: &[u8], extension: &str) -> AppResult<Jpeg> {
    // 若呼叫執行緒已在 STA，沿用其 apartment；只平衡本次成功的初始化。
    let status = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if status.is_err() && status != RPC_E_CHANGED_MODE {
        return Err(format!("無法初始化圖片轉檔：{status:?}"));
    }
    let _apartment = Apartment(status.is_ok());
    unsafe { convert_inner(bytes, extension) }.map_err(|e| format!("圖片轉 JPG 失敗：{e}"))
}

unsafe fn convert_inner(bytes: &[u8], extension: &str) -> AppResult<Jpeg> {
    let result = (|| -> windows::core::Result<Jpeg> {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        // WICStream 使用此 slice；bytes 活到 decoder 與 stream 釋放之後。
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        let expected = match extension {
            "jpg" | "jpeg" => GUID_ContainerFormatJpeg,
            "png" => GUID_ContainerFormatPng,
            "bmp" => GUID_ContainerFormatBmp,
            "tif" | "tiff" => GUID_ContainerFormatTiff,
            "gif" => GUID_ContainerFormatGif,
            _ => {
                return Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_INVALIDARG,
                ))
            }
        };
        if decoder.GetContainerFormat()? != expected {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "副檔名與圖片內容不一致",
            ));
        }
        let frames = decoder.GetFrameCount()?;
        let frame = decoder.GetFrame(0)?;
        let (mut width, mut height) = (0, 0);
        frame.GetSize(&mut width, &mut height)?;
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || u64::from(width) * u64::from(height) > 16_000_000
        {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "圖片限8192×8192以内、總計1600萬像素",
            ));
        }
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let mut pixels = vec![0; width as usize * height as usize * 4];
        converter.CopyPixels(std::ptr::null(), width * 4, &mut pixels)?;
        let orientation = frame
            .GetMetadataQueryReader()
            .ok()
            .and_then(|reader| {
                [w!("/app1/ifd/{ushort=274}"), w!("/ifd/{ushort=274}")]
                    .iter()
                    .find_map(|name| {
                        let mut value = PROPVARIANT::default();
                        reader.GetMetadataByName(*name, &mut value).ok()?;
                        PropVariantToUInt32(&value)
                            .ok()
                            .filter(|n| (1..=8).contains(n))
                    })
            })
            .unwrap_or(1);
        let (pixels, width, height) = flatten(&pixels, width, height, orientation);
        let output = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatJpeg, std::ptr::null())?;
        encoder.Initialize(&output, WICBitmapEncoderNoCache)?;
        let (mut frame, mut options) = (None, None);
        encoder.CreateNewFrame(&mut frame, &mut options)?;
        let frame = frame.ok_or_else(windows::core::Error::from_thread)?;
        if let Some(options) = options.as_ref() {
            let mut name: Vec<u16> = "ImageQuality\0".encode_utf16().collect();
            let property = PROPBAG2 {
                pstrName: PWSTR(name.as_mut_ptr()),
                ..Default::default()
            };
            options.Write(1, &property, &VARIANT::from(0.85f32))?;
        }
        frame.Initialize(options.as_ref())?;
        frame.SetSize(width, height)?;
        let mut format = GUID_WICPixelFormat24bppBGR;
        frame.SetPixelFormat(&mut format)?;
        if format != GUID_WICPixelFormat24bppBGR {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "JPEG編碼器未接受BGR格式",
            ));
        }
        frame.WritePixels(height, width * 3, &pixels)?;
        frame.Commit()?;
        encoder.Commit()?;
        let mut stat = STATSTG::default();
        output.Stat(&mut stat, STATFLAG_NONAME)?;
        if stat.cbSize == 0 || stat.cbSize > super::input::MAX_BYTES as u64 {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "轉成JPG後仍超過5 MB（5,000,000 bytes），請先縮小圖片",
            ));
        }
        let mut bytes = vec![0; stat.cbSize as usize];
        output.Seek(0, STREAM_SEEK_SET, None)?;
        let mut read = 0;
        output
            .Read(
                bytes.as_mut_ptr().cast(),
                bytes.len() as u32,
                Some(&mut read),
            )
            .ok()?;
        if read as usize != bytes.len() {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "JPEG輸出不完整",
            ));
        }
        Ok(Jpeg {
            bytes,
            width,
            height,
            frames,
        })
    })();
    result.map_err(|e| e.to_string())
}

/// 透明處合成白底；套用 EXIF 方向後移除中繼資料，避免旋轉照片辨識方向錯誤。
fn flatten(rgba: &[u8], width: u32, height: u32, orientation: u32) -> (Vec<u8>, u32, u32) {
    let (out_width, out_height) = if orientation >= 5 {
        (height, width)
    } else {
        (width, height)
    };
    let mut bgr = vec![0; out_width as usize * out_height as usize * 3];
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = match orientation {
                2 => (width - 1 - x, y),
                3 => (width - 1 - x, height - 1 - y),
                4 => (x, height - 1 - y),
                5 => (y, x),
                6 => (height - 1 - y, x),
                7 => (height - 1 - y, width - 1 - x),
                8 => (y, width - 1 - x),
                _ => (x, y),
            };
            let source = ((y * width + x) * 4) as usize;
            let target = ((dy * out_width + dx) * 3) as usize;
            let alpha = u32::from(rgba[source + 3]);
            for channel in 0..3 {
                bgr[target + channel] =
                    ((u32::from(rgba[source + 2 - channel]) * alpha + 255 * (255 - alpha) + 127)
                        / 255) as u8;
            }
        }
    }
    (bgr, out_width, out_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codecs_convert_and_reject_disguised_files() {
        for (data, extension) in [
            (
                include_bytes!("../../../examples/fixtures/vision.png").as_slice(),
                "png",
            ),
            (
                include_bytes!("../../../examples/fixtures/vision.jpg").as_slice(),
                "jpg",
            ),
            (
                include_bytes!("../../../examples/fixtures/vision.bmp").as_slice(),
                "bmp",
            ),
            (
                include_bytes!("../../../examples/fixtures/vision.tiff").as_slice(),
                "tiff",
            ),
            (
                include_bytes!("../../../examples/fixtures/vision.gif").as_slice(),
                "gif",
            ),
        ] {
            let result = convert(data, extension).unwrap();
            assert_eq!((result.width, result.height, result.frames), (96, 64, 1));
            assert!(result.bytes.starts_with(&[0xff, 0xd8]));
            assert!(convert(data, if extension == "png" { "tiff" } else { "png" }).is_err());
        }
        assert!(convert(b"not an image", "bmp").is_err());
    }
    #[test]
    fn orientation_and_alpha_are_preserved_visually() {
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 0, 0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 0, 255, 0, 0,
            0, 255,
        ];
        let (pixels, w, h) = flatten(&rgba, 2, 3, 6);
        assert_eq!((w, h), (3, 2));
        assert_eq!(&pixels[..9], &[0, 255, 255, 255, 0, 0, 0, 0, 255]);
        assert_eq!(&pixels[15..], &[255, 255, 255]);
    }
}
