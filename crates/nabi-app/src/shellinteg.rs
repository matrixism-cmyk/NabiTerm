//! PowerShell 프로필에 셸 통합 스니펫 설치.
//!
//! v3 이 넣는 것: OSC 133(명령 경계·종료코드), OSC 7(cwd), OSC 633;E(실행한 명령줄,
//! base64), 그리고 PSReadLine 프롬프트 자리 바로잡기.
//!
//! OSC 633;E는 PSConsoleHostReadLine을 감싸 사용자가 입력한 명령줄을 base64로 보고하며,
//! 워크스페이스 복원 시 "종료 직전 실행 중이던 명령"(claude 등) 재실행에 쓰인다.
//!
//! ## 프롬프트가 깨지던 것 (사용자 보고, 2026-09-06 실측)
//!
//! `PS C:\Users\Administrator>` 에서 `|` 나 `@` 를 치면 `PS C:\Users\Administrator|` 이
//! 됐다. 프롬프트의 `>` 가 입력한 글자로 덮였다.
//!
//! 재현해서 하나씩 갈랐다.
//!
//! * `cmd.exe` 는 멀쩡하다 → 우리 터미널 코어가 아니다.
//! * 우리 표식을 뺀 순수 프롬프트에서도 그렇다 → 이 스니펫의 OSC 때문이 아니다.
//! * 창 크기를 바꿔 **전체를 다시 그려도 그대로다** → 우리 화면이 어긋난 것이 아니라
//!   ConPTY 버퍼에 그렇게 박혀 있다. PSReadLine 이 그 자리에 쓴 것이다.
//! * 받은 바이트는 `ESC[1;26H|` — 프롬프트가 27칸인데 26칸에 쓰라고 온다. 두 칸 왼쪽이다.
//!
//! PSReadLine 이 세션 처음부터 "입력이 어디서 시작하는가"를 두 칸 왼쪽으로 잡고 있었다.
//! 평소에는 드러나지 않는다 — 글자를 이어 붙일 때는 지금 커서 자리에 쓰기 때문이다.
//! 그런데 `|` 나 `@` 는 그것만으로 **구문이 미완성**이라 PSReadLine 이 줄을 다시 칠하고,
//! 그때 잘못 잡아 둔 자리가 드러난다.
//!
//! `Set-PSReadLineOption -PromptText` 를 **한 번 부르면** 그 자리가 바로잡힌다. 값이
//! 무엇이든 상관없었다(빈 값·`'> '`·엉뚱한 값 모두 고쳐졌다) — 부른다는 사실이 중요하다.

/// 현재 버전 식별자(중복 설치 방지·업그레이드 판정). 가드 BEGIN/END에도 같은 문구.
const MARKER: &str = "nabi shell integration v3";

/// 프로필에 덧붙일 PowerShell 스니펫(5.1/7 공용). BEGIN/END 가드로 재설치 시 교체 가능.
fn snippet() -> &'static str {
    r#"# nabi shell integration v3 BEGIN
function prompt {
    $gle = $global:LASTEXITCODE
    $e = [char]27; $a = [char]7
    $p = $executionContext.SessionState.Path.CurrentLocation
    $path = ($p.ProviderPath -replace '\\','/')
    $global:LASTEXITCODE = $gle
    -join ("$e]133;D;$gle$a", "$e]7;file://localhost/$path$a", "$e]133;A$a",
        "PS $p$('>' * ($nestedPromptLevel + 1)) ", "$e]133;B$a")
}
if (Get-Command PSConsoleHostReadLine -CommandType Function -ErrorAction Ignore) {
    if (-not (Test-Path Function:\__nabiReadLine)) {
        Rename-Item Function:\PSConsoleHostReadLine __nabiReadLine
    }
    function global:PSConsoleHostReadLine {
        $c = __nabiReadLine
        if ($c) {
            $b = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($c))
            [Console]::Write("$([char]27)]633;E;$b$([char]7)")
        }
        $c
    }
}
# PSReadLine 이 잡아 둔 "입력 시작 자리"를 바로잡는다(위 설명 참고). 값보다 **부른다는
# 사실**이 중요하다. PSReadLine 이 없는 환경에서는 조용히 건너뛴다.
if (Get-Command Set-PSReadLineOption -ErrorAction Ignore) {
    Set-PSReadLineOption -PromptText '> '
}
# nabi shell integration v3 END
"#
}

