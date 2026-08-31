//! pane 스크롤백 옮기기 — 사람이 휠을 굴리는 것과 같은 일을 명령으로 한다.
//!
//! ## 왜 읽기(capture)로는 부족한가
//!
//! `capture` 는 **저장된 글**을 준다. 그런데 사용자가 겪는 것은 화면이다 — 스크롤했을 때
//! 무엇이 그려지는가는 그리는 자리까지 가 봐야 안다. 2026-08-30 "스크롤하면 글자가
//! 사라진다"는 보고를 받고, 재현할 방법이 없어서 만들었다. 스크롤해 놓고 `screenshot` 을
//! 찍으면 사람이 보는 것을 그대로 볼 수 있다.
//!
//! 에이전트에게도 쓸모가 있다. 긴 로그를 되짚을 때 `capture --start/--end` 는 글만 주지만,
//! 그림이 섞인 화면(인라인 이미지·표)은 눈으로 봐야 한다.

use crate::protocol::ControlResponse;
use nabi_orchestrator::SharedPanes;
use nabi_types::PaneId;

/// 그 pane 의 화면을 옮기고, 옮긴 뒤의 자리를 돌려준다.
///
/// 옮긴 **뒤의 자리를 돌려주는** 까닭은 부른 쪽이 끝에 닿았는지 알아야 하기 때문이다.
/// 위로 계속 굴리는 쪽은 언제 멈출지를 이 값으로 정한다(코어가 상한에서 클램프한다).
pub(crate) fn scroll(panes: &SharedPanes, pane: u64, lines: i32, to: &str) -> ControlResponse {
    let id = PaneId::new(pane);
    let Ok(map) = panes.read() else {
        return ControlResponse::Err { message: "pane 레지스트리 잠금 실패".into() };
    };
    let Some(v) = map.get(&id) else {
        return ControlResponse::Err { message: format!("pane {pane} 없음 — `list` 로 번호를 확인할 것") };
    };
    let Ok(mut m) = v.model.lock() else {
        return ControlResponse::Err { message: "pane 모델 잠금 실패".into() };
    };
    match to {
        "top" => m.scroll_to_top(),
        "bottom" => m.scroll_to_bottom(),
        _ => m.scroll_by(lines),
    }
    ControlResponse::Scrolled { offset: m.scrollback_offset() as u64, history: m.history_size() as u64 }
}
