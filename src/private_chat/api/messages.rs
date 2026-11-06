//! 訊息依伺服器契約傳送。一次取一筆，讓每次回應有明確的記憶體上限。
use super::{Api, Error};
use crate::private_chat::{
    crypto::Secret,
    messages::{self, History, Incoming, Kind, Payload, Receipt, State, MAX_IMAGE},
    valid_device, Vault,
};
use crate::AppResult;
use serde::Deserialize;
use serde_json::json;
use std::ptr;
use windows_sys::Win32::Security::Cryptography::*;

#[derive(Default)]
pub(crate) struct Status {
    pub(crate) pending_messages: u32,
    pub(crate) pending_invites: u32,
    pub(crate) receipts: Vec<Receipt>,
}

#[derive(Deserialize)]
pub(crate) struct IncomingMessage {
    pub(crate) server_message_id: String,
    pub(crate) from_friend_id: String,
    pub(crate) server_time: String,
    kind: String,
    original_kind: Option<String>,
    text: Option<String>,
    image_base64: Option<String>,
    mime: Option<String>,
}

impl Drop for IncomingMessage {
    fn drop(&mut self) {
        if let Some(text) = &mut self.text {
            crate::private_chat::clear_text(text);
        }
        if let Some(image) = &mut self.image_base64 {
            crate::private_chat::clear_text(image);
        }
    }
}

impl IncomingMessage {
    /// 完整處理並保存後才取得 ack 許可。伺服器重播會回傳許可，但不顯示第二筆。
    pub(crate) fn persist(
        mut self,
        history: &mut History,
        vault: &Vault,
        blocked: bool,
    ) -> AppResult<(ReadyAck, Option<Payload>, bool)> {
        let (kind, body) = if blocked {
            (Kind::Text, None)
        } else {
            self.content()?
        };
        let added = history.receive(
            vault,
            Incoming {
                server_id: &self.server_message_id,
                peer: &self.from_friend_id,
                time: &self.server_time,
                kind,
                body: body.as_ref(),
                blocked,
            },
        )?;
        Ok((
            ReadyAck(std::mem::take(&mut self.server_message_id)),
            body,
            added,
        ))
    }

    fn content(&mut self) -> AppResult<(Kind, Option<Payload>)> {
        match self.kind.as_str() {
            "text" => Ok((
                Kind::Text,
                Some(messages::text_payload(
                    self.text.take().unwrap_or_default(),
                )?),
            )),
            "image" => {
                let format = match self.mime.as_deref() {
                    Some("image/jpeg") => "jpeg",
                    Some("image/png") => "png",
                    _ => return Err("私聊圖片格式不支援。".into()),
                };
                let encoded = Secret(self.image_base64.take().unwrap_or_default().into_bytes());
                let decoded = decode_image(&encoded.0)?;
                let converted = crate::projects::vision::jpeg::convert_private(&decoded.0, format)?;
                Ok((Kind::Image, Some(Payload::Image(converted.bytes))))
            }
            "expired" => match self.original_kind.as_deref() {
                Some("text") => Ok((Kind::ExpiredText, None)),
                Some("image") => Ok((Kind::ExpiredImage, None)),
                _ => Err("過期私聊訊息的原格式不正確。".into()),
            },
            _ => Err("不支援的私聊訊息種類。".into()),
        }
    }
}

pub(crate) struct ReadyAck(String);

#[derive(Deserialize)]
pub(crate) struct Sent {
    pub(crate) server_message_id: String,
    pub(crate) server_time: String,
}

impl Api {
    pub(crate) fn status(&self) -> Result<Status, Error> {
        #[derive(Deserialize)]
        struct WireStatus {
            pending_messages: u32,
            pending_invites: u32,
            #[serde(default)]
            receipts: Vec<WireReceipt>,
        }
        #[derive(Deserialize)]
        struct WireReceipt {
            client_message_id: String,
            state: String,
        }
        let wire: WireStatus = self.read("GET", "status", None)?;
        let receipts = wire
            .receipts
            .into_iter()
            .filter_map(|receipt| {
                if !valid_device(&receipt.client_message_id) {
                    return None;
                }
                let state = match receipt.state.as_str() {
                    "delivered" => State::Delivered,
                    "expired" => State::Expired,
                    _ => return None,
                };
                Some(Receipt {
                    client_message_id: receipt.client_message_id,
                    state,
                })
            })
            .collect();
        Ok(Status {
            pending_messages: wire.pending_messages,
            pending_invites: wire.pending_invites,
            receipts,
        })
    }

