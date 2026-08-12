//! 업데이트 네트워크 계층: GitHub 최신 릴리스 조회 + 인스톨러 다운로드
//! (native-tls/Schannel, 리다이렉트 추적, 진행률 보고). nabidrive 검증 구조 이식.

use super::{is_newer_version, DownloadProgress, ReleaseInfo, UpdateStatus, ASSET_PREFIX, CURRENT_VERSION, REPO_PATH};
use crate::netparse::{dechunk, find_header, parse_status_code, parse_url};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 최신 릴리스를 조회한다. 새 버전이 있으면 Some(ReleaseInfo).
pub(super) fn check_github_release() -> Result<Option<ReleaseInfo>, String> {
    let body = https_get_text("api.github.com", REPO_PATH, GITHUB_ACCEPT)?;
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

    let notes = release["body"].as_str().unwrap_or("").to_string();
    Ok(Some(ReleaseInfo {
        version: version.to_string(),
        download_url,
        sha256: parse_sha256(&notes), // 노트에 적힌 체크섬(설치 전 대조용).
        notes,
    }))
}

/// 릴리스 노트에서 인스톨러의 SHA-256을 찾는다.
///
/// `nabiTerm-setup.exe`가 언급된 줄, 또는 `sha256`이 언급된 줄에서 64자리 16진수를 취한다.
/// 형식을 강제하지 않으려 느슨하게 훑되, 64자리 16진수만 유효로 본다.
pub(super) fn parse_sha256(notes: &str) -> Option<String> {
    let is_hash = |w: &str| w.len() == 64 && w.chars().all(|c| c.is_ascii_hexdigit());
    notes
        .lines()
        .filter(|l| {
            let low = l.to_ascii_lowercase();
            low.contains("sha256") || low.contains(&ASSET_PREFIX.to_ascii_lowercase())
        })
        .find_map(|l| {
            l.split(|c: char| !c.is_ascii_hexdigit())
                .find(|w| is_hash(w))
                .map(|w| w.to_ascii_lowercase())
        })
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

/// GitHub API가 기대하는 Accept. 호스트마다 다르다 — npm 레지스트리는 이 값을 주면 406으로 막는다.
pub(super) const GITHUB_ACCEPT: &str = "application/vnd.github+json";

/// 범용 HTTPS GET → 텍스트. `accept`는 호스트에 맞춰 호출부가 정한다.
pub(super) fn get_text(host: &str, path: &str, accept: &str) -> Result<String, String> {
    https_get_text(host, path, accept)
}

/// 범용 HTTPS 파일 다운로드(진행률 미보고 — 폰트 등 소형 파일용, 리다이렉트 추적).
pub(super) fn download_plain(url: &str, dest: &str) -> Result<(), String> {
    https_download_file(url, dest, &Arc::new(Mutex::new(UpdateStatus::Idle)))
}

/// TLS 스트림 생성(native-tls = Windows Schannel).
///
/// 오류에 **호스트를 반드시 넣는다.** 다운로드는 GitHub에서 자산 호스트로 리다이렉트되므로
/// 실제로 실패한 곳이 둘 중 어디인지 모르면 진단이 불가능하다(사용자 보고 2026-08-06:
/// "TLS 연결 실패: 인증서의 CN 이름이…" — 어느 호스트인지 알 수 없어 원인을 좁히지 못했다).
fn tls_connect(host: &str, port: u16) -> Result<native_tls::TlsStream<TcpStream>, String> {
    let tcp = TcpStream::connect((host, port))
        .map_err(|e| format!("TCP 연결 실패({host}:{port}): {e}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let connector = native_tls::TlsConnector::new().map_err(|e| format!("TLS 초기화 실패: {e}"))?;
    connector.connect(host, tcp).map_err(|e| {
        // 인증서 이름 불일치는 거의 항상 중간에 낀 것(사내 프록시·백신 HTTPS 검사)이다.
        // 우리 쪽에서 고칠 수 있는 게 아니므로, 무엇을 의심해야 하는지까지 적어 준다.
        let hint = match e.to_string().contains("CN") {
            true => " — HTTPS를 가로채는 백신·사내 프록시일 수 있습니다. 릴리스 페이지에서 직접 받으세요",
            false => "",
        };
        format!("TLS 연결 실패({host}): {e}{hint}")
    })
}

/// HTTPS GET → 텍스트(GitHub API용, 리다이렉트 미지원, chunked 디코드).
fn https_get_text(host: &str, path: &str, accept: &str) -> Result<String, String> {
    let mut stream = tls_connect(host, 443)?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: nabiTerm/{CURRENT_VERSION}\r\n\
         Accept: {accept}\r\nConnection: close\r\n\r\n"
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


#[cfg(test)]
mod live {
    /// 진단용: 실제 릴리스 자산을 **우리 코드로** 받아 본다(네트워크 필요).
    ///
    /// 사용자가 다운로드 실패를 보고하면 먼저 이걸 돌려 우리 쪽 문제인지 가른다.
    /// 2026-08-06 보고 때 이 테스트가 통과해서(9.5MB 정상 수신) 코드가 아니라 환경
    /// 문제로 좁힐 수 있었다.
    /// `cargo test -p nabi-release live_ -- --ignored --nocapture`
    #[test]
    #[ignore = "네트워크 필요"]
    fn live_download_real_asset() {
        let url = "https://github.com/matrixism-cmyk/NabiTermPub/releases/download/v0.1.410/nabiTerm-setup.exe";
        let status = std::sync::Arc::new(std::sync::Mutex::new(crate::UpdateStatus::Idle));
        let dest = std::env::temp_dir().join("nabi-live-dl.exe");
        match super::https_download_file(url, dest.to_str().unwrap(), &status) {
            Ok(()) => {
                let n = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                println!("OK {n} 바이트");
                let _ = std::fs::remove_file(&dest);
                assert!(n > 1_000_000, "받은 크기가 너무 작다: {n}");
            }
            Err(e) => panic!("다운로드 실패: {e}"),
        }
    }
}
