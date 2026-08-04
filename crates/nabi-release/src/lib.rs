//! 자동 업데이트 — GitHub Releases 확인 + 인스톨러 다운로드 + 실행.
//!
//! nabidrive의 검증된 구조 이식. 네트워크 계층은 [`net`], 여기는 공개 타입과
//! 다운로드/설치 상태기계를 소유한다. 저장소는 `matrixism-cmyk/NabiTermPub`.

mod net;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 현재 빌드 버전(Cargo.toml workspace.package.version).
pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// GitHub 릴리스 자산 접두 — `nabiTerm-setup*.exe`만 인스톨러로 인정.
pub(crate) const ASSET_PREFIX: &str = "nabiTerm-setup";
/// 공개 배포 저장소(릴리스가 올라오는 곳).
pub(crate) const REPO_PATH: &str = "/repos/matrixism-cmyk/NabiTermPub/releases/latest";

/// 사용자가 릴리스 페이지를 직접 열 수 있는 URL.
pub const RELEASES_URL: &str = "https://github.com/matrixism-cmyk/NabiTermPub/releases";

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub notes: String,
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
    Downloaded(String),
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
                *status.lock().unwrap_or_else(|e| e.into_inner()) =
                    UpdateStatus::Downloaded(path.clone());
                let _ = launch_installer(&path, &request_quit);
            }
            Err(e) => {
                *status.lock().unwrap_or_else(|e| e.into_inner()) =
                    UpdateStatus::Error(format!("다운로드 실패: {e}"));
            }
        });
    }
}

/// 범용 HTTPS GET → 텍스트(폰트 설치기 등 — GitHub API/raw). host 예: "api.github.com".
pub fn http_get_text(host: &str, path: &str) -> Result<String, String> {
    net::get_text(host, path)
}

/// 범용 HTTPS 파일 다운로드(폰트 등 소형 파일 — 진행률 미보고, 리다이렉트 추적).
pub fn download_file(url: &str, dest: &str) -> Result<(), String> {
    net::download_plain(url, dest)
}

/// 다운로드된 인스톨러 실행 + 종료 플래그 설정(호출측이 다음 프레임에 종료).
pub fn launch_installer(path: &str, request_quit: &Arc<AtomicBool>) -> Result<(), String> {
    std::process::Command::new(path)
        .spawn()
        .map_err(|e| format!("인스톨러 실행 실패: {e}"))?;
    request_quit.store(true, Ordering::Relaxed);
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
