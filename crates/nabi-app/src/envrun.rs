//! 환경 설치 **실행기** — 진행률이 실제 진행을 따라가게 한다.
//!
//! 지난번에 받은 지적이 정확했다: "한세월 기다리다가는 다들 창을 닫아버릴걸." 시간을
//! 추측해서 막대를 슬금슬금 미는 것은 거짓말이다. 그래서 반대로 뒤집었다 — **설치
//! 스크립트가 자기 단계를 말하고**(`@@STEP i/n 라벨`), 여기서는 그 말만 옮긴다.
//!
//! 그러면 막대가 멈춰 있을 때 그것은 실제로 그 단계가 오래 걸리는 것이고, 사용자는
//! 라벨로 무엇을 기다리는지 안다. 추측이 없으니 어긋날 것도 없다.

use crate::aicli::{finish, hidden, set_progress, ActionJob, ActionProgress};
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

/// `@@STEP i/n 라벨` 한 줄을 (진행률, 라벨)로 읽는다. 그 꼴이 아니면 None.
pub(crate) fn parse_step(line: &str) -> Option<(f32, String)> {
    let rest = line.trim().strip_prefix("@@STEP")?.trim_start();
    let (frac, label) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let (i, n) = frac.split_once('/')?;
    let (i, n): (f32, f32) = (i.trim().parse().ok()?, n.trim().parse().ok()?);
    if n <= 0.0 || i < 0.0 || i > n {
        return None;
    }
    Some((i / n, label.trim().to_string()))
}

/// 설치/제거 스크립트를 돌리며 `@@STEP`을 그대로 진행률에 옮긴다.
///
/// 스크립트가 한 줄도 말하지 않으면 막대는 시작 위치에 머문다 — **그게 정직하다.**
/// 가짜로 밀지 않는다.
pub(crate) fn start_script(script: String, first: String) -> ActionJob {
    let job: ActionJob = Arc::new(Mutex::new(ActionProgress { message: first, ..Default::default() }));
    let worker = job.clone();
    std::thread::spawn(move || {
        // **표준 오류도 같이 받는다.** 처음에는 stdout만 읽었는데, PowerShell의 throw는
        // stderr로 나가므로 실패했을 때 화면에 "설치 명령 실패" 여섯 글자만 남았다 —
        // 실제 이유(0x80073CF3, 어떤 프레임워크가 없는지)가 통째로 사라졌다.
        let child = hidden("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return finish(&worker, Err(e)),
        };
        // stderr는 다른 스레드에서 빨아들인다 — 파이프가 차면 자식이 거기서 멈춘다.
        let err_buf = std::sync::Arc::new(Mutex::new(String::new()));
        let err_reader = child.stderr.take().map(|e| {
            let sink = err_buf.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    if let (Ok(mut b), false) = (sink.lock(), line.trim().is_empty()) {
                        b.push_str(line.trim());
                        b.push(' ');
                    }
                }
            })
        });
        let mut tail = String::new();
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                match parse_step(&line) {
                    Some((f, label)) => set_progress(&worker, f, label),
                    // 마지막 평범한 줄은 실패했을 때 보여 줄 실마리로 남겨 둔다.
                    None if !line.trim().is_empty() => tail = line.trim().to_string(),
                    None => {}
                }
            }
        }
        let status = child.wait();
        if let Some(h) = err_reader {
            let _ = h.join();
        }
        let why = err_buf.lock().map(|b| b.trim().to_string()).unwrap_or_default();
        let res = status.map(|status| std::process::Output {
            status,
            stdout: failure_reason(&why, &tail).into_bytes(),
            stderr: Vec::new(),
        });
        finish(&worker, res);
    });
    job
}

/// 실패했을 때 사용자에게 보여 줄 한 줄. 오류가 있으면 그것을, 없으면 마지막 출력을 쓴다.
///
/// 너무 길면 창을 밀어내므로 자른다 — 전문은 진단 로그에 남는다.
pub(crate) fn failure_reason(stderr: &str, tail: &str) -> String {
    const MAX: usize = 300;
    let src = if stderr.trim().is_empty() { tail.trim() } else { stderr.trim() };
    if src.chars().count() <= MAX {
        return src.to_string();
    }
    let cut: String = src.chars().take(MAX).collect();
    format!("{cut}…")
}