/// 기존 nabi 블록을 제거한다 — 마커 줄부터 END 가드까지(v1처럼 가드가 없으면 파일 끝까지).
/// nabi 블록은 항상 파일 끝에 덧붙여지므로 EOF 폴백이 사용자 내용을 지우지 않는다.
fn strip_old_block(existing: &str) -> String {
    let lines: Vec<&str> = existing.lines().collect();
    let Some(start) = lines.iter().position(|l| l.contains("nabi shell integration")) else {
        return existing.to_string();
    };
    let end = lines
        .iter()
        .skip(start)
        .position(|l| l.contains("nabi shell integration") && l.contains("END"))
        .map(|rel| start + rel + 1)
        .unwrap_or(lines.len());
    let kept: Vec<&str> = lines[..start].iter().chain(lines[end..].iter()).copied().collect();
    kept.join("\n").trim_end().to_string()
}

/// PowerShell 5.1/7 프로필 경로(설치·설치확인 공용).
fn profile_paths() -> Vec<std::path::PathBuf> {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return Vec::new();
    };
    let docs = std::path::Path::new(&home).join("Documents");
    vec![
        docs.join("PowerShell").join("Microsoft.PowerShell_profile.ps1"), // pwsh 7
        docs.join("WindowsPowerShell").join("Microsoft.PowerShell_profile.ps1"), // 5.1
    ]
}

/// 셸 통합 v2가 어느 프로필에든 설치돼 있는지(설치 권장 판정용).
pub(crate) fn is_installed() -> bool {
    profile_paths()
        .iter()
        .any(|p| std::fs::read_to_string(p).map(|c| c.contains(MARKER)).unwrap_or(false))
}

/// PowerShell 5.1·7 프로필 모두에 스니펫을 설치/업그레이드한다(이미 v2면 건너뜀).
/// 성공 시 사람이 읽을 결과 요약을 돌려준다.
pub(crate) fn install() -> Result<String, String> {
    let targets = profile_paths();
    if targets.is_empty() {
        return Err("USERPROFILE 없음".into());
    }
    let (mut installed, mut skipped) = (0u32, 0u32);
    for path in &targets {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        if existing.contains(MARKER) {
            skipped += 1;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let cleaned = strip_old_block(&existing); // 이전 버전(v1 등) 블록 제거 후 교체.
        let body = format!("{cleaned}\n{}\n", snippet());
        std::fs::write(path, body).map_err(|e| e.to_string())?;
        installed += 1;
    }
    Ok(if installed == 0 {
        "이미 설치됨".to_string()
    } else {
        format!("{installed}개 프로필 설치({skipped} 기존) — 새 셸부터 적용")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn snippet_has_osc_marks() {
        let s = super::snippet();
        assert!(s.contains("133;D")); // 종료코드 보고.
        assert!(s.contains("133;A") && s.contains("133;B")); // 프롬프트 경계.
        assert!(s.contains("]7;file://")); // cwd 보고.
        assert!(s.contains("]633;E;")); // 명령줄 보고(v2).
        assert!(s.contains(super::MARKER)); // 중복 설치 방지 마커.
    }

    #[test]
    fn strip_removes_old_guarded_block() {
        let existing = "echo hi\n# nabi shell integration v2 BEGIN\nfunction prompt {}\n# nabi shell integration v2 END";
        assert_eq!(super::strip_old_block(existing), "echo hi");
    }

    #[test]
    fn strip_removes_unguarded_v1_to_eof() {
        let existing = "echo hi\n# nabi shell integration v1\nfunction prompt {\n}";
        assert_eq!(super::strip_old_block(existing), "echo hi");
    }
}
