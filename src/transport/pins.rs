//! 公司 HTTPS v2 在 WinHTTP 送出任何 HTTP 資料前，比對 TLS 憑證的 SPKI SHA-256。
use super::{network_error, Handle};
use crate::{config, AppResult};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicU8, Ordering},
};
use url::Url;
use windows_sys::Win32::{Networking::WinHttp::*, Security::Cryptography::*};

const WAITING: u8 = 0;
const VERIFIED: u8 = 1;
const REJECTED: u8 = 2;
pub(super) const PIN_ERROR: &str = "HTTPS v2 伺服器憑證不符，已停止連線（TLS_PIN_MISMATCH）。";

#[derive(Clone)]
pub(crate) struct Pins {
    digests: [[u8; 32]; 2],
}

impl Pins {
    pub(crate) fn parse(primary: &str, backup: &str) -> AppResult<Self> {
        fn digest(value: &str) -> AppResult<[u8; 32]> {
            if value.len() != 44
                || !value.ends_with('=')
                || !value[..43]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
            {
                return Err("HTTPS v2 公鑰雜湊必須是 32 位元組的 Base64。".into());
            }
            let mut bytes = [0; 32];
            let mut size = bytes.len() as u32;
            let decoded = unsafe {
                CryptStringToBinaryA(
                    value.as_ptr(),
                    value.len() as u32,
                    CRYPT_STRING_BASE64 | CRYPT_STRING_STRICT,
                    bytes.as_mut_ptr(),
                    &mut size,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            if decoded == 0 || size != 32 {
                return Err("HTTPS v2 公鑰雜湊格式無效。".into());
            }
            Ok(bytes)
        }
        let digests = [digest(primary)?, digest(backup)?];
        if digests[0] == digests[1] {
            return Err("HTTPS v2 正式與備用公鑰必須不同。".into());
        }
        Ok(Self { digests })
    }
}

pub(crate) fn configured() -> AppResult<Option<Pins>> {
    if [config::PRIMARY_SPKI, config::BACKUP_SPKI]
        .iter()
        .any(|value| value.starts_with("LMAI-PIN-PLACEHOLDER-"))
    {
        return Ok(None);
    }
    Pins::parse(config::PRIMARY_SPKI, config::BACKUP_SPKI).map(Some)
}

pub(super) fn for_url(url: &Url) -> AppResult<Option<Pins>> {
    if Url::parse(config::SERVER_URL_V2).is_ok_and(|company| company.origin() == url.origin()) {
        return configured()?
            .map(Some)
            .ok_or_else(|| "HTTPS v2 未啟用：尚未設定正式公鑰。".into());
    }
    Ok(None)
}

/// 只略過未知 CA，保留 Windows 的主機名稱、到期日及其餘安全檢查。
pub(super) fn set_security(request: *mut c_void) -> AppResult<()> {
    let flags = SECURITY_FLAG_IGNORE_UNKNOWN_CA;
    if unsafe {
        WinHttpSetOption(
            request,
            WINHTTP_OPTION_SECURITY_FLAGS,
            (&flags as *const u32).cast(),
            4,
        )
    } == 0
    {
        return Err(network_error("WinHttpSetOption/pinned-ca"));
    }
    Ok(())
}

pub(super) struct Check {
    pins: Pins,
    result: AtomicU8,
}

impl Check {
    pub(super) fn new(pins: Pins) -> Self {
        Self {
            pins,
            result: AtomicU8::new(WAITING),
        }
    }

    /// 回呼在 WinHTTP 的資訊階段執行。回傳 false 時呼叫端必須關閉請求。
    pub(super) fn before_send(&self, request: *mut c_void) -> bool {
        if matches_certificate(request, &self.pins) {
            self.result.store(VERIFIED, Ordering::Release);
            return true;
        }
        self.result.store(REJECTED, Ordering::Release);
        false
    }

    pub(super) fn rejected(&self) -> bool {
        self.result.load(Ordering::Acquire) == REJECTED
    }

    pub(super) fn ensure_verified(&self) -> AppResult<()> {
        match self.result.load(Ordering::Acquire) {
            VERIFIED => Ok(()),
            REJECTED => Err(PIN_ERROR.into()),
            _ => Err("HTTPS v2 未取得憑證驗證結果，已停止連線（TLS_PIN_UNVERIFIED）。".into()),
        }
    }
}

fn matches_certificate(request: *mut c_void, pins: &Pins) -> bool {
    let mut certificate: *const CERT_CONTEXT = ptr::null();
    let mut length = std::mem::size_of_val(&certificate) as u32;
    // SAFETY: WinHTTP 取得獨立的憑證參照，由本函式釋放；結構只在參照有效時使用。
    let queried = unsafe {
        WinHttpQueryOption(
            request,
            WINHTTP_OPTION_SERVER_CERT_CONTEXT,
            (&mut certificate as *mut *const CERT_CONTEXT).cast(),
            &mut length,
        )
    };
    if queried == 0 || certificate.is_null() {
        return false;
    }
    let mut der = [0; 8192];
    let mut der_length = der.len() as u32;
    let encoded = unsafe {
        let info = (*certificate).pCertInfo;
        let result = !info.is_null()
            && CryptEncodeObjectEx(
                X509_ASN_ENCODING,
                X509_PUBLIC_KEY_INFO,
                (&(*info).SubjectPublicKeyInfo as *const CERT_PUBLIC_KEY_INFO).cast(),
                0,
                ptr::null(),
                der.as_mut_ptr().cast(),
                &mut der_length,
            ) != 0;
        CertFreeCertificateContext(certificate);
        result
    };
    encoded
        && der.get(..der_length as usize).is_some_and(|bytes| {
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            pins.digests.contains(&digest)
        })
}

unsafe extern "system" fn check_sync(
    request: *mut c_void,
    context: usize,
    status: u32,
    _: *mut c_void,
    _: u32,
) {
    if status == WINHTTP_CALLBACK_STATUS_SENDING_REQUEST && context != 0 {
        // SAFETY: 請求擁有 Box<Check>，它在關閉請求後才釋放。
        let check = unsafe { &*(context as *const Check) };
        if !check.rejected() && !check.before_send(request) {
            unsafe { WinHttpCloseHandle(request) };
        }
    }
}

pub(super) fn send(
    request: &mut Handle,
    url: &Url,
    headers: &[u16],
    total: u32,
    test_pins: Option<&Pins>,
) -> AppResult<()> {
    if test_pins.is_some()
        && (!cfg!(test) || url.scheme() != "https" || url.host_str() != Some("localhost"))
    {
        return Err("測試公鑰只供本機 TLS 驗收使用。".into());
    }
    let production = for_url(url)?;
    let pin_set = production.or_else(|| test_pins.cloned());
    request.1 = pin_set.map(|pins| Box::new(Check::new(pins)));
    if request.1.is_some() {
        set_security(request.0)?;
        let previous = unsafe {
            WinHttpSetStatusCallback(
                request.0,
                Some(check_sync),
                WINHTTP_CALLBACK_STATUS_SENDING_REQUEST,
                0,
            )
        };
        // WinHTTP 以 -1 指標回報無法安裝回呼；沒有前一個回呼時傳回 NULL。
        if previous.is_some_and(|callback| callback as usize == usize::MAX) {
            return Err(network_error("WinHttpSetStatusCallback/pin"));
        }
    }
    let context = request
        .1
        .as_deref()
        .map_or(0, |check| check as *const Check as usize);
    let sent = unsafe {
        WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            ptr::null(),
            0,
            total,
            context,
        )
    };
    if request.1.as_deref().is_some_and(Check::rejected) {
        request.0 = ptr::null_mut();
        return Err(PIN_ERROR.into());
    }
    if sent == 0 {
        return Err(network_error("WinHttpSendRequest"));
    }
    if let Some(check) = request.1.as_deref() {
        check.ensure_verified()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn company_placeholders_are_separate_from_pin_format_validation() {
        let a = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        let b = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAE=";
        assert!(Pins::parse(a, b).is_ok());
        assert!(Pins::parse(a, a).is_err());
        assert!(Pins::parse(config::PRIMARY_SPKI, "invalid").is_err());
        if configured().unwrap().is_none() {
            assert!(config::Config::default().v2_candidate().unwrap().is_none());
            assert!(for_url(&Url::parse(config::SERVER_URL_V2).unwrap()).is_err());
        }
    }

    /// Test-HttpsPins.py 產生獨立自簽憑證，並從伺服器端核對有沒有收到 Header／本文。
    #[test]
    #[ignore = "需要 scripts/Test-HttpsPins.py 啟動 loopback TLS fixture"]
    fn tls_pin_loopback_fixture() {
        use super::super::{exchange_inner, Payload};
        let get = |name| std::env::var(name).expect("本機 TLS fixture 參數");
        let url = Url::parse(&get("LMAI_TLS_FIXTURE_URL")).unwrap();
        let pins = Pins::parse(
            &get("LMAI_TLS_FIXTURE_PRIMARY"),
            &get("LMAI_TLS_FIXTURE_BACKUP"),
        )
        .unwrap();
        if get("LMAI_TLS_FIXTURE_EXPECTED") == "signed_update" {
            use super::super::download_file_inner;
            let reply = exchange_inner(
                &url,
                "GET",
                "application/json",
                Payload {
                    reader: &mut std::io::empty(),
                    length: 0,
                },
                None,
                5_000,
                None,
                "application/json",
                Some(&pins),
            )
            .unwrap();
            assert_eq!(reply.status, 200);
            let manifest: crate::deployment::UpdateArtifact =
                serde_json::from_str(&reply.body).unwrap();
            let package_url = url.join(&manifest.url).unwrap();
            assert_eq!(package_url.origin(), url.origin());
            let output = std::path::PathBuf::from(get("LMAI_TLS_FIXTURE_OUTPUT"));
            let package_path = output.join("received-setup.exe");
            download_file_inner(&package_url, &package_path, manifest.size, Some(&pins)).unwrap();
            let config = config::Config {
                server_url: url.origin().ascii_serialization(),
                ..config::Config::default()
            };
            crate::deployment::verify_download_fixture(&config, &manifest, &package_path).unwrap();
            return;
        }
        let body = "fixture-payload";
        let mut reader = std::io::Cursor::new(body.as_bytes());
        let reply = exchange_inner(
            &url,
            "POST",
            "text/plain",
            Payload {
                reader: &mut reader,
                length: body.len() as u32,
            },
            Some(("Authorization", "Bearer fixture-placeholder")),
            5_000,
            None,
            "application/json",
            Some(&pins),
        );
        match get("LMAI_TLS_FIXTURE_EXPECTED").as_str() {
            "match" => assert_eq!(reply.unwrap().status, 200),
            "pin_error" => assert_eq!(reply.err().unwrap(), PIN_ERROR),
            "name_error" => assert!(reply.err().unwrap().contains("12175")),
            _ => panic!("未知 TLS fixture 驗收情境"),
        }
    }
}
