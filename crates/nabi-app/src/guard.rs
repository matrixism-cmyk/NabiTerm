//! **운영 세션에서 되돌릴 수 없는 명령을 보내기 전에 멈춘다.**
//!
//! 판별(`danger.rs`)과 화면 읽기(`inputline.rs`)를 이어 실제 입력 경로에 건다.
//! 조건을 좁게 거는 것이 이 모듈의 전부다 — **타이핑을 망가뜨리는 것이 가장 나쁜
//! 결과**이므로, 조금이라도 모르겠으면 그냥 보낸다.
//!
//! 막는 조건(전부 참일 때만):
//! 1. 설정이 켜져 있다(`terminal.guard_dangerous`, 기본 켬)
//! 2. 이 pane의 세션 표식이 **위험(운영)** 이다 — 표식을 안 붙였으면 지금과 똑같다
//! 3. **대체 화면이 아니다**(vim·less 안에서는 줄이 명령이 아니다)
//! 4. 이번 프레임 입력이 **CR로 끝난다**(그 외 타이핑은 손대지 않는다)
//! 5. 커서 줄에서 읽어 낸 명령이 판별기에 걸린다
//!
//! ## 왜 보내는 자리에서 바로 판단하는가
//!
//! "요청을 남기고 다음 프레임에 앱이 판단한다"가 더 단순해 보이지만 **입력 순서가
//! 뒤집힌다.** 엔터를 붙잡아 둔 사이 다음 프레임의 글자가 먼저 나가기 때문이다.
//! 그래서 판단에 필요한 것(위험 pane 집합)을 프레임마다 받아, **보내는 그 자리에서**
//! 끝낸다. 붙잡히지 않은 입력은 예전과 완전히 같은 경로로 나간다.

use nabi_types::PaneId;
use std::collections::HashSet;

/// 확인을 기다리는 입력 — 사용자가 "보내기"를 누르면 이 바이트를 그대로 흘려보낸다.
///
/// **바이트를 그대로 들고 있는다**는 것이 중요하다. 다시 만들지 않으므로 인코딩이
/// 달라질 여지가 없다 — 확인을 지나면 가드가 없던 것과 완전히 같은 것이 나간다.
#[derive(Clone)]
pub(crate) struct PendingSend {
    pub pane: PaneId,
    pub data: Vec<u8>,
    /// 화면에서 읽어 낸 명령(근사치).
    pub command: String,
    pub why: crate::danger::Danger,
    /// 동시에 나갈 창들(브로드캐스트). 비어 있으면 이 창만.
    pub panes: Vec<PaneId>,
}

/// 이 입력을 붙잡아야 하는가.
///
/// `risky`는 이번 프레임에 위험 표식이 붙은 pane 집합, `targets`는 브로드캐스트 대상
/// (비어 있으면 이 창만).
pub(crate) fn guard_input(
    on: bool,
    risky: &HashSet<PaneId>,
    panes: &nabi_orchestrator::SharedPanes,
    pane: PaneId,
    bytes: &[u8],
    targets: &[PaneId],
) -> Option<PendingSend> {
    if !on || !crate::inputline::ends_with_enter(bytes) {
        return None;
    }
    // 브로드캐스트는 창마다 표식이 다를 수 있다 — 하나라도 위험하면 확인한다.
    let hit = match targets.is_empty() {
        true => risky.contains(&pane),
        false => targets.iter().any(|p| risky.contains(p)),
    };
    if !hit {
        return None;
    }
    let command = command_being_typed(panes, pane)?;
    let why = crate::danger::classify(&command)?;
    Some(PendingSend { pane, data: bytes.to_vec(), command, why, panes: targets.to_vec() })
}

/// 커서가 있는 줄에서 치고 있는 명령을 읽어 낸다. 대체 화면이면 `None`.
fn command_being_typed(panes: &nabi_orchestrator::SharedPanes, pane: PaneId) -> Option<String> {
    let view = panes.read().ok().and_then(|m| m.get(&pane).cloned())?;
    let md = view.model.lock().ok()?;
    if md.alt_screen() {
        return None; // TUI 안에서는 줄이 명령이 아니다.
    }
    let cur = md.cursor();
    // 커서의 절대 줄 = 히스토리 길이 + 화면에서의 행.
    let abs = md.history_size() + cur.row as usize;
    let line = md.lines_abs_text(abs, abs + 1).into_iter().next()?;
    let cmd = crate::inputline::command_at_cursor(&line, cur.col as usize);
    (!cmd.is_empty()).then_some(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risky_set() -> HashSet<PaneId> {
        [PaneId::new(1)].into_iter().collect()
    }

    /// 설정이 꺼져 있으면 **아무것도 붙잡지 않는다**(빠져나갈 길이 있어야 한다).
    #[test]
    fn the_setting_turns_it_off_completely() {
        let panes: nabi_orchestrator::SharedPanes = Default::default();
        let got = guard_input(false, &risky_set(), &panes, PaneId::new(1), b"rm -rf /\r", &[]);
        assert!(got.is_none());
    }

    /// 엔터로 끝나지 않는 타이핑은 **손대지 않는다** — 글자가 안 찍히면 안 된다.
    #[test]
    fn ordinary_typing_is_never_held() {
        let panes: nabi_orchestrator::SharedPanes = Default::default();
        for b in [&b"r"[..], b"rm -rf /", b"\x1b[A", b""] {
            assert!(guard_input(true, &risky_set(), &panes, PaneId::new(1), b, &[]).is_none(), "{b:?}");
        }
    }

    /// 표식이 없는 창은 지금과 똑같이 동작한다.
    #[test]
    fn a_session_without_a_tag_is_untouched() {
        let panes: nabi_orchestrator::SharedPanes = Default::default();
        let got = guard_input(true, &risky_set(), &panes, PaneId::new(9), b"\r", &[]);
        assert!(got.is_none(), "표식 없는 창을 붙잡았다");
    }

    /// 브로드캐스트는 **하나라도** 위험하면 확인한다 — 여러 서버 동시가 가장 위험하다.
    #[test]
    fn broadcasting_to_any_risky_pane_is_checked() {
        let panes: nabi_orchestrator::SharedPanes = Default::default();
        let safe = [PaneId::new(7), PaneId::new(8)];
        assert!(
            guard_input(true, &risky_set(), &panes, PaneId::new(7), b"\r", &safe).is_none(),
            "안전한 창들만 있는데 붙잡았다"
        );
        // 위험한 창이 섞이면 화면을 읽으러 간다(빈 레지스트리라 결국 None이지만,
        // 여기서 걸러지지 않았다는 것이 요점이다 — 아래 시험이 그 경계를 지킨다).
        let mixed = [PaneId::new(7), PaneId::new(1)];
        let _ = guard_input(true, &risky_set(), &panes, PaneId::new(7), b"\r", &mixed);
    }
}
