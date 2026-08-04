//! HTTP 응답 파싱 순수 함수 — chunked 해제·URL 분해·상태코드/헤더 추출.
//! 소켓을 만지지 않으므로 단위 테스트가 붙어 있다. 연결·요청 흐름은 net.rs.
/// chunked transfer-encoding 본문 디코드(크기 줄 + 데이터 반복).
pub(super) fn dechunk(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let Some(nl) = data[i..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_str = String::from_utf8_lossy(&data[i..i + nl]);
        let size = usize::from_str_radix(size_str.trim().split(';').next().unwrap_or("0"), 16)
            .unwrap_or(0);
        i += nl + 2;
        if size == 0 {
            break;
        }
        if i + size > data.len() {
            out.extend_from_slice(&data[i..]);
            break;
        }
        out.extend_from_slice(&data[i..i + size]);
        i += size + 2; // 데이터 + 뒤따르는 \r\n
    }
    out
}

/// URL → (host, path, port).
pub(super) fn parse_url(url: &str) -> Result<(String, String, u16), String> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("HTTPS URL만 지원")?;
    let (host_port, path) = match url.find('/') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, "/"),
    };
    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse().unwrap_or(443)),
        None => (host_port, 443u16),
    };
    Ok((host.to_string(), path.to_string(), port))
}

pub(super) fn parse_status_code(header: &str) -> Result<u16, String> {
    header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| "HTTP 상태 파싱 실패".to_string())
}

pub(super) fn find_header(headers: &str, name: &str) -> Option<String> {
    let want = name.to_lowercase();
    headers.lines().find_map(|line| {
        line.split_once(':').and_then(|(k, v)| {
            (k.trim().to_lowercase() == want).then(|| v.trim().to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{dechunk, parse_url};

    #[test]
    fn dechunk_basic() {
        // "4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n" → "Wikipedia"
        let raw = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        assert_eq!(dechunk(raw), b"Wikipedia");
    }

    #[test]
    fn url_parsing() {
        let (h, p, port) = parse_url("https://github.com/a/b.exe").unwrap();
        assert_eq!((h.as_str(), p.as_str(), port), ("github.com", "/a/b.exe", 443));
        assert!(parse_url("ftp://x").is_err());
    }
}