    pub(crate) fn send_message(
        &self,
        peer: &str,
        id: &str,
        payload: &Payload,
    ) -> Result<Sent, Error> {
        crate::jobs::validate_id(peer)?;
        if !valid_device(id) {
            return Err("私聊訊息識別格式不正確。".to_string().into());
        }
        let body = match payload {
            Payload::Text(text) => {
                json!({"to_friend_id":peer,"client_message_id":id,"kind":"text","text":text,"encryption":null})
            }
            Payload::Image(bytes) => {
                json!({"to_friend_id":peer,"client_message_id":id,"kind":"image",
                "image_base64":encode_image(bytes)?,"mime":"image/jpeg","encryption":null})
            }
        };
        let raw = self.call("POST", "messages", None, Some(body))?;
        let reply: Sent =
            serde_json::from_slice(&raw.0).map_err(|_| "私聊送出回應格式不正確。".to_string())?;
        crate::jobs::validate_id(&reply.server_message_id)?;
        messages::server_time(&reply.server_time)?;
        Ok(reply)
    }

    pub(crate) fn incoming_message(&self) -> Result<Option<IncomingMessage>, Error> {
        let mut messages: Vec<IncomingMessage> =
            self.read("GET", "messages", Some(("limit", "1")))?;
        if messages.len() > 1 {
            return Err("私聊回應超過本次取件上限。".to_string().into());
        }
        Ok(messages.pop())
    }

    pub(crate) fn ack_message(&self, receipt: ReadyAck) -> Result<(), Error> {
        self.call(
            "POST",
            "messages/ack",
            None,
            Some(json!({"server_message_ids":[receipt.0]})),
        )?;
        Ok(())
    }
}

pub(crate) fn encode_image(bytes: &[u8]) -> AppResult<String> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE {
        return Err("私聊圖片不得超過 10 MB。".into());
    }
    let mut encoded = Secret(vec![0; bytes.len().div_ceil(3) * 4 + 1]);
    let mut size = encoded.0.len() as u32;
    // SAFETY: 輸入／輸出使用各自固定長度緩衝區。
    if unsafe {
        CryptBinaryToStringA(
            bytes.as_ptr(),
            bytes.len() as u32,
            CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF,
            encoded.0.as_mut_ptr(),
            &mut size,
        )
    } == 0
    {
        return Err("無法編碼私聊圖片。".into());
    }
    let text =
        std::str::from_utf8(&encoded.0[..size as usize]).map_err(|_| "私聊圖片編碼格式不正確。")?;
    Ok(text.trim_end_matches('\0').into())
}