/// 이 도구를 어떻게 깔 것인가 — winget이 있으면 winget, 없으면 직접 내려받기.
///
/// winget이 없는 PC(Windows Server가 그렇다)에서 winget 경로만 두면 화면이 통째로 죽는다.
pub(crate) fn install_script(tool: &crate::envcat::Tool, has_winget: bool) -> Option<String> {
    if tool.unavailable.is_some() {
        return None;
    }
    if has_winget {
        if let Some(id) = tool.winget {
            return Some(winget_install(id, tool.store_pkg));
        }
    }
    // winget이 없으면 MSI를 직접 받는다. 이 길에도 스토어판 제거를 앞에 붙인다 —
    // 안 그러면 정식 설치본을 깔아도 별칭이 PATH를 먼저 차지해 여전히 안 열린다.
    let purge = purge_store(tool.store_pkg);
    match tool.fallback {
        Some(s) if s.starts_with("GHMSI:") => crate::envcat::gh_msi_script(s).map(|x| purge + &x),
        // npm 패키지(언어 서버 등). Node가 없으면 그 안내를 먼저 내보낸다 — 조용히
        // 실패하면 "왜 안 깔리지"만 남는다.
        Some(s) if s.starts_with("NPM:") => Some(purge + &npm_install_script(&s["NPM:".len()..])),
        Some(s) => Some(purge + s),
        // winget이 없는데 winget 통로밖에 없다면, winget부터 깔라고 해야 한다.
        None => None,
    }
}

/// 제거 스크립트. winget으로 깐 것만 우리가 지울 수 있다.
pub(crate) fn remove_script(tool: &crate::envcat::Tool, has_winget: bool) -> Option<String> {
    if let Some(s) = tool.remove {
        return Some(s.to_string());
    }
    let id = tool.winget?;
    has_winget.then(|| {
        format!(
            "Write-Output '@@STEP 1/2 removing'; winget uninstall --id {id} --silent \
             --disable-interactivity --accept-source-agreements; Write-Output '@@STEP 2/2 done'"
        )
    })
}

/// npm 전역 설치 스크립트. `@@STEP` 두 단계를 뱉는다.
///
/// Node가 없으면 **깔지 않고 알려 준다.** 여기서 Node까지 받아 오게 하면 이 함수가
/// AI CLI 설치 경로(`aicli::install_npm_cli`)와 같은 일을 두 벌로 하게 된다 — 그쪽은
/// 진행률까지 다루므로 훨씬 낫다. 언어 서버는 흔치 않은 경로라 안내로 충분하다.
fn npm_install_script(package: &str) -> String {
    format!(
        "Write-Output '@@STEP 1/2 installing'; \
         if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {{ \
           Write-Error 'Node.js(npm)가 필요합니다. 도구 > 환경 관리자에서 AI CLI를 설치하면 Node도 함께 깔립니다.'; exit 1 }}; \
         npm.cmd install -g {package}; \
         if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}; Write-Output '@@STEP 2/2 done'"
    )
}

/// 스토어판을 먼저 지우는 앞머리. 없으면 빈 문자열.
///
/// 스토어판이 남아 있으면 `WindowsApps` 의 앱 실행 별칭이 PATH를 먼저 차지한다. 그 계정에
/// 앱 라이선스가 없으면 정식 설치본을 깔아도 별칭이 먼저 잡혀 여전히 실행되지 않는다
/// (PowerShell 7에서 실제로 그랬다 — 사용자 보고 2026-08-26).
///
/// **`@@STEP`을 뱉지 않는다.** 뒤에 붙는 설치 스크립트가 제 번호를 매기는데 여기서
/// 끼어들면 진행바가 어긋난다. 안 깔려 있으면 아무 일도 하지 않는다.
pub(crate) fn purge_store(pkg: Option<&str>) -> String {
    let Some(pkg) = pkg else { return String::new() };
    format!(
        "Get-AppxPackage -Name {pkg}* | ForEach-Object {{ \
           Remove-AppxPackage -Package $_.PackageFullName -ErrorAction SilentlyContinue }}; "
    )
}

