//! Telegram Bot API 최소 클라이언트 — getUpdates(롱폴링)·sendMessage.
//! HTTPS는 native-tls/Schannel(OpenSSL 불필요, gnu 빌드 OK). JSON 파싱·URL 인코딩은 순수 함수(테스트).
//!
//! ⚠️ 봇 토큰은 호출측(앱)이 keyring에서 가져와 전달한다 — 이 크레이트는 토큰을 저장/로그하지 않는다.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const HOST: &str = "api.telegram.org";

/// 수신 메시지 한 건(채팅 ID + 텍스트). update_id는 offset 추적용.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgMessage {
    pub update_id: i64,
    pub chat_id: i64,
    pub text: String,
}

/// getUpdates: offset부터 timeout초 롱폴링. 텍스트 메시지 목록과 다음 offset을 돌려준다.
pub fn get_updates(token: &str, offset: i64, timeout_secs: u64) -> Result<(Vec<TgMessage>, i64), String> {
    let path = format!("/bot{token}/getUpdates?timeout={timeout_secs}&offset={offset}&allowed_updates=%5B%22message%22%5D");
    let body = https_get(&path, timeout_secs + 10)?;
    Ok(parse_updates(&body, offset))
}

/// sendMessage: chat_id에 text를 보낸다(URL 인코딩). 성공 여부만.
pub fn send_message(token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    let path = format!("/bot{token}/sendMessage?chat_id={chat_id}&text={}", url_encode(text));
    let body = https_get(&path, 30)?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(())
    } else {
        Err(v["description"].as_str().unwrap_or("sendMessage 실패").to_string())
    }
}

/// getUpdates 응답 JSON을 파싱한다 — (텍스트 메시지들, 다음 offset). 파싱 실패/ok=false면 빈 목록+기존 offset.
pub fn parse_updates(json: &str, cur_offset: i64) -> (Vec<TgMessage>, i64) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return (Vec::new(), cur_offset) };
    if !v["ok"].as_bool().unwrap_or(false) {
        return (Vec::new(), cur_offset);
    }
    let Some(arr) = v["result"].as_array() else { return (Vec::new(), cur_offset) };
    let mut msgs = Vec::new();
    let mut next = cur_offset;
    for u in arr {
        let Some(uid) = u["update_id"].as_i64() else { continue };
        next = next.max(uid + 1); // 처리한 update는 다음 폴링에서 제외.
        let m = &u["message"];
        if let (Some(chat_id), Some(text)) = (m["chat"]["id"].as_i64(), m["text"].as_str()) {
            msgs.push(TgMessage { update_id: uid, chat_id, text: text.to_string() });
        }
    }
    (msgs, next)
}

/// 브리지 명령(`/`접두 제거된 본문 파싱). 인자 오류·미상은 Help로.
#[derive(Debug, PartialEq, Eq)]
pub enum TgCmd {
    Panes,
    Use(u64),
    Cancel,
    Help,
}

/// `/` 제거된 명령 본문을 파싱한다. `use N`만 인자가 필요(잘못되면 Help).
pub fn parse_command(rest: &str) -> TgCmd {
    let mut it = rest.split_whitespace();
    match it.next() {
        Some("panes") => TgCmd::Panes,
        Some("use") => it.next().and_then(|n| n.parse::<u64>().ok()).map_or(TgCmd::Help, TgCmd::Use),
        Some("cancel") => TgCmd::Cancel,
        _ => TgCmd::Help,
    }
}

/// 쿼리 파라미터용 퍼센트 인코딩(비예약 문자 외 모두 %XX). 공백=%20.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// HTTPS GET → 본문 텍스트(chunked 디코드). read_timeout은 롱폴링보다 길게.
fn https_get(path: &str, read_timeout_secs: u64) -> Result<String, String> {
    let tcp = TcpStream::connect((HOST, 443)).map_err(|e| format!("TCP 연결 실패: {e}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(read_timeout_secs))).ok();
    let connector = native_tls::TlsConnector::new().map_err(|e| format!("TLS 초기화 실패: {e}"))?;
    let mut stream = connector.connect(HOST, tcp).map_err(|e| format!("TLS 연결 실패: {e}"))?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: nabiTerm\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| e.to_string())?;
    let end = response.windows(4).position(|w| w == b"\r\n\r\n").ok_or("HTTP 헤더 끝 없음")?;
    let header = String::from_utf8_lossy(&response[..end]);
    let status = header.lines().next().unwrap_or("");
    if !status.contains("200") {
        return Err(format!("HTTP 오류: {status}"));
    }
    let body = &response[end + 4..];
    let body = if header.to_lowercase().contains("transfer-encoding: chunked") { dechunk(body) } else { body.to_vec() };
    String::from_utf8(body).map_err(|e| format!("UTF-8 디코드 실패: {e}"))
}

/// chunked transfer-encoding 본문을 합친다.
fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(nl) = data.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(std::str::from_utf8(&data[..nl]).unwrap_or("").trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let start = nl + 2;
        let endc = (start + size).min(data.len());
        out.extend_from_slice(&data[start..endc]);
        data = &data[(endc + 2).min(data.len())..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{parse_updates, url_encode, TgMessage};

    #[test]
    fn parse_extracts_text_messages_and_offset() {
        let j = r#"{"ok":true,"result":[
            {"update_id":10,"message":{"chat":{"id":42},"text":"hello"}},
            {"update_id":11,"message":{"chat":{"id":42},"text":"ls -la"}},
            {"update_id":12,"edited_message":{"chat":{"id":42},"text":"ignored"}}
        ]}"#;
        let (msgs, next) = parse_updates(j, 0);
        assert_eq!(msgs, vec![
            TgMessage { update_id: 10, chat_id: 42, text: "hello".into() },
            TgMessage { update_id: 11, chat_id: 42, text: "ls -la".into() },
        ]); // 텍스트 메시지만(편집 메시지는 message가 아니라 제외).
        assert_eq!(next, 13); // 최대 update_id+1.
    }

    #[test]
    fn parse_handles_errors_and_empty() {
        assert_eq!(parse_updates("not json", 5), (Vec::new(), 5)); // 파싱 실패 → offset 유지.
        assert_eq!(parse_updates(r#"{"ok":false,"description":"x"}"#, 5), (Vec::new(), 5));
        assert_eq!(parse_updates(r#"{"ok":true,"result":[]}"#, 5), (Vec::new(), 5));
    }

    #[test]
    fn parse_command_cases() {
        use super::{parse_command, TgCmd};
        assert_eq!(parse_command("panes"), TgCmd::Panes);
        assert_eq!(parse_command("use 42"), TgCmd::Use(42));
        assert_eq!(parse_command("use"), TgCmd::Help); // 인자 없음.
        assert_eq!(parse_command("use abc"), TgCmd::Help); // 잘못된 인자.
        assert_eq!(parse_command("cancel"), TgCmd::Cancel);
        assert_eq!(parse_command("zzz"), TgCmd::Help); // 미상.
    }

    #[test]
    fn url_encode_escapes() {
        assert_eq!(url_encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(url_encode("ls -la"), "ls%20-la"); // '-'는 비예약.
        assert_eq!(url_encode("한"), "%ED%95%9C"); // UTF-8 바이트별.
    }
}
