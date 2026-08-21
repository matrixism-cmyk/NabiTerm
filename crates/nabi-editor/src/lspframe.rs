//! LSP JSON-RPC 프레이밍(T6-4) — `Content-Length: N\r\n\r\n<본문>` 인코드/디코드(순수).

use std::io::BufRead;

/// 메시지 하나를 프레임으로 감싼다.
pub fn encode(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// 스트림에서 프레임 하나를 읽는다(블로킹). EOF/형식 오류면 None.
pub fn read_frame(r: &mut impl BufRead) -> Option<String> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None; // EOF.
        }
        let t = line.trim_end();
        if t.is_empty() {
            break; // 헤더 끝.
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
        // Content-Type 등 다른 헤더는 무시.
    }
    let n = len?;
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

/// 파일 경로 → file:// URI(Windows 드라이브 문자 처리).
pub fn path_to_uri(p: &std::path::Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let esc: String = s
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '?' => "%3F".to_string(),
            c => c.to_string(),
        })
        .collect();
    if esc.starts_with('/') {
        format!("file://{esc}")
    } else {
        format!("file:///{esc}")
    }
}

/// URI를 비교용 정규형으로 — 서버마다 드라이브 문자 대소문자·`%3A` 표기가 다르다
/// (rust-analyzer는 `file:///c:/...`로 소문자화해 보냄). 진단 맵 키는 항상 이걸 쓴다.
pub fn canon_uri(uri: &str) -> String {
    let s = uri.replace("%3A", ":").replace("%3a", ":");
    // `file:///X:/` 꼴이면 드라이브 문자만 소문자로(경로 본문은 보존).
    let b = s.as_bytes();
    if s.len() > 9 && s.starts_with("file:///") && b[9] == b':' && b[8].is_ascii_alphabetic() {
        let mut out = s.clone();
        // 안전: 8번째 바이트는 ASCII 알파벳임을 위에서 확인.
        // SAFETY: 바꾸는 바이트는 위에서 ASCII 알파벳임을 확인했고, 결과도 ASCII라 UTF-8 불변식이
        // 유지된다(멀티바이트 경계를 건드리지 않는다).
        unsafe { out.as_bytes_mut()[8] = b[8].to_ascii_lowercase() };
        return out;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn canonicalizes_drive_letter() {
        assert_eq!(canon_uri("file:///C:/a/b.rs"), "file:///c:/a/b.rs");
        assert_eq!(canon_uri("file:///c%3A/a/b.rs"), "file:///c:/a/b.rs");
        assert_eq!(canon_uri("file:///home/a.rs"), "file:///home/a.rs", "유닉스 경로 무변형");
    }

    #[test]
    fn frame_roundtrip() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"x"}"#;
        let mut r = BufReader::new(std::io::Cursor::new(encode(body)));
        assert_eq!(read_frame(&mut r).as_deref(), Some(body));
        assert!(read_frame(&mut r).is_none(), "EOF");
    }

    #[test]
    fn tolerates_extra_headers_and_utf8_len() {
        let body = "{\"m\":\"한글\"}"; // Content-Length는 바이트 수.
        let framed = format!("Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let mut r = BufReader::new(std::io::Cursor::new(framed.into_bytes()));
        assert_eq!(read_frame(&mut r).as_deref(), Some(body));
    }

    #[test]
    fn windows_path_to_uri() {
        let u = path_to_uri(std::path::Path::new(r"C:\proj\한 글\main.rs"));
        assert_eq!(u, "file:///C:/proj/한%20글/main.rs");
    }
}