fn decode_image(encoded: &[u8]) -> AppResult<Secret<Vec<u8>>> {
    if encoded.is_empty()
        || encoded.len() > MAX_IMAGE.div_ceil(3) * 4
        || !encoded.len().is_multiple_of(4)
    {
        return Err("私聊圖片不得超過 10 MB。".into());
    }
    let mut size = 0;
    let flags = CRYPT_STRING_BASE64 | CRYPT_STRING_STRICT;
    // SAFETY: 先查詢實際解碼長度，確認大小後才配置。
    if unsafe {
        CryptStringToBinaryA(
            encoded.as_ptr(),
            encoded.len() as u32,
            flags,
            ptr::null_mut(),
            &mut size,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    } == 0
        || !(1..=MAX_IMAGE as u32).contains(&size)
    {
        return Err("私聊圖片資料格式不正確或超過 10 MB。".into());
    }
    let mut bytes = Secret(vec![0; size as usize]);
    // SAFETY: 按上一次查詢的長度配置，輸入保持相同。
    if unsafe {
        CryptStringToBinaryA(
            encoded.as_ptr(),
            encoded.len() as u32,
            flags,
            bytes.0.as_mut_ptr(),
            &mut size,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    } == 0
    {
        return Err("無法解碼私聊圖片。".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_chat::Account;

    #[test]
    fn bounds_image_data_and_reencodes_without_exif() {
        assert!(decode_image(b"not image").is_err());
        assert!(decode_image(&vec![b'A'; MAX_IMAGE.div_ceil(3) * 4 + 4]).is_err());
        assert!(encode_image(&vec![0; MAX_IMAGE + 1]).is_err());
        let original = include_bytes!("../../../examples/fixtures/vision.jpg");
        // APP1 使用完整的小端 TIFF IFD，ImageDescription 有可搜尋的合成文字。
        let marker = b"FIXTURE_PRIVATE_EXIF\0";
        let mut app = b"Exif\0\0II\x2A\x00\x08\x00\x00\x00\x01\x00\x0E\x01\x02\x00".to_vec();
        app.extend_from_slice(&(marker.len() as u32).to_le_bytes());
        app.extend_from_slice(&26_u32.to_le_bytes());
        app.extend_from_slice(&[0; 4]);
        app.extend_from_slice(marker);
        let mut original_with_metadata = vec![0xFF, 0xD8, 0xFF, 0xE1];
        original_with_metadata.extend_from_slice(&((app.len() + 2) as u16).to_be_bytes());
        original_with_metadata.extend(app);
        original_with_metadata.extend_from_slice(&original[2..]);
        let jpeg = crate::projects::vision::jpeg::convert_private(&original_with_metadata, "jpeg")
            .unwrap();
        assert!(jpeg.bytes.starts_with(&[0xFF, 0xD8]));
        assert!(!jpeg
            .bytes
            .windows(marker.len())
            .any(|bytes| bytes == marker));
        assert!(crate::projects::vision::jpeg::convert_private(&jpeg.bytes, "png").is_err());
        assert_eq!(
            decode_image(encode_image(&jpeg.bytes).unwrap().as_bytes())
                .unwrap()
                .0,
            jpeg.bytes
        );
        let png = include_bytes!("../../../examples/fixtures/vision.png");
        let converted = crate::projects::vision::jpeg::convert_private(png, "png").unwrap();
        assert!(converted.bytes.starts_with(&[0xFF, 0xD8]));
        assert!(converted.width <= 4096 && converted.height <= 4096);

        // 寬幅剪貼簿等同一張未壓縮 BMP。用 5,000 像素觸發實際 WIC 縮放。
        let mut wide_bmp = vec![0; 54 + 30_000];
        let size = wide_bmp.len() as u32;
        wide_bmp[..2].copy_from_slice(b"BM");
        wide_bmp[2..6].copy_from_slice(&size.to_le_bytes());
        wide_bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        wide_bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        wide_bmp[18..22].copy_from_slice(&5_000_i32.to_le_bytes());
        wide_bmp[22..26].copy_from_slice(&2_i32.to_le_bytes());
        wide_bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        wide_bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        wide_bmp[34..38].copy_from_slice(&30_000_u32.to_le_bytes());
        let scaled = crate::projects::vision::jpeg::convert_private(&wide_bmp, "bmp").unwrap();
        assert_eq!((scaled.width, scaled.height), (4096, 1));
    }

    #[test]
    #[ignore = "需要 scripts/Test-PrivateChat.py --messages 啟動本機 TLS"]
    fn tls_messages_fixture() {
        use crate::transport::pins::Pins;
        let get = |name| std::env::var(name).expect("本機訊息 TLS fixture 參數");
        let base = url::Url::parse(&get("LMAI_CHAT_FIXTURE_URL")).unwrap();
        let pins = Pins::parse(
            &get("LMAI_CHAT_FIXTURE_PRIMARY"),
            &get("LMAI_CHAT_FIXTURE_BACKUP"),
        )
        .unwrap();
        let a = Api::fixture(base.clone(), pins.clone());
        let b = Api::fixture_account(
            base,
            pins,
            "fixture-private-recipient",
            "48fc54f7-ea9f-4d55-a445-bdfd660cc040",
        );
        a.activate().unwrap();
        b.activate().unwrap();
        let root = std::path::PathBuf::from(get("LMAI_CHAT_FIXTURE_DATA"));
        let alice = Account::new(&root, "fixture", "fixture-alice").unwrap();
        let bob = Account::new(&root, "fixture", "fixture-bob").unwrap();
        let a_vault = alice
            .create("Test123", &a.key_secret(false).unwrap())
            .unwrap();
        let b_vault = bob
            .create("Test234", &b.key_secret(false).unwrap())
            .unwrap();
        let mut a_history = History::load(&a_vault).unwrap();
        let mut b_history = History::load(&b_vault).unwrap();
        let body = messages::text_payload("Hello <script>not active</script>".into()).unwrap();
        let id = a_history
            .append_outgoing(&a_vault, "friend-bob", &body)
            .unwrap();
        let sent = a.send_message("friend-bob", &id, &body).unwrap();
        assert_eq!(
            a.send_message("friend-bob", &id, &body)
                .unwrap()
                .server_message_id,
            sent.server_message_id
        );
        a_history
            .mark_sent(&a_vault, &id, &sent.server_time)
            .unwrap();
        // 上鎖只查 status。收件落盤故障時，沒有任何可用的 ack 許可。
        assert_eq!(b.status().unwrap().pending_messages, 1);
        let broken_index = bob.directory.join("messages.gcm");
        std::fs::create_dir(&broken_index).unwrap();
        let pending = b.incoming_message().unwrap().unwrap();
        assert!(pending.persist(&mut b_history, &b_vault, false).is_err());
        assert_eq!(b.status().unwrap().pending_messages, 1);
        assert!(a.status().unwrap().receipts.is_empty());
        std::fs::remove_dir(&broken_index).unwrap();
        let received = b.incoming_message().unwrap().unwrap();
        let (ack, _, added) = received.persist(&mut b_history, &b_vault, false).unwrap();
        assert!(added);
        let redelivery = b.incoming_message().unwrap().unwrap();
        assert!(
            !redelivery
                .persist(&mut b_history, &b_vault, false)
                .unwrap()
                .2
        );
        b.ack_message(ack).unwrap();
        let receipts = a.status().unwrap().receipts;
        assert_eq!(receipts.len(), 1);
        alice.stage_receipts(&receipts).unwrap();
        assert!(a.status().unwrap().receipts.is_empty());
        a_history
            .apply_receipts(&a_vault, &alice.read_receipts().unwrap())
            .unwrap();
        alice.clear_receipts().unwrap();
        assert_eq!(a_history.record(&id).unwrap().state, State::Delivered);
        let received_id = b_history.for_peer("friend-alice")[0].id.clone();
        b_history.remove(&b_vault, &received_id).unwrap();
        assert!(b_history
            .record(&received_id)
            .unwrap()
            .payload(&b_vault)
            .is_err());
        assert!(a_history.record(&id).unwrap().payload(&a_vault).is_ok());

        let input = include_bytes!("../../../examples/fixtures/vision.png");
        let jpeg = crate::projects::vision::jpeg::convert_private(input, "png").unwrap();
        let image = Payload::Image(jpeg.bytes);
        a.send_message("friend-bob", &crate::private_chat::uuid().unwrap(), &image)
            .unwrap();
        let (ack, payload, added) = b
            .incoming_message()
            .unwrap()
            .unwrap()
            .persist(&mut b_history, &b_vault, false)
            .unwrap();
        assert!(added && matches!(payload, Some(Payload::Image(_))));
        b.ack_message(ack).unwrap();

        for (scenario, description) in [
            ("paused-fixture", "超過 72 小時"),
            ("queue-full-fixture", "已達上限"),
            ("disabled-fixture", "帳號已停用"),
        ] {
            let body = messages::text_payload(scenario.into()).unwrap();
            let error = a
                .send_message("friend-bob", &crate::private_chat::uuid().unwrap(), &body)
                .err()
                .unwrap();
            assert!(error.message.contains(description));
        }
        let expired = messages::text_payload("expire-fixture".into()).unwrap();
        let expired_id = a_history
            .append_outgoing(&a_vault, "friend-bob", &expired)
            .unwrap();
        let sent = a.send_message("friend-bob", &expired_id, &expired).unwrap();
        a_history
            .mark_sent(&a_vault, &expired_id, &sent.server_time)
            .unwrap();
        a_history
            .apply_receipts(&a_vault, &a.status().unwrap().receipts)
            .unwrap();
        assert_eq!(a_history.record(&expired_id).unwrap().state, State::Expired);
        let (ack, payload, added) = b
            .incoming_message()
            .unwrap()
            .unwrap()
            .persist(&mut b_history, &b_vault, false)
            .unwrap();
        assert!(added && payload.is_none());
        assert!(b_history
            .for_peer("friend-alice")
            .iter()
            .any(|entry| entry.kind == Kind::ExpiredText && entry.time.is_some()));
        b.ack_message(ack).unwrap();
        let blocked = messages::text_payload("blocked-fixture".into()).unwrap();
        a.send_message(
            "friend-bob",
            &crate::private_chat::uuid().unwrap(),
            &blocked,
        )
        .unwrap();
        let (ack, payload, added) = b
            .incoming_message()
            .unwrap()
            .unwrap()
            .persist(&mut b_history, &b_vault, true)
            .unwrap();
        assert!(!added && payload.is_none());
        b.ack_message(ack).unwrap();
        assert_eq!(b.status().unwrap().pending_messages, 0);
    }
}
