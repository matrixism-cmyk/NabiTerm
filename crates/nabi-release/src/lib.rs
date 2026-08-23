//! 자동 업데이트 — GitHub Releases 확인 + 인스톨러 다운로드 + 실행.
//!
//! nabidrive의 검증된 구조 이식. 네트워크 계층은 [`net`], 여기는 공개 타입과
//! 다운로드/설치 상태기계를 소유한다. 저장소는 `matrixism-cmyk/NabiTerm`(소스+릴리스 통합,
//! 2026-08-19 오픈소스 전환). 구 저장소 NabiTermPub은 전환기 동안 이중 게시 후 일몰 —
//! v0.1.446 이하 클라이언트는 Pub을 보므로, 전환기에는 두 곳 모두에 릴리스를 올린다.

mod net;
mod netparse;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 현재 빌드 버전(Cargo.toml workspace.package.version).
pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// GitHub 릴리스 자산 접두 — `nabiTerm-setup*.exe`만 인스톨러로 인정.
pub(crate) const ASSET_PREFIX: &str = "nabiTerm-setup";
/// 공개 배포 저장소(릴리스가 올라오는 곳) — 소스와 같은 NabiTerm 레포.
pub(crate) const REPO_PATH: &str = "/repos/matrixism-cmyk/NabiTerm/releases/latest";

/// 사용자가 릴리스 페이지를 직접 열 수 있는 URL.
pub const RELEASES_URL: &str = "https://github.com/matrixism-cmyk/NabiTerm/releases";

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub notes: String,
    /// 인스톨러의 기대 SHA-256(소문자 16진 64자). 릴리스 노트에 적혀 있으면 채워지며,
    /// 설치 실행 **전에** 대조한다. 없으면 검증 없이 실행하지 않고 사용자에게 알린다.
    pub sha256: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub speed_bps: u64,
}

impl DownloadProgress {
    pub fn percent(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f32 / self.total as f32) * 100.0
        }
    }

    pub fn display(&self) -> String {
        let mb = self.downloaded as f64 / 1_048_576.0;
        let speed_mb = self.speed_bps as f64 / 1_048_576.0;
        if self.total > 0 {
            let total_mb = self.total as f64 / 1_048_576.0;
            format!("{mb:.1}/{total_mb:.1} MB  ({speed_mb:.1} MB/s)")
        } else {
            format!("{mb:.1} MB  ({speed_mb:.1} MB/s)")
        }
    }
}

#[derive(Clone, Debug)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading(DownloadProgress),
    /// (내려받은 경로, 릴리스가 공지한 기대 SHA-256) — 실행 전 이 해시로 검증한다.
    Downloaded(String, Option<String>),
    Error(String),
}

/// 백그라운드 스레드와 UI가 공유하는 업데이트 상태.
#[derive(Clone)]
pub struct UpdateChecker {
    status: Arc<Mutex<UpdateStatus>>,
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
        }
    }

    fn set(&self, s: UpdateStatus) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }

    pub fn get_status(&self) -> UpdateStatus {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 백그라운드로 최신 릴리스 확인(네트워크는 별도 스레드 — UI 비차단).
    pub fn check_async(&self) {
        let status = self.status.clone();
        *status.lock().unwrap_or_else(|e| e.into_inner()) = UpdateStatus::Checking;
        std::thread::spawn(move || {
            let next = match net::check_github_release() {
                Ok(Some(r)) => UpdateStatus::Available(r),
                Ok(None) => UpdateStatus::UpToDate,
                Err(e) => UpdateStatus::Error(e),
            };
            *status.lock().unwrap_or_else(|e| e.into_inner()) = next;
        });
    }

    /// 인스톨러 다운로드(진행률 보고) → 완료 시 자동 실행 + 종료 요청.
    pub fn download_async(&self, release: ReleaseInfo, request_quit: Arc<AtomicBool>) {
        let status = self.status.clone();
        self.set(UpdateStatus::Downloading(DownloadProgress {
            downloaded: 0,
            total: 0,
            speed_bps: 0,
        }));
        std::thread::spawn(move || match net::download_installer(&release.download_url, &status) {
            Ok(path) => {
                let want = release.sha256.clone();
                // 검증 실패면 실행하지 않고 오류 상태로 남긴다(변조·손상 파일 실행 차단).
                match launch_installer(&path, want.as_deref(), &request_quit) {
                    Ok(()) => {
                        *status.lock().unwrap_or_else(|e| e.into_inner()) =
                            UpdateStatus::Downloaded(path, want);
                    }
                    Err(e) => {
                        *status.lock().unwrap_or_else(|e| e.into_inner()) =
                            UpdateStatus::Error(e);
                    }
                }
            }
            Err(e) => {
                // 실패한 **URL을 함께 남긴다.** 자동 업데이트가 막히는 환경(HTTPS 검사
                // 백신·사내 프록시)에서는 이 주소를 브라우저에 넣으면 대개 받아진다.
                // 어디서 막혔는지 알려 주지 않으면 사용자도 우리도 원인을 좁힐 수 없다.
                let url = release.download_url.clone();
                *status.lock().unwrap_or_else(|e| e.into_inner()) =
                    UpdateStatus::Error(format!("다운로드 실패: {e}\n{url}"));
            }
        });
    }
}

