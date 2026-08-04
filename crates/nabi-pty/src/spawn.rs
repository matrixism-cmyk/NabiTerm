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
        _ => resolve_program(&program_name(shell)),
    }
}

/// 셸 종류를 실행 가능한 CommandBuilder로 만든다.
pub fn build_command(shell: &ShellKind) -> CommandBuilder {
    match shell {
        ShellKind::Pwsh => {
            let mut c = CommandBuilder::new("pwsh.exe");
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
