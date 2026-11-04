//! 桌寵圖片只接受PNG；以WIC縮到256像素、重編碼並移除額外中繼資料。
//! BGRA保留透明度，儲存結果嚴格小於500,000 bytes，不覆寫使用者原圖。
use crate::AppResult;
use windows::Win32::{
    Foundation::HGLOBAL,
    Graphics::Imaging::*,
    System::Com::{StructuredStorage::*, *},
};

#[derive(Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
}
pub const MAX_BYTES: usize = 500_000;

pub fn normalize(bytes: &[u8]) -> AppResult<(Vec<u8>, Image)> {
    if bytes.len() > 20_000_000 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("請選擇20MB以內的PNG圖片，建議先去背。".into());
    }
    unsafe { normalize_inner(bytes) }.map_err(|e| format!("桌寵PNG處理失敗：{e}"))
}
unsafe fn normalize_inner(bytes: &[u8]) -> AppResult<(Vec<u8>, Image)> {
    let convert = || -> windows::core::Result<(Vec<u8>, Image)> {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        if decoder.GetContainerFormat()? != GUID_ContainerFormatPng {
            return Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_INVALIDARG,
            ));
        }
        let frame = decoder.GetFrame(0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        if w == 0 || h == 0 || w > 8192 || h > 8192 || u64::from(w) * u64::from(h) > 16_000_000 {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "圖片限8192×8192及1600萬像素",
            ));
        }
        let scale = (256.0 / f64::from(w.max(h))).min(1.0);
        w = (f64::from(w) * scale).round().max(1.0) as u32;
        h = (f64::from(h) * scale).round().max(1.0) as u32;
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(&frame, w, h, WICBitmapInterpolationModeFant)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        let mut pixels = vec![0; w as usize * h as usize * 4];
        converter.CopyPixels(std::ptr::null(), w * 4, &mut pixels)?;
        let output = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&output, WICBitmapEncoderNoCache)?;
        let (mut frame, mut options) = (None, None);
        encoder.CreateNewFrame(&mut frame, &mut options)?;
        let frame = frame.ok_or_else(windows::core::Error::from_thread)?;
        frame.Initialize(options.as_ref())?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format)?;
        if format != GUID_WICPixelFormat32bppBGRA {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "PNG編碼器不支援透明色彩",
            ));
        }
        frame.WritePixels(h, w * 4, &pixels)?;
        frame.Commit()?;
        encoder.Commit()?;
        let mut stat = STATSTG::default();
        output.Stat(&mut stat, STATFLAG_NONAME)?;
        if stat.cbSize == 0 || stat.cbSize >= MAX_BYTES as u64 {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "壓縮結果仍超過500KB，請使用較簡單圖片",
            ));
        }
        let mut encoded = vec![0; stat.cbSize as usize];
        output.Seek(0, STREAM_SEEK_SET, None)?;
        let mut read = 0;
        output
            .Read(
                encoded.as_mut_ptr().cast(),
                encoded.len() as u32,
                Some(&mut read),
            )
            .ok()?;
        if read as usize != encoded.len() {
            return Err(windows::core::Error::from_thread());
        }
        Ok((
            encoded,
            Image {
                width: w as usize,
                height: h as usize,
                bgra: pixels,
            },
        ))
    };
    convert().map_err(|e| e.to_string())
}
pub fn data_url(bytes: &[u8]) -> AppResult<String> {
    use windows_sys::Win32::Security::Cryptography::*;
    let mut encoded = vec![0u8; bytes.len().div_ceil(3) * 4 + 1];
    let mut size = encoded.len() as u32;
    if unsafe {
        CryptBinaryToStringA(
            bytes.as_ptr(),
            bytes.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            encoded.as_mut_ptr(),
            &mut size,
        )
    } == 0
    {
        return Err("PNG預覽編碼失敗。".into());
    }
    Ok(format!(
        "data:image/png;base64,{}",
        std::str::from_utf8(&encoded[..size as usize]).map_err(|e| e.to_string())?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_reencoding_strips_padding_preserves_alpha_and_limits_size() {
        let _apartment = crate::com_apartment::Apartment::sta().unwrap();
        let mut source = include_bytes!("../../ui/pets/cat_idle.png").to_vec();
        source.extend(vec![0; 600_000]);
        let (encoded, decoded) = normalize(&source).unwrap();
        assert!(source.len() > MAX_BYTES);
        assert!(encoded.len() < MAX_BYTES);
        assert!(decoded.width <= 256 && decoded.height <= 256);
        assert!(decoded.bgra.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        assert!(decoded
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 255));
        let (_, again) = normalize(&encoded).unwrap();
        assert_eq!(decoded.bgra, again.bgra);
        assert!(data_url(&encoded)
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR"));
        assert!(normalize(b"GIF89a").is_err());
        assert!(normalize(b"\x89PNG\r\n\x1a\ninvalid").is_err());
    }
}
