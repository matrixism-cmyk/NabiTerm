//! 다른 프로그램에서 **가져올 것이 있는지 먼저 찾아본다** — 가져오기 한 화면의 알맹이.
//!
//! 임포터는 일곱 개나 있는데(PuTTY·MobaXterm·FileZilla·WinSCP·Xshell·ssh config·우리 형식)
//! 메뉴에 흩어져 있어, 무엇을 쓸 수 있는지 알려면 하나씩 눌러 봐야 했다(감사 2026-08-25).
//! 임포터의 목적은 **전환 장벽 제거**인데 찾기 어려우면 그 목적을 잃는다.
//!
//! 여기서는 설치 여부를 먼저 훑어 "이 PC에 있는 것"만 골라 보여 준다. 없는 것을 목록에
//! 채워 넣으면 눌러 보고 나서야 없다는 것을 알게 된다.
//!
//! **탐지는 세션 파일이 실제로 있는지로 한다** — 프로그램이 깔려 있어도 세션이 없으면
//! 가져올 것이 없고, 반대로 프로그램을 지웠어도 설정 파일은 남아 있어 가져올 수 있다.

use crate::menu::MenuAction;

/// 가져올 수 있는 곳 하나.
pub(crate) struct Source {
    /// 목록에 쓸 이름(제품명이라 번역하지 않는다).
    pub name: &'static str,
    /// 눌렀을 때 실행할 기존 가져오기 동작.
    pub action: MenuAction,
    /// 이 PC에서 찾았는가.
    pub found: bool,
    /// 찾았다면 어디에서(경로나 "레지스트리"). 못 찾았으면 빈 문자열.
    pub where_: String,
}

/// 모든 가져오기 원본을 훑는다. 찾은 것이 앞으로 온다.
pub(crate) fn scan() -> Vec<Source> {
    let mut v = vec![
        probe("PuTTY", MenuAction::ImportPuTTY, putty_where()),
        probe("WinSCP", MenuAction::ImportWinScp, winscp_where()),
        probe("MobaXterm", MenuAction::ImportMobaXterm, path_where(crate::mobaxterm::default_path())),
        probe("FileZilla", MenuAction::ImportFileZilla, path_where(crate::filezilla::default_path())),
        probe("Xshell", MenuAction::ImportXshell, path_where(crate::xshell::default_sessions_dir())),
        probe("OpenSSH config", MenuAction::ImportSshConfig, ssh_config_where()),
    ];
    // 찾은 것을 위로 — 눌러야 할 것이 먼저 보이게. 같은 무리 안에서는 원래 순서를 지킨다.
    v.sort_by_key(|s| !s.found);
    v
}

fn probe(name: &'static str, action: MenuAction, where_: Option<String>) -> Source {
    Source { name, action, found: where_.is_some(), where_: where_.unwrap_or_default() }
}

fn path_where(p: Option<std::path::PathBuf>) -> Option<String> {
    p.map(|p| p.display().to_string())
}

/// PuTTY는 세션을 레지스트리에 둔다 — 내보내 봐서 뭔가 나오면 있는 것이다.
fn putty_where() -> Option<String> {
    crate::putty::export_registry_text()
        .filter(|t| t.contains("Sessions"))
        .map(|_| "Windows 레지스트리".to_string())
}

/// WinSCP는 ini나 레지스트리 어느 한쪽 — 이미 둘 다 찾아보는 함수가 있다.
fn winscp_where() -> Option<String> {
    crate::winscp::find_config().map(|_| "WinSCP.ini / 레지스트리".to_string())
}

fn ssh_config_where() -> Option<String> {
    let p = crate::browser::home_dir().join(".ssh").join("config");
    p.is_file().then(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 찾은 것이 앞에 온다 — 눌러야 할 것이 먼저 보여야 한다.
    #[test]
    fn found_sources_are_listed_first() {
        let mut v = [
            Source { name: "b", action: MenuAction::ImportPuTTY, found: false, where_: String::new() },
            Source { name: "a", action: MenuAction::ImportWinScp, found: true, where_: "x".into() },
        ];
        v.sort_by_key(|s| !s.found);
        assert_eq!(v[0].name, "a");
    }

    /// 훑기는 **터지지 않아야 한다** — 아무것도 안 깔린 PC에서도 목록은 나와야 한다.
    #[test]
    fn scanning_an_empty_machine_still_lists_every_source() {
        let v = scan();
        assert_eq!(v.len(), 6, "원본 수가 줄면 사용자가 길을 잃는다");
        // 찾지 못한 것은 위치가 비어 있다(화면에서 흐리게 그린다).
        for s in &v {
            assert_eq!(s.found, !s.where_.is_empty());
        }
    }

    /// 찾은 것들이 목록 앞쪽에 몰려 있어야 한다(정렬이 실제로 걸렸는지).
    #[test]
    fn the_list_is_partitioned_by_found() {
        let v = scan();
        let first_missing = v.iter().position(|s| !s.found).unwrap_or(v.len());
        assert!(v[first_missing..].iter().all(|s| !s.found), "찾은 것이 뒤에 섞였다");
    }
}
