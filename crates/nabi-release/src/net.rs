//! 업데이트 네트워크 계층: GitHub 최신 릴리스 조회 + 인스톨러 다운로드
//! (native-tls/Schannel, 리다이렉트 추적, 진행률 보고). nabidrive 검증 구조 이식.

use super::{is_newer_version, DownloadProgress, ReleaseInfo, UpdateStatus, ASSET_PREFIX, CURRENT_VERSION, REPO_PATH};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 최신 릴리스를 조회한다. 새 버전이 있으면 Some(ReleaseInfo).
pub(super) fn check_github_release() -> Result<Option<ReleaseInfo>, String> {
    let body = https_get_text("api.github.com", REPO_PATH)?;
    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON 파싱 실패: {e}"))?;

    if release["draft"].as_bool().unwrap_or(false) {
        return Ok(None);
    }
    if release["prerelease"].as_bool().unwrap_or(false) {
        return Ok(None);
    }

    // tag: v0.1.2 → 0.1.2
    let version = release["tag_name"].as_str().unwrap_or("").trim_start_matches('v');
    if version.is_empty() || !is_newer_version(version, CURRENT_VERSION) {
        return Ok(None);
    }

    let download_url = release["assets"]
        .as_array()
        .and_then(|a| {
            a.iter().find(|a| {
                let name = a["name"].as_str().unwrap_or("");
                name.starts_with(ASSET_PREFIX) && name.ends_with(".exe")
            })
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .unwrap_or("")
        .to_string();

    if download_url.is_empty() {
        return Err(format!("릴리스에 {ASSET_PREFIX}*.exe 자산이 없습니다"));
    }

    Ok(Some(ReleaseInfo {
        version: version.to_string(),
        download_url,
        notes: release["body"].as_str().unwrap_or("").to_string(),
    }))
}

/// 인스톨러를 %LOCALAPPDATA%\nabi\update 아래로 내려받고 경로를 돌려준다.
pub(super) fn download_installer(
    url: &str,
    status: &Arc<Mutex<UpdateStatus>>,
) -> Result<String, String> {
    if url.is_empty() {
        return Err("다운로드 URL 없음".into());
    }
    let dir = std::env::var("LOCALAPPDATA")
        .map(|d| format!("{d}\\nabi\\update"))
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    std::fs::create_dir_all(&dir).ok();
    let dest = format!("{dir}\\nabiTerm-setup-update.exe");
    https_download_file(url, &dest, status)?;
    Ok(dest)
}

/// 범용 HTTPS GET → 텍스트(폰트 설치기의 GitHub 릴리스 조회용).
pub(super) fn get_text(host: &str, path: &str) -> Result<String, String> {
    https_get_text(host, path)
}

/// 범용 HTTPS 파일 다운로드(진행률 미보고 — 폰트 등 소형 파일용, 리다이렉트 추적).
pub(super) fn download_plain(url: &str, dest: &str) -> Result<(), String> {
    https_download_file(url, dest, &Arc::new(Mutex::new(UpdateStatus::Idle)))
}

/// TLS 스트림 생성(native-tls = Windows Schannel).
fn tls_connect(host: &str, port: u16) -> Result<native_tls::TlsStream<TcpStream>, String> {
    let tcp = TcpStream::connect((host, port)).map_err(|e| format!("TCP 연결 실패: {e}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let connector = native_tls::TlsConnector::new().map_err(|e| format!("TLS 초기화 실패: {e}"))?;
    connector.connect(host, tcp).map_err(|e| format!("TLS 연결 실패: {e}"))
}

/// HTTPS GET → 텍스트(GitHub API용, 리다이렉트 미지원, chunked 디코드).
fn https_get_text(host: &str, path: &str) -> Result<String, String> {
    let mut stream = tls_connect(host, 443)?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: nabiTerm/{CURRENT_VERSION}\r\n\
         Accept: application/vnd.github+json\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| e.to_string())?;

    let header_end = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("HTTP 헤더 끝 없음")?;
    let header = String::from_utf8_lossy(&response[..header_end]);
    let status_line = header.lines().next().unwrap_or("");
    if !status_line.contains("200") {
        return Err(format!("HTTP 오류: {status_line}"));
    }

    let body = &response[header_end + 4..];
    let body = if header.to_lowercase().contains("transfer-encoding: chunked") {
        dechunk(body)
    } else {
        body.to_vec()
    };
    String::from_utf8(body).map_err(|e| format!("UTF-8 디코드 실패: {e}"))
}

/// HTTPS 바이너리 다운로드(리다이렉트 추적 + 진행률 보고).
fn https_download_file(
    initial_url: &str,
    dest: &str,
    status: &Arc<Mutex<UpdateStatus>>,
) -> Result<(), String> {
    let mut url = initial_url.to_string();
    for _ in 0..5 {
        let (host, path, port) = parse_url(&url)?;
        let mut stream = tls_connect(&host, port)?;
        stream.get_ref().set_read_timeout(Some(Duration::from_secs(60))).ok();
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: nabiTerm/{CURRENT_VERSION}\r\n\
             Accept: */*\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

        let header = read_headers(&mut stream)?;
        let code = parse_status_code(&header)?;
        if matches!(code, 301 | 302 | 307 | 308) {
            url = match find_header(&header, "location") {
                Some(loc) if loc.starts_with("http") => loc,
                Some(loc) => format!("https://{host}{loc}"),
                None => return Err("리다이렉트 Location 없음".into()),
            };
            continue;
        }
        if code != 200 {
            return Err(format!("HTTP {code}"));
        }

        let total = find_header(&header, "content-length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        write_body(&mut stream, dest, total, status)?;
        return Ok(());
    }
    Err("리다이렉트 5회 초과".into())
}

/// 응답 헤더(\r\n\r\n까지)를 바이트 단위로 읽는다.
fn read_headers(stream: &mut native_tls::TlsStream<TcpStream>) -> Result<String, String> {
    let mut buf = Vec::with_capacity(4096);
    let mut b = [0u8; 1];
    loop {
        stream.read_exact(&mut b).map_err(|e| format!("헤더 읽기 실패: {e}"))?;
        buf.push(b[0]);
        if buf.len() >= 4 && &buf[buf.len() - 4..] == b"\r\n\r\n" {
            return Ok(String::from_utf8_lossy(&buf).into_owned());
        }
        if buf.len() > 16384 {
            return Err("HTTP 헤더 과대".into());
        }
    }
}

/// 본문을 파일에 저장하며 진행률을 status에 보고한다.
fn write_body(
    stream: &mut native_tls::TlsStream<TcpStream>,
    dest: &str,
    total: u64,
    status: &Arc<Mutex<UpdateStatus>>,
) -> Result<(), String> {
    let mut file = std::fs::File::create(dest).map_err(|e| format!("파일 생성 실패: {e}"))?;
    let mut downloaded = 0u64;
    let mut buf = [0u8; 65536];
    let started = Instant::now();
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buf[..n]).map_err(|e| format!("파일 쓰기 실패: {e}"))?;
                downloaded += n as u64;
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = (downloaded as f64 / elapsed) as u64;
                if let Ok(mut s) = status.lock() {
                    *s = UpdateStatus::Downloading(DownloadProgress {
                        downloaded,
                        total,
                        speed_bps: speed,
                    });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(format!("다운로드 실패: {e}")),
        }
    }
    if total > 0 && downloaded < total {
        return Err(format!("불완전 다운로드: {downloaded}/{total} 바이트"));
    }
    Ok(())
}

/// chunked transfer-encoding 본문 디코드(크기 줄 + 데이터 반복).
fn dechunk(data: &[u8]) -> Vec<u8> {
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
fn parse_url(url: &str) -> Result<(String, String, u16), String> {
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

fn parse_status_code(header: &str) -> Result<u16, String> {
    header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| "HTTP 상태 파싱 실패".to_string())
}

fn find_header(headers: &str, name: &str) -> Option<String> {
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
