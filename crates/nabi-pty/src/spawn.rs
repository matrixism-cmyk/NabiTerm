//! ShellKind → portable-pty CommandBuilder 변환.

use nabi_proto::ShellKind;
use portable_pty::CommandBuilder;
use std::path::{Path, PathBuf};

/// 셸 종류의 실행파일 이름(존재 확인·에러 메시지용).
pub fn program_name(shell: &ShellKind) -> String {
    match shell {
        ShellKind::Pwsh => "pwsh.exe".into(),
        ShellKind::WindowsPowerShell => "powershell.exe".into(),
        ShellKind::Cmd => "cmd.exe".into(),
        ShellKind::GitBash => "bash.exe".into(),
        ShellKind::Wsl { .. } => "wsl.exe".into(),
        ShellKind::Custom { program, .. } => program.clone(),
    }
}

/// 실행파일을 PATH(+절대경로)에서 찾는다(`where`/`Get-Command` 상당). 없으면 None.
/// 스폰 전에 호출해, 없는 셸이면 ConPTY 행(hang)/타임아웃 대신 즉시 명확한 에러를 낸다.
pub fn resolve_program(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.is_absolute() {
        return p.is_file().then(|| p.to_path_buf());
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let cand = dir.join(program);
        if cand.is_file() {
            return Some(cand);
        }
        // 확장자 없는 이름이면 .exe/.cmd/.bat도 시도(Windows PATHEXT 간이).
        if p.extension().is_none() {
            for ext in ["exe", "cmd", "bat"] {
                let c = dir.join(format!("{program}.{ext}"));
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// 이 경로가 **Microsoft Store 앱 실행 별칭**인가.
///
/// 스토어판 앱은 `%LOCALAPPDATA%\Microsoft\WindowsApps\`에 0바이트 재분석 지점을 놓는다.
/// 파일처럼 보여서 존재 확인은 통과하지만 진짜 실행 파일이 아니다. 그 계정에 앱
/// 라이선스가 없으면 실행 순간 `0xC0E90002`("관련 앱 라이선스를 찾을 수 없습니다")로
/// 죽는다 — 목록에는 뜨는데 안 열리는 이유다(사용자 보고 2026-08-26).
pub fn is_store_alias(path: &Path) -> bool {
    let zero = std::fs::metadata(path).map(|m| m.len() == 0).unwrap_or(false);
    zero && path.to_string_lossy().to_ascii_lowercase().contains(r"\microsoft\windowsapps\")
}

/// PowerShell 7(pwsh.exe) 위치 — **정식 설치본을 먼저** 찾는다.
///
/// PATH만 훑으면 스토어 별칭이 먼저 걸리는 PC가 있다. 정식 설치본이 함께 있는데도
/// 열리지 않는 일이 생기므로, 표준 설치 경로를 앞에 둔다.
fn pwsh_path() -> Option<PathBuf> {
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        let Some(base) = std::env::var_os(var) else { continue };
        for sub in [r"PowerShell\7\pwsh.exe", r"Microsoft\PowerShell\7\pwsh.exe"] {
            let p = Path::new(&base).join(sub);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 정식 설치본이 없으면 PATH(스토어 별칭 포함)를 쓴다 — 대부분의 PC에서는 이것도 열린다.
    resolve_program("pwsh.exe")
}

/// Git Bash의 bash.exe 위치 — PATH에 없으면 기본 설치 경로(Program Files\Git…)를 찾는다.
/// Git 설치 시 bash.exe가 PATH에 없는 경우가 흔해, 이를 보완해야 실제로 띄울 수 있다.
fn git_bash_path() -> Option<PathBuf> {
    if let Some(p) = resolve_program("bash.exe") {
        return Some(p);
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            for sub in ["Git\\bin\\bash.exe", "Git\\usr\\bin\\bash.exe"] {
                let p = Path::new(&base).join(sub);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 설치된 WSL 배포판 이름 목록(`wsl.exe --list --quiet`). 미설치/실패면 빈 목록.
/// 출력은 UTF-16LE(BOM 가능) — 콘솔 창이 깜빡이지 않게 CREATE_NO_WINDOW로 헤드리스 실행.
pub fn wsl_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .stdin(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) if o.status.success() => parse_wsl_list(&o.stdout),
        _ => Vec::new(),
    }
}

/// `wsl -l -q`의 UTF-16LE 바이트를 배포판 이름 벡터로 파싱(BOM·NUL·CR·빈 줄 제거).
fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let mut u16s: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    if u16s.first() == Some(&0xFEFF) {
        u16s.remove(0); // 선두 BOM 제거.
    }
    String::from_utf16_lossy(&u16s)
        .lines()
        .map(|l| l.trim().trim_matches('\0').trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// 셸의 실행파일 절대경로를 해석한다(없으면 None). 스폰 전 존재 확인·목록 필터·build_command 공용.
pub fn resolve_shell(shell: &ShellKind) -> Option<PathBuf> {
    match shell {
        ShellKind::GitBash => git_bash_path(),
        ShellKind::Pwsh => pwsh_path(),
        _ => resolve_program(&program_name(shell)),
    }
}

/// 셸 종류를 실행 가능한 CommandBuilder로 만든다.
pub fn build_command(shell: &ShellKind) -> CommandBuilder {
    match shell {
        ShellKind::Pwsh => {
            // 이름만 주면 PATH가 스토어 별칭을 먼저 집는 PC가 있다 — 찾은 경로를 그대로 쓴다.
            let exe = pwsh_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "pwsh.exe".into());
            let mut c = CommandBuilder::new(exe);
            // -NoLogo: 배너 생략. -ExecutionPolicy Bypass: 시스템 정책이 Restricted여도 사용자 프로파일을
            // 로드(프로세스 스코프 — 시스템 정책 불변). 미적용 시 매 셸 기동마다 프로파일 로드 에러 발생.
            c.args(["-NoLogo", "-ExecutionPolicy", "Bypass"]);
            c
        }
        ShellKind::WindowsPowerShell => {
            let mut c = CommandBuilder::new("powershell.exe");
            c.args(["-NoLogo", "-ExecutionPolicy", "Bypass"]);
            c
        }
        ShellKind::Cmd => CommandBuilder::new("cmd.exe"),
        ShellKind::GitBash => {
            // PATH에 없으면 기본 설치 경로의 절대 bash.exe를 쓴다(PATH 미등록 Git 지원).
            let bash = git_bash_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "bash.exe".into());
            let mut c = CommandBuilder::new(bash);
            c.arg("-l");
            c
        }
        ShellKind::Wsl { distro } => {
            let mut c = CommandBuilder::new("wsl.exe");
            if let Some(d) = distro {
                c.arg("-d");
                c.arg(d);
            }
            c
        }
        ShellKind::Custom { program, args } => {
            let mut c = CommandBuilder::new(program);
            for a in args {
                c.arg(a);
            }
            c
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_wsl_list, program_name};
    use nabi_proto::ShellKind;

    /// **0바이트 앱 실행 별칭**을 진짜 실행 파일로 세면, 목록에는 뜨는데 안 열린다.
    #[test]
    fn a_store_alias_is_recognised_for_what_it_is() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("nabi-alias-{}", std::process::id()));
        let apps = dir.join("Microsoft").join("WindowsApps");
        std::fs::create_dir_all(&apps).unwrap();
        // 스토어 별칭 흉내 — 0바이트에 WindowsApps 아래.
        let alias = apps.join("pwsh.exe");
        std::fs::File::create(&alias).unwrap();
        assert!(super::is_store_alias(&alias), "0바이트 WindowsApps 항목을 못 알아봤다");
        // 같은 이름이라도 내용이 있으면 진짜 실행 파일로 본다.
        let real = dir.join("pwsh.exe");
        std::fs::File::create(&real).unwrap().write_all(b"MZ").unwrap();
        assert!(!super::is_store_alias(&real));
        // WindowsApps 밖의 0바이트 파일도 별칭이 아니다.
        let empty = dir.join("empty.exe");
        std::fs::File::create(&empty).unwrap();
        assert!(!super::is_store_alias(&empty));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 없는 파일에 물어도 터지지 않아야 한다(탐지 경로에서 늘 불린다).
    #[test]
    fn asking_about_a_missing_file_is_safe() {
        assert!(!super::is_store_alias(std::path::Path::new(r"C:\nope\pwsh.exe")));
    }
    #[test]
    fn program_names_per_shell() {
        assert_eq!(program_name(&ShellKind::Pwsh), "pwsh.exe");
        assert_eq!(program_name(&ShellKind::WindowsPowerShell), "powershell.exe");
        assert_eq!(program_name(&ShellKind::Cmd), "cmd.exe");
        assert_eq!(program_name(&ShellKind::GitBash), "bash.exe");
        assert_eq!(program_name(&ShellKind::Wsl { distro: None }), "wsl.exe");
        // Custom은 지정한 program을 그대로 사용.
        assert_eq!(program_name(&ShellKind::Custom { program: "zsh".into(), args: vec![] }), "zsh");
    }

    /// UTF-16LE 인코딩한 문자열을 바이트로(테스트 입력 생성).
    fn utf16le(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    #[test]
    fn parses_bom_and_trims() {
        let bytes = utf16le("\u{FEFF}Ubuntu\r\nDebian\r\n\r\n");
        assert_eq!(parse_wsl_list(&bytes), vec!["Ubuntu".to_string(), "Debian".to_string()]);
    }

    #[test]
    fn empty_on_blank() {
        assert!(parse_wsl_list(&[]).is_empty());
        assert!(parse_wsl_list(&utf16le("\r\n  \r\n")).is_empty());
    }
}
