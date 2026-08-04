//! 로컬 셸 종류(ConPTY 스폰 대상).

use serde::{Deserialize, Serialize};

/// 로컬 터미널이 띄울 수 있는 셸 종류.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ShellKind {
    /// PowerShell 7 (pwsh.exe)
    #[default]
    Pwsh,
    /// Windows PowerShell (powershell.exe)
    WindowsPowerShell,
    /// 명령 프롬프트 (cmd.exe)
    Cmd,
    /// WSL 배포판(미지정이면 기본 배포판)
    Wsl { distro: Option<String> },
    /// Git Bash
    GitBash,
    /// 사용자 지정 프로그램
    Custom { program: String, args: Vec<String> },
}


impl ShellKind {
    /// 사람이 읽는 표시 이름.
    pub fn label(&self) -> &str {
        match self {
            ShellKind::Pwsh => "PowerShell 7",
            ShellKind::WindowsPowerShell => "Windows PowerShell",
            ShellKind::Cmd => "Command Prompt",
            ShellKind::Wsl { .. } => "WSL",
            ShellKind::GitBash => "Git Bash",
            ShellKind::Custom { .. } => "Custom",
        }
    }
}