#[cfg(test)]
mod verify_tests {
    use super::verify_installer;

    fn tmp(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("nabi-verify-{}-{name}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn accepts_matching_hash() {
        let p = tmp("ok");
        std::fs::write(&p, b"nabi").unwrap();
        let actual = super::sha256_hex(&p).expect("해시 계산");
        assert_eq!(actual.len(), 64, "SHA-256은 16진 64자");
        assert!(verify_installer(&p, Some(&actual)).is_ok(), "일치하면 통과");
        // 대소문자 무관하게 일치해야 한다(릴리스 노트가 대문자로 적힐 수 있음).
        assert!(verify_installer(&p, Some(&actual.to_uppercase())).is_ok());
        assert!(std::path::Path::new(&p).exists(), "정상 파일은 지우지 않는다");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rejects_and_deletes_on_mismatch() {
        let p = tmp("bad");
        std::fs::write(&p, b"tampered").unwrap();
        let wrong = "0".repeat(64);
        assert!(verify_installer(&p, Some(&wrong)).is_err(), "불일치는 거부");
        assert!(!std::path::Path::new(&p).exists(), "변조 파일은 남기지 않는다");
    }

    #[test]
    fn refuses_when_no_hash_published() {
        let p = tmp("nohash");
        std::fs::write(&p, b"x").unwrap();
        assert!(verify_installer(&p, None).is_err(), "해시 미공지면 실행하지 않는다");
        assert!(!std::path::Path::new(&p).exists());
    }

    #[test]
    fn parses_hash_from_release_notes() {
        let h = "a".repeat(64);
        let notes = format!("## v1\n\n- 수정 사항\n\nSHA256 (nabiTerm-setup.exe) = {h}\n");
        assert_eq!(crate::net::parse_sha256(&notes).as_deref(), Some(h.as_str()));
        assert_eq!(crate::net::parse_sha256("체크섬 없음"), None);
        // 64자리가 아니면 무시한다.
        assert_eq!(crate::net::parse_sha256("sha256: abc123"), None);
    }
}

/// 범용 HTTPS GET → 텍스트(폰트 설치기 등 — GitHub API/raw). host 예: "api.github.com".
pub fn http_get_text(host: &str, path: &str) -> Result<String, String> {
    net::get_text(host, path, net::GITHUB_ACCEPT)
}

/// Accept 헤더를 지정하는 HTTPS GET.
///
/// GitHub용 Accept를 그대로 쓰면 다른 호스트가 거절한다 — npm 레지스트리는 실제로 **406**을
/// 돌려준다(2026-08-12 확인). 호스트가 늘 때마다 여기서 Accept를 맞춘다.
pub fn http_get_text_accept(host: &str, path: &str, accept: &str) -> Result<String, String> {
    net::get_text(host, path, accept)
}

/// 범용 HTTPS 파일 다운로드(폰트 등 소형 파일 — 진행률 미보고, 리다이렉트 추적).
pub fn download_file(url: &str, dest: &str) -> Result<(), String> {
    net::download_plain(url, dest)
}

/// 파일의 SHA-256을 소문자 16진 문자열로.
fn sha256_hex(path: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let data = std::fs::read(path).map_err(|e| format!("파일 읽기 실패: {e}"))?;
    let mut h = Sha256::new();
    h.update(&data);
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 내려받은 인스톨러가 릴리스에 공지된 해시와 일치하는지 확인한다.
///
/// 관리자 권한으로 실행될 파일이므로 **검증 없이 실행하지 않는다**. 해시가 공지되지 않았거나
/// 일치하지 않으면 파일을 지우고 오류를 돌려준다(사용자는 릴리스 페이지에서 수동 설치 가능).
pub fn verify_installer(path: &str, expected: Option<&str>) -> Result<(), String> {
    let Some(expected) = expected else {
        let _ = std::fs::remove_file(path);
        return Err("릴리스에 SHA-256이 공지되지 않아 설치를 중단했습니다".into());
    };
    let actual = sha256_hex(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    let _ = std::fs::remove_file(path); // 손상·변조 파일은 남기지 않는다.
    Err(format!("무결성 검증 실패(SHA-256 불일치): {actual} ≠ {expected}"))
}

/// 검증된 인스톨러 실행 + 종료 플래그 설정(호출측이 다음 프레임에 종료).
///
/// `expected_sha256`은 릴리스가 공지한 해시. 일치할 때만 실행한다.
pub fn launch_installer(
    path: &str,
    expected_sha256: Option<&str>,
    request_quit: &Arc<AtomicBool>,
) -> Result<(), String> {
    verify_installer(path, expected_sha256)?;
    spawn_after_we_exit(path)?;
    request_quit.store(true, Ordering::Relaxed);
    Ok(())
}

/// 우리가 **완전히 종료한 뒤에** 인스톨러를 띄운다.
///
/// 왜 기다리는가: 인스톨러는 시작하자마자 `AppMutex`로 실행 중인 nabiTerm을 찾는다.
/// 예전에는 종료를 요청하기 **직전에** 인스톨러를 띄웠기 때문에, 우리가 아직 살아 있는 채로
/// 설치가 시작돼 "nabiTerm이 실행 중입니다. 닫고 확인을 누르세요" 대화상자가 떴다.
/// 그 창이 다른 창 뒤에 가리면 사용자는 앱이 닫힌 것만 보고 설치가 끝난 줄 안다 —
/// 결과는 "업데이트했는데 다시 켜지지 않는다"였다(사용자 보고 2026-08-22, 설치 로그로 확인:
/// `Defaulting to Cancel for suppressed message box … currently running` → 종료 코드 1).
///
/// 그래서 몇 초 뒤에 시작하도록 예약한다. 그 사이 우리는 워크스페이스를 저장하고 나간다.
/// `/SILENT`으로 마법사 대신 진행 창만 띄우고, 설치가 끝나면 인스톨러가 nabiTerm을 다시 켠다
/// (installer/nabiTerm.iss의 `[Run]` — 조용한 설치에서도 실행되도록 `skipifsilent`를 뺐다).
fn spawn_after_we_exit(path: &str) -> Result<(), String> {
    let dir = std::path::Path::new(path)
        .parent()
        .ok_or_else(|| format!("인스톨러 경로가 이상합니다: {path}"))?;
    spawn_script(dir, &delay_script(path))
}

/// 지연 실행 스크립트 본문. **따옴표는 이 파일 안에만 있다** — 명령줄로 넘기지 않는다.
///
/// `timeout`은 콘솔이 없으면 실패하므로(창 없이 띄운다) `ping`으로 기다린다.
fn delay_script(target: &str) -> String {
    format!("@echo off\r\nping -n 6 127.0.0.1 >nul\r\nstart \"\" \"{target}\" /SILENT\r\n")
}

/// 스크립트를 파일로 써서 cmd에 **파일 경로 하나만** 넘겨 실행한다.
///
/// 예전에는 명령 전체를 `Command::args(["/c", &script])`로 넘겼다. Rust는 공백이 든 인자를
/// 따옴표로 감싸면서 **안쪽 따옴표를 `\"`로 이스케이프**하는데, cmd.exe는 `\"`를 모른다
/// (백슬래시는 cmd의 이스케이프 문자가 아니다). 그래서 명령줄이 깨져 `\`로 시작하는 무언가를
/// 실행하려 했고, 사용자에게는 이렇게 보였다 —
/// **"'\'을(를) 찾을 수 없습니다"**(사용자 보고 2026-08-23).
///
/// 따옴표를 스크립트 **파일 안에** 두면 그 문제가 통째로 사라진다. cmd에는 경로 하나만
/// 넘기고, `/s`와 바깥 따옴표 한 쌍으로 경로에 공백이 있어도 정확히 해석되게 한다.
fn spawn_script(dir: &std::path::Path, body: &str) -> Result<(), String> {
    let script = dir.join("nabi-run-update.cmd");
    std::fs::create_dir_all(dir).map_err(|e| format!("업데이트 폴더를 쓸 수 없습니다: {e}"))?;
    std::fs::write(&script, body).map_err(|e| format!("업데이트 스크립트를 쓸 수 없습니다: {e}"))?;

    let mut cmd = std::process::Command::new("cmd");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // 따옴표를 **두 겹** 씌운다. `/s`는 첫·마지막 따옴표를 떼어 내므로, 한 겹만 씌우면
        // 경로가 벌거벗은 채 남아 공백에서 잘린다(직접 확인:
        // `'C:\Users\...\Temp\nabi' is not recognized`). 두 겹이면 바깥 한 쌍만 벗겨지고
        // 안쪽 따옴표가 남아 공백 있는 경로가 정확히 해석된다.
        cmd.raw_arg(format!("/s /c \"\"{}\"\"", script.display()));
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        cmd.arg(&script);
    }
    cmd.spawn().map_err(|e| format!("인스톨러 실행 예약 실패: {e}"))?;
    Ok(())
}

/// 버전 비교(숫자 기반): "0.1.6" > "0.1.5" → true. 'v' 접두는 호출측이 제거.
pub(crate) fn is_newer_version(remote: &str, current: &str) -> bool {
    // 선두 'v'를 방어적으로 제거 — 'vX'가 0으로 파싱돼 major≥1에서 오판하는 것을 막는다(호출자 무관).
    let parse = |v: &str| -> Vec<u32> {
        v.trim().trim_start_matches('v').split('.').map(|s| s.parse::<u32>().unwrap_or(0)).collect()
    };
    let (r, c) = (parse(remote), parse(current));
    for i in 0..r.len().max(c.len()) {
        let (rv, cv) = (r.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
        if rv != cv {
            return rv > cv;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_newer_version;

    #[test]
    fn version_compare() {
        assert!(is_newer_version("0.3.25", "0.3.24"));
        assert!(!is_newer_version("0.3.24", "0.3.24"));
        assert!(!is_newer_version("0.3.23", "0.3.24"));
        assert!(is_newer_version("0.4.0", "0.3.99"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("0.1.0", "0.0.0"));
        assert!(is_newer_version("0.3.24.1", "0.3.24"));
        assert!(!is_newer_version("0.3", "0.3.0"));
        // 'v' 접두사가 있어도 정확(태그 vX.Y.Z 방어) — major≥1 오판 회귀 방지.
        assert!(is_newer_version("v1.0.0", "0.9.9"));
        assert!(is_newer_version("v0.1.361", "v0.1.360"));
    }
}

#[cfg(test)]
mod launch_tests {
    use super::{delay_script, spawn_script};

    /// 스크립트 본문에 따옴표가 제대로 들어가는가(경로에 공백이 있어도).
    #[test]
    fn the_script_quotes_the_installer_path() {
        let s = delay_script(r"C:\Program Files\nabi\setup update.exe");
        assert!(s.contains("ping -n 6"), "기다리는 줄이 있어야 한다: {s}");
        assert!(
            s.contains(r#"start "" "C:\Program Files\nabi\setup update.exe" /SILENT"#),
            "공백 있는 경로를 따옴표로 감싸야 한다: {s}"
        );
        assert!(!s.contains(r#"\""#), "이스케이프한 따옴표가 있으면 cmd가 못 읽는다: {s}");
    }

    /// **회귀 시험**: 실제로 cmd를 띄워 스크립트가 도는지 본다.
    ///
    /// 예전 코드는 명령 전체를 인자로 넘겨서, Rust가 안쪽 따옴표를 `\"`로 바꿔 놓는 바람에
    /// cmd가 `\`로 시작하는 것을 실행하려 했다("'\'을(를) 찾을 수 없습니다").
    /// **공백이 든 폴더**에서 돌려야 그 함정을 실제로 밟는다.
    #[test]
    #[cfg(windows)]
    fn the_script_actually_runs_from_a_path_with_spaces() {
        let dir = std::env::temp_dir().join(format!("nabi run update {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("it ran.txt");
        // 인스톨러 대신 표식 파일을 남기게 한다(설치를 실제로 하지 않는다).
        let body = format!("@echo off\r\necho ok> \"{}\"\r\n", marker.display());
        spawn_script(&dir, &body).expect("띄우기");

        // 자식이 끝날 때까지 잠깐 기다린다(폴링 — 고정 대기는 느리고 불안정하다).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(marker.exists(), "공백 있는 경로에서 스크립트가 돌지 않았다: {}", dir.display());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