/// winget 설치 한 줄. **원본을 `winget`으로 못 박는다** — msstore 원본이 잡히면
/// 방금 지운 스토어판이 도로 깔린다.
fn winget_install(id: &str, store_pkg: Option<&str>) -> String {
    format!(
        "{}Write-Output '@@STEP 1/2 installing'; \
         winget install --id {id} --source winget --silent --disable-interactivity \
         --accept-package-agreements --accept-source-agreements -e; \
         if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}; Write-Output '@@STEP 2/2 done'",
        purge_store(store_pkg)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envcat;

    /// **스토어판을 먼저 지운다** — 남겨 두면 앱 실행 별칭이 PATH를 먼저 차지해
    /// 정식 설치본을 깔아도 여전히 안 열린다.
    #[test]
    fn installing_powershell_removes_the_store_edition_first() {
        let pwsh = envcat::find("pwsh").copied().expect("pwsh 항목이 있어야 한다");
        assert_eq!(pwsh.store_pkg, Some("Microsoft.PowerShell"));
        for has_winget in [true, false] {
            let s = install_script(&pwsh, has_winget).expect("설치 통로가 있어야 한다");
            assert!(s.contains("Remove-AppxPackage"), "winget={has_winget}: 스토어판을 안 지운다");
            let purge_at = s.find("Remove-AppxPackage").unwrap();
            let install_at = s.find("installing").or_else(|| s.find("msi")).unwrap_or(usize::MAX);
            assert!(purge_at < install_at, "winget={has_winget}: 지우기가 설치보다 뒤에 있다");
        }
    }

    /// **winget 원본을 못 박는다** — msstore 원본이 잡히면 스토어판이 도로 깔린다.
    #[test]
    fn the_winget_source_is_pinned() {
        let pwsh = envcat::find("pwsh").copied().unwrap();
        let s = install_script(&pwsh, true).unwrap();
        assert!(s.contains("--source winget"), "{s}");
    }

    /// 스토어판이 없는 도구는 **아무 일도 하지 않는다**(쓸데없는 명령을 넣지 않는다).
    #[test]
    fn tools_without_a_store_edition_are_untouched() {
        assert_eq!(purge_store(None), "");
        let gh = envcat::find("gh").copied().unwrap();
        let s = install_script(&gh, true).unwrap();
        assert!(!s.contains("Remove-AppxPackage"), "{s}");
    }

    /// **실패 이유가 사라지면 안 된다** — 실제로 0x80073CF3 전문이 통째로 날아갔었다.
    #[test]
    fn a_failure_keeps_its_reason() {
        assert_eq!(failure_reason("HRESULT 0x80073CF3", "Downloading"), "HRESULT 0x80073CF3");
        assert_eq!(failure_reason("", "Downloading"), "Downloading", "오류가 없으면 마지막 출력이라도");
        assert_eq!(failure_reason("  ", "  "), "");
    }

    /// 아주 긴 오류가 창을 밀어내면 안 된다.
    #[test]
    fn a_very_long_reason_is_trimmed() {
        let long = "가".repeat(1000);
        let got = failure_reason(&long, "");
        assert_eq!(got.chars().count(), 301, "300자 + 말줄임");
        assert!(got.ends_with('…'));
    }

    #[test]
    fn a_step_line_becomes_a_fraction() {
        assert_eq!(parse_step("@@STEP 1/4 vclibs"), Some((0.25, "vclibs".to_string())));
        assert_eq!(parse_step("  @@STEP 4/4 install "), Some((1.0, "install".to_string())));
        assert_eq!(parse_step("@@STEP 0/3"), Some((0.0, String::new())));
    }

    /// 평범한 출력이 진행률로 둔갑하면 막대가 제멋대로 뛴다.
    #[test]
    fn ordinary_output_is_not_a_step() {
        assert!(parse_step("Downloading...").is_none());
        assert!(parse_step("@@STEP").is_none());
        assert!(parse_step("@@STEP abc/def x").is_none());
        assert!(parse_step("@@STEP 5/4 x").is_none(), "100%를 넘는 단계는 거짓이다");
        assert!(parse_step("@@STEP 1/0 x").is_none());
    }

    /// **winget이 없어도 길이 있어야 한다** — 서버에서 화면이 죽지 않게.
    #[test]
    fn without_winget_the_fallback_is_used() {
        let gh = envcat::find("gh").unwrap();
        let s = install_script(gh, false).expect("직접 내려받기 경로가 있어야 한다");
        assert!(s.contains("msiexec") && s.contains("repos/cli/cli"), "{s}");
        assert!(!s.contains("winget install"));
    }

    #[test]
    fn with_winget_the_package_manager_wins() {
        let gh = envcat::find("gh").unwrap();
        let s = install_script(gh, true).unwrap();
        assert!(s.contains("winget install --id GitHub.cli"));
        assert!(s.contains("@@STEP"), "진행 단계를 말해야 한다");
    }

    /// winget 자신은 winget으로 깔 수 없다 — 있어도 직접 내려받기로 가야 한다.
    #[test]
    fn winget_installs_itself_by_download() {
        let w = envcat::find("winget").unwrap();
        let s = install_script(w, true).unwrap();
        assert!(s.contains("Add-AppxPackage"), "{s}");
    }

    /// 윈도우에 없는 것은 설치 버튼이 나오면 안 된다.
    #[test]
    fn an_unavailable_tool_has_no_script() {
        let sp = envcat::find("sshpass").unwrap();
        assert!(install_script(sp, true).is_none());
        assert!(remove_script(sp, true).is_none());
    }

    /// winget이 없으면 제거도 못 한다 — 못 하는 것을 되는 척하지 않는다.
    #[test]
    fn removal_needs_a_channel() {
        let rg = envcat::find("ripgrep").unwrap();
        assert!(remove_script(rg, true).is_some());
        assert!(remove_script(rg, false).is_none());
    }
}

/// 실제로 이 PC에 설치해 보는 검증. 순수 시험은 스크립트의 **모양**만 볼 수 있고,
/// URL이 404인지·msiexec가 조용히 도는지는 돌려 봐야만 안다(실제로 URL 두 개가 404였다).
///
/// ```text
/// $env:NABI_ENV_REAL="gh"; cargo test -p nabi-app real_install -- --ignored --nocapture
/// ```
#[cfg(test)]
mod real {
    use super::*;

    #[test]
    #[ignore = "이 PC에 실제로 설치한다(NABI_ENV_REAL=<도구 id>)"]
    fn real_install_puts_the_tool_on_path() {
        let Ok(id) = std::env::var("NABI_ENV_REAL") else {
            eprintln!("NABI_ENV_REAL 없음 — 건너뜀");
            return;
        };
        let tool = crate::envcat::TOOLS.iter().find(|t| t.id == id).expect("카탈로그에 없는 id");
        let has_winget = probe("winget");
        let script = install_script(tool, has_winget).expect("설치 통로가 없다");
        eprintln!("[통로] winget={has_winget}");
        let job = start_script(script, "시작".into());
        let mut last = String::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let p = job.lock().unwrap();
            if p.message != last {
                last = p.message.clone();
                eprintln!("[{:>3.0}%] {last}", p.fraction * 100.0);
            }
            if p.done {
                assert!(p.success, "설치 실패: {last}");
                break;
            }
        }
        // 설치 프로그램이 고친 PATH는 이미 뜬 프로세스에 저절로 오지 않는다.
        crate::envpath::refresh();
        assert!(probe(tool.probe), "설치는 성공했다는데 {} 가 PATH에 없다", tool.probe);
    }

    fn probe(name: &str) -> bool {
        crate::aicli::hidden("where.exe").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
    }
}
