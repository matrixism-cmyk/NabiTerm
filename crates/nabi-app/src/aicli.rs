//! AI CLI 설치 탐지와 공식 설치 관리자 실행.

use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub(crate) struct CliStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
}

impl CliStatus {
    pub fn installed(&self) -> bool {
        self.path.is_some()
    }
}

pub(crate) fn detect_all() -> Vec<CliStatus> {
    [
        ("claude", "Claude Code", "claude"),
        ("codex", "OpenAI Codex", "codex"),
        ("antigravity", "Google Antigravity", "agy"),
    ]
    .into_iter()
    .map(|(id, name, command)| detect(id, name, command))
    .collect()
}

fn detect(id: &'static str, name: &'static str, command: &'static str) -> CliStatus {
    let path = resolve(command);
    let version = path.as_ref().and_then(|p| {
        hidden(p)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let s = if o.stdout.is_empty() {
                    &o.stderr
                } else {
                    &o.stdout
                };
                String::from_utf8_lossy(s)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
    });
    CliStatus {
        id,
        name,
        command,
        path,
        version,
    }
}

fn resolve(command: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in ["exe", "cmd", "bat"] {
            let p = dir.join(format!("{command}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(program);
    c.creation_flags(0x0800_0000).stdin(Stdio::null());
    c
}

/// 공식 패키지 관리자/설치 프로그램을 사용한다. 진행과 오류를 사용자가 볼 수 있도록
/// 별도 PowerShell 창을 유지한다.
pub(crate) fn launch_action(id: &str, remove: bool) -> std::io::Result<()> {
    let cmd = match (id, remove) {
        ("claude", false) => "winget install --id Anthropic.ClaudeCode -e --source winget",
        ("claude", true) => "winget uninstall --id Anthropic.ClaudeCode -e",
        ("codex", false) => "npm install -g @openai/codex@latest",
        ("codex", true) => "npm uninstall -g @openai/codex",
        ("antigravity", false) =>
            "$p=Join-Path $env:TEMP 'antigravity-install.ps1'; Invoke-WebRequest https://antigravity.google/cli/install.ps1 -OutFile $p; & $p",
        // 공식 문서에 안정적인 제거 명령이 공개되지 않아 임의 파일 삭제를 하지 않는다.
        ("antigravity", true) => "Start-Process 'https://antigravity.google/download'",
        _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "unknown AI CLI")),
    };
    Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoExit",
            "-ExecutionPolicy",
            "RemoteSigned",
            "-Command",
            cmd,
        ])
        .spawn()
        .map(|_| ())
}
