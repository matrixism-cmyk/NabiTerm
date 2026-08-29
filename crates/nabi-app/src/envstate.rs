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
    /// 도구 id → 버전 문자열(배치 AK). 읽지 못했으면 없다.
    ///
    /// 설치되어 있다는 사실만으로는 부족하다는 사용자 요청으로 넣었다. 어차피 "정말
    /// 실행되는가"를 확인하려고 한 번 실행하므로, 그때 받은 값을 그대로 쓴다.
    pub versions: std::collections::HashMap<String, String>,
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
        // 도구마다 **정말 실행되는지** 확인하고, 되면 버전을 함께 받는다(배치 AK).
        //
        // 예전에는 파일이 스토어 별칭이면 무조건 "없음"으로 봤다. 그런데 winget 은 별칭인데도
        // 잘 실행된다. 파일 모양으로는 winget 과 pwsh 를 구분할 수 없으므로 실행해서 묻는다.
        let mut installed: Vec<String> = Vec::new();
        let mut versions = std::collections::HashMap::new();
        for t in crate::envcat::TOOLS.iter() {
            // PATH 에 명령이 없는 것(런타임 등)은 폴더로 본다.
            if let Some(dir) = t.folder {
                if let Some(v) = probe_folder(dir) {
                    installed.push(t.id.to_string());
                    versions.insert(t.id.to_string(), v);
                }
                continue;
            }
            match probe(t.probe) {
                None => continue,
                Some(v) => {
                    installed.push(t.id.to_string());
                    if let Some(v) = v {
                        versions.insert(t.id.to_string(), v);
                    }
                }
            }
        }
        let has_winget = installed.iter().any(|i| i == "winget");
        let has_wsl = on_path("wsl");
        let (distros, wsl_installed) = if has_wsl { wsl_lists() } else { (Vec::new(), Vec::new()) };
        if let Ok(mut s) = worker.lock() {
            *s = EnvState { done: true, installed, versions, has_winget, distros, wsl_installed, has_wsl };
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

/// 이 도구가 **쓸 수 있게** 설치되어 있는가. 있으면 `Some(버전)`(버전은 못 읽을 수도 있다).
///
/// 먼저 `where` 로 찾는다. 스토어 별칭이 아니면 그것으로 충분하다 — 실행해 보는 값이
/// 아깝다(도구가 열 개가 넘고, 매번 훑는다).
///
/// **별칭일 때만 실행해 본다.** 그때는 파일이 있어도 실행되지 않을 수 있어서, 물어보는
/// 수밖에 없다. winget 은 되고 스토어판 pwsh 는 안 된다.
/// 폴더로 찾는다. 있으면 그 안의 **판 번호 폴더 이름**이 곧 버전이다.
///
/// WebView2 런타임처럼 실행 파일을 PATH 에 두지 않는 것들이 있다. `where.exe` 로 찾으면
/// 영영 "없음"이 되어, 설치해 놓고도 설치하라고 계속 권하게 된다.
fn probe_folder(rel: &str) -> Option<String> {
    let base = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let dir = std::path::Path::new(&base).join(rel);
    let mut best: Option<String> = None;
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        // 판 번호처럼 생긴 것만 — SetupMetrics 같은 폴더가 섞여 있다.
        if name.starts_with(|c: char| c.is_ascii_digit()) && best.as_deref() < Some(name.as_str()) {
            best = Some(name);
        }
    }
    best
}

fn probe(name: &str) -> Option<Option<String>> {
    let Ok(o) = crate::aicli::hidden("where.exe").arg(name).output() else { return None };
    if !o.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&o.stdout);
    let paths: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if paths.is_empty() {
        return None;
    }
    let real = paths.iter().any(|l| !nabi_pty::is_store_alias(std::path::Path::new(l)));
    if real {
        return Some(crate::envprobe::probe_version(name, "--version"));
    }
    // 별칭뿐이다 — 실행되면 설치된 것이고, 안 되면 없는 것으로 본다.
    crate::envprobe::probe_version(name, "--version").map(Some)
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
