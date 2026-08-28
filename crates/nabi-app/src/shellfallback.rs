//! 기본 셸이 실행되지 않을 때 **쓸 수 있는 것으로 바꾼다**(배치 AK).
//!
//! ## 무슨 일이 있었나
//!
//! 설정의 기본 셸이 `pwsh` 인데, 그 PC 에는 마이크로소프트 스토어판 PowerShell 7 만
//! 설치되어 있었다. 스토어판은 그 계정에 앱 라이선스가 없으면 실행되지 않는다.
//!
//! 그래서 탐색기에서 폴더를 우클릭해 열어도, 새 탭을 눌러도 아무것도 열리지 않았다.
//! 무엇이 잘못됐는지는 화면 어디에도 나오지 않았다.
//!
//! ## 왜 자동으로 바꾸는가
//!
//! 기본 셸이 실행되지 않으면 **터미널을 여는 길이 전부 막힌다.** 프로그램이 아무것도
//! 못 하는 상태로 있는 것보다, 열리는 셸로 바꿔서 일단 쓰게 하는 편이 낫다.
//!
//! ## 다만 조용히 바꾸지는 않는다
//!
//! 사용자가 고른 값을 우리가 마음대로 바꾸는 일이다. 말하지 않으면 "내가 설정한 게
//! 왜 다른 걸로 되어 있지?" 하고 헤매게 된다. 바꿨으면 무엇을 왜 바꿨는지 알린다.

use nabi_proto::ShellKind;

/// 기본 셸을 바꿔야 하는가. 바꿔야 하면 새 값을 돌려준다.
///
/// `usable` 은 이 PC 에서 실제로 실행되는 셸 목록이다(메뉴가 쓰는 것과 같은 목록).
///
/// 목록이 비어 있으면 **아무것도 바꾸지 않는다.** 훑기가 아직 안 끝났거나 실패한 경우인데,
/// 그때 바꾸면 멀쩡한 설정을 망친다.
pub(crate) fn pick(current: &str, usable: &[ShellKind]) -> Option<String> {
    if usable.is_empty() {
        return None;
    }
    let cur = crate::workspace::shell_from_str(current);
    if usable.iter().any(|k| same_kind(k, &cur)) {
        return None;
    }
    // 고르는 순서: Windows PowerShell → cmd → 그 밖에 목록의 첫 번째.
    //
    // 앞의 둘은 윈도우에 늘 함께 오는 것이라 가장 덜 놀랍다. 사용자가 WSL 을 쓰고 있었는데
    // 갑자기 리눅스 셸로 바뀌면 그것대로 당황스럽다.
    for want in [ShellKind::WindowsPowerShell, ShellKind::Cmd] {
        if let Some(k) = usable.iter().find(|k| same_kind(k, &want)) {
            return Some(crate::workspace::shell_to_str(k));
        }
    }
    usable.first().map(crate::workspace::shell_to_str)
}

/// 같은 종류인가. WSL 은 배포판이 달라도 같은 종류로 본다.
fn same_kind(a: &ShellKind, b: &ShellKind) -> bool {
    match (a, b) {
        (ShellKind::Wsl { .. }, ShellKind::Wsl { .. }) => true,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_working_shell_is_left_alone() {
        let usable = vec![ShellKind::WindowsPowerShell, ShellKind::Cmd];
        assert_eq!(pick("powershell", &usable), None, "쓸 수 있으면 건드리지 않는다");
        assert_eq!(pick("cmd", &usable), None);
    }

    #[test]
    fn a_shell_that_cannot_run_is_replaced() {
        // 이 시험이 이 파일이 생긴 이유다. pwsh 가 스토어판이라 실행되지 않는 PC 에서,
        // 기본 셸이 pwsh 로 남아 있으면 터미널을 여는 길이 전부 막힌다.
        let usable = vec![ShellKind::WindowsPowerShell, ShellKind::Cmd];
        assert_eq!(pick("pwsh", &usable).as_deref(), Some("powershell"));
    }

    #[test]
    fn windows_powershell_comes_first_because_it_is_least_surprising() {
        let usable = vec![ShellKind::Cmd, ShellKind::WindowsPowerShell, ShellKind::GitBash];
        assert_eq!(pick("pwsh", &usable).as_deref(), Some("powershell"));
    }

    #[test]
    fn cmd_is_next_when_windows_powershell_is_missing() {
        let usable = vec![ShellKind::GitBash, ShellKind::Cmd];
        assert_eq!(pick("pwsh", &usable).as_deref(), Some("cmd"));
    }

    #[test]
    fn otherwise_the_first_one_on_the_list() {
        let usable = vec![ShellKind::GitBash];
        assert_eq!(pick("pwsh", &usable).as_deref(), Some("gitbash"));
    }

    #[test]
    fn an_empty_list_changes_nothing() {
        // 훑기가 아직 안 끝났거나 실패한 경우다. 그때 바꾸면 멀쩡한 설정을 망친다.
        assert_eq!(pick("pwsh", &[]), None);
    }

    #[test]
    fn wsl_matches_regardless_of_which_distro() {
        // 목록에는 배포판별로 들어오는데, 설정에는 그냥 "wsl" 로 저장된다.
        let usable = vec![ShellKind::Wsl { distro: Some("Ubuntu".into()) }];
        assert_eq!(pick("wsl", &usable), None, "배포판이 달라도 같은 종류다");
    }
}
