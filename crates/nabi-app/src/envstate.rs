//! 이 PC의 **현재 상태**를 읽는다 — 무엇이 이미 깔려 있는가.
//!
//! 조회는 전부 프로세스를 띄우는 일이라 UI 스레드에서 하지 않는다. 화면은 `Loading`을
//! 그대로 그리고, 끝나면 한 번에 갈아 끼운다.

use crate::envwsl::Distro;
use std::sync::{Arc, Mutex};

/// 한 번 훑은 결과.
#[derive(Clone, Default, Debug)]
pub(crate) struct EnvState {
    /// 조회가 끝났는가.
    pub done: bool,
    /// PATH에서 찾은 도구 id들.
    pub installed: Vec<String>,
    /// winget이 이 PC에 있는가(설치 통로 선택이 여기 달렸다).
    pub has_winget: bool,
    /// 설치할 수 있는 WSL 배포판.
    pub distros: Vec<Distro>,
    /// 이미 깔린 WSL 배포판 이름.
    pub wsl_installed: Vec<String>,
    /// WSL 자체가 이 PC에 있는가.
    pub has_wsl: bool,
}

pub(crate) type EnvScan = Arc<Mutex<EnvState>>;

/// 배경에서 한 번 훑는다.
pub(crate) fn scan() -> EnvScan {
    let out: EnvScan = Arc::new(Mutex::new(EnvState::default()));
    let worker = out.clone();
    std::thread::spawn(move || {
        let installed: Vec<String> = crate::envcat::TOOLS
            .iter()
            .filter(|t| on_path(t.probe))
            .map(|t| t.id.to_string())
            .collect();
        let has_winget = installed.iter().any(|i| i == "winget");
        let has_wsl = on_path("wsl");
        let (distros, wsl_installed) = if has_wsl { wsl_lists() } else { (Vec::new(), Vec::new()) };
        if let Ok(mut s) = worker.lock() {
            *s = EnvState { done: true, installed, has_winget, distros, wsl_installed, has_wsl };
        }
    });
    out
}

/// PATH에 이 이름의 실행 파일이 있는가.
/// 이 도구가 **실제로 쓸 수 있게** 설치돼 있는가.
///
/// `where`만 믿으면 안 된다. Microsoft Store판은 `WindowsApps\`에 0바이트 앱 실행
/// 별칭을 놓는데, `where`는 그것도 찾아 준다. 그 계정에 앱 라이선스가 없으면 실행은
/// 실패하므로(PowerShell 7에서 실제로 그랬다 — 사용자 보고 2026-08-26), 별칭뿐이면
/// **설치되지 않은 것으로 본다.** 그래야 설치 단추가 나온다.
fn on_path(name: &str) -> bool {
    let Ok(o) = crate::aicli::hidden("where.exe").arg(name).output() else { return false };
    if !o.status.success() {
        return false;
    }
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .any(|l| !nabi_pty::is_store_alias(std::path::Path::new(l)))
}

/// 온라인 목록과 설치된 목록을 한 번에 읽는다.
fn wsl_lists() -> (Vec<Distro>, Vec<String>) {
    let online = wsl_text(&["--list", "--online"]);
    let mine = wsl_text(&["--list", "--quiet"]);
    (crate::envwsl::parse_online(&online), crate::envwsl::parse_installed(&mine))
}

/// wsl.exe를 돌려 **UTF-16LE을 풀어** 글자로 돌려준다.
fn wsl_text(args: &[&str]) -> String {
    crate::aicli::hidden("wsl.exe")
        .args(args)
        .output()
        .map(|o| crate::envwsl::decode(&o.stdout))
        .unwrap_or_default()
}

/// 배포판 하나를 까는 스크립트. WSL 자체가 없으면 그것부터 켠다.
///
/// `wsl --install`은 기능을 켜느라 **재부팅을 요구할 수 있다** — 화면이 그렇게 안내한다.
pub(crate) fn distro_script(name: &str, has_wsl: bool) -> String {
    let mut s = String::new();
    if !has_wsl {
        s.push_str("Write-Output '@@STEP 1/3 wsl'; wsl --install --no-distribution; ");
    }
    s.push_str("Write-Output '@@STEP 2/3 distro'; ");
    s.push_str(&format!("wsl --install -d {name}; "));
    s.push_str("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; Write-Output '@@STEP 3/3 done'");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WSL이 이미 있으면 기능 켜기 단계를 건너뛴다(괜히 재부팅을 부르지 않는다).
    #[test]
    fn an_existing_wsl_is_not_reinstalled() {
        let s = distro_script("Ubuntu", true);
        assert!(!s.contains("--no-distribution"), "{s}");
        assert!(s.contains("wsl --install -d Ubuntu"));
    }

    #[test]
    fn a_missing_wsl_is_enabled_first() {
        let s = distro_script("Debian", false);
        assert!(s.contains("--no-distribution"));
        assert!(s.find("--no-distribution") < s.find("-d Debian"), "순서가 뒤집혔다");
    }

    #[test]
    fn the_script_reports_progress() {
        assert!(distro_script("Ubuntu", true).contains("@@STEP"));
    }

    /// 훑기 전에는 아무것도 깔린 것으로 보이면 안 된다.
    #[test]
    fn an_unscanned_state_claims_nothing() {
        let s = EnvState::default();
        assert!(!s.done && !s.has_winget && !s.has_wsl);
        assert!(s.installed.is_empty() && s.distros.is_empty());
    }
}
