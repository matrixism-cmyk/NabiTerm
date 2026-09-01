//! 팔레트 항목 옆에 보여 줄 **단축키**.
//!
//! 팔레트는 명령을 찾는 자리이면서 **단축키를 배우는 자리**이기도 하다. VS Code·Sublime이
//! 오른쪽에 조합을 적어 두는 이유가 그것이다 — 몇 번 쓰다 보면 팔레트를 안 열게 된다.
//!
//! ## 조합 문자열을 여기 적지 않는다
//!
//! 도움말의 단축키 표(`helppages::KEYS`)에서 **가져온다.** 처음엔 여기에도 조합을 적고
//! 시험으로 동일성을 지키려 했는데, 그 시험이 곧바로 어긋남을 잡았다("Ctrl + -"가 도움말
//! 표에는 "Ctrl + =  /  -  /  0" 한 줄로 들어 있었다). 지킬 수 있는 규칙보다 **어길 수 없는
//! 구조**가 낫다 — 이제 두 곳이 다를 방법이 없다.

use crate::palette::PaletteAction as A;

/// 이 동작이 도움말 표의 어느 줄에 해당하는가.
///
/// 한 줄이 여러 키를 함께 설명하는 경우(글꼴 크기 `=`/`-`/`0`)는 넣지 않는다 — 팔레트
/// 한 줄에 세 조합을 적으면 오히려 읽기 어렵다.
fn help_key(a: &A) -> Option<&'static str> {
    Some(match a {
        A::DuplicateTab => "help.key.dup",
        A::SelectAll => "help.key.selectall",
        A::ZoomPane => "help.key.zoom",
        A::ToggleBroadcast => "help.key.broadcast",
        A::ClearBuffer => "help.key.clear",
        A::ToggleStatusBar => "help.key.statusbar",
        A::QuickConnect => "help.key.connect",
        A::OpenBrowserTab => "help.key.browser",
        // 이 둘은 팔레트에 새로 올린 것이다. 팔레트에서 한두 번 쓰다 단축키를 익히는 것이
        // 이 자리의 목적이므로, 올렸으면 조합도 같이 보여 준다.
        A::FindInPane => "help.key.find",
        A::Fullscreen => "help.key.fullscreen",
        _ => return None,
    })
}

/// 이 동작에 단축키가 있으면 그 조합(도움말 표의 문자열 그대로). 없으면 None.
pub(crate) fn accel(a: &A) -> Option<&'static str> {
    let want = help_key(a)?;
    crate::helppages::KEYS.iter().find(|(_, k)| *k == want).map(|(combo, _)| *combo)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 가리키는 도움말 줄이 실제로 있어야 한다 — 없으면 팔레트에 조합이 사라진다.
    #[test]
    fn every_mapped_action_resolves_to_a_real_help_row() {
        let all = [
            A::DuplicateTab, A::SelectAll, A::ZoomPane, A::ToggleBroadcast,
            A::ClearBuffer, A::ToggleStatusBar, A::QuickConnect, A::OpenBrowserTab,
        ];
        for a in &all {
            let key = help_key(a).expect("이 목록의 동작에는 도움말 줄이 있다");
            assert!(accel(a).is_some(), "도움말 표에 {key} 줄이 없다");
        }
    }

    /// 단축키가 없는 동작은 조용히 None — 팔레트가 빈 칸을 그리지 않게.
    #[test]
    fn actions_without_a_shortcut_return_none() {
        assert!(accel(&A::FindAll).is_none());
        assert!(accel(&A::OpenSettings).is_none());
    }

    /// 조합은 도움말 표에서 **그대로** 온다(우리가 따로 적지 않는다).
    #[test]
    fn the_string_comes_straight_from_the_help_table() {
        let from_table = crate::helppages::KEYS
            .iter()
            .find(|(_, k)| *k == "help.key.zoom")
            .map(|(c, _)| *c);
        assert_eq!(accel(&A::ZoomPane), from_table);
    }
}
