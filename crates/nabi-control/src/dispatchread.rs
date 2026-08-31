//! 읽기 응답 구성(list/capture) — dispatch.rs에서 분리(라인 한도).

use crate::protocol::{ControlResponse, PaneInfo};
use nabi_orchestrator::SharedPanes;
use nabi_types::PaneId;
use nabi_proto::shell::ShellKind;

pub(crate) fn list_panes(panes: &SharedPanes) -> ControlResponse {
    let Ok(map) = panes.read() else {
        return err("pane 레지스트리 잠금 실패");
    };
    let mut out: Vec<PaneInfo> = map
        .iter()
        .map(|(id, v)| {
            let (cols, rows) = v.model.lock().map(|m| (m.size().cols(), m.size().rows())).unwrap_or((0, 0));
            // 제목 우선순위: 사용자/제어 지정 > OSC 제목 > 스폰 라벨.
            let t = v
                .user_title
                .lock()
                .ok()
                .and_then(|u| u.clone())
                .or_else(|| {
                    v.model.lock().ok().map(|m| m.title().to_string()).filter(|t| !t.is_empty())
                })
                .unwrap_or_else(|| v.title.clone());
            // CP-6: 메타(종류·cwd·활동 상태·exit) 동봉.
            let (kind, cwd, state, state_ms, last_exit) = v
                .meta
                .lock()
                .map(|m| {
                    (m.kind.to_string(), m.cwd.clone(), m.state().to_string(), m.state_ms(), m.last_exit)
                })
                .unwrap_or_else(|_| (String::new(), None, String::new(), 0, None));
            PaneInfo { id: id.get(), title: t, cols, rows, kind, cwd, state, state_ms, last_exit }
        })
        .collect();
    out.sort_by_key(|p| p.id);
    ControlResponse::Panes { panes: out }
}

pub(crate) fn capture(
    panes: &SharedPanes,
    pane: u64,
    lines: u32,
    start: Option<i64>,
    end: Option<i64>,
    escapes: bool,
    view: bool,
) -> ControlResponse {
    let Ok(map) = panes.read() else {
        return err("pane 레지스트리 잠금 실패");
    };
    let Some(v) = map.get(&PaneId::new(pane)) else {
        return err(&format!("pane {pane} 없음"));
    };
    let Ok(m) = v.model.lock() else {
        return err("모델 잠금 실패");
    };
    let cur = m.cursor();
    let lines = lines.clamp(1, 10_000) as usize;
    // 지금 보고 있는 화면 그대로 — 스크롤을 올려 뒀으면 그 자리의 줄들이다.
    let text = if view {
        let top = m.top_abs_line();
        let rows = m.size().rows() as usize;
        m.lines_abs_text(top, top + rows).join("\n")
    // CP-8: 범위/SGR 캡처 — 음수 인덱스는 끝(총 줄 수)에서 거꾸로.
    } else if start.is_some() || end.is_some() || escapes {
        let total = m.total_abs_lines() as i64;
        let abs = |v: i64| if v < 0 { total + v } else { v }.clamp(0, total) as usize;
        let e = end.map(abs).unwrap_or(total as usize);
        let s = start.map(abs).unwrap_or(e.saturating_sub(lines)).min(e);
        if escapes {
            m.lines_abs_sgr(s, e, &nabi_vt::Theme::default()).join("\n")
        } else {
            m.lines_abs_text(s, e).join("\n")
        }
    } else {
        m.dump_text(lines)
    };
    ControlResponse::Captured { pane, text, row: cur.row, col: cur.col, alt_screen: m.alt_screen() }
}

/// 설정 문자열 → ShellKind(미상은 기본 PowerShell). workspace의 shell_from_str과 동일 규칙.
pub(crate) fn shell_from_str(s: &str) -> ShellKind {
    match s.to_ascii_lowercase().as_str() {
        "pwsh" => ShellKind::Pwsh,
        "cmd" => ShellKind::Cmd,
        "wsl" => ShellKind::Wsl { distro: None },
        "gitbash" => ShellKind::GitBash,
        _ => ShellKind::WindowsPowerShell,
    }
}

pub(crate) fn err(m: &str) -> ControlResponse {
    ControlResponse::Err { message: m.to_string() }
}

/// **그 pane 이 실제로 있는가** — 없으면 돌려줄 오류를, 있으면 `None` 을 준다.
///
/// ## 왜 한 곳에 모으는가
///
/// `--pane` 을 받는 동사가 열 개가 넘는데, 확인하는 것과 안 하는 것이 섞여 있었다
/// (2026-09-01 점검: `resize`·`focus`·`set-title`·`progress`·`history` 다섯이 안 했다).
/// 확인하지 않으면 없는 번호에도 성공을 돌려주고, 부른 쪽은 된 줄 안다.
///
/// 자리마다 적으면 새 동사에서 또 빠진다. 문구도 갈라진다 — 실제로 `scroll` 만 영어로
/// "not found" 라고 답하고 있었다.
///
/// 앱까지 물어볼 것도 없다. pane 목록은 여기 있으므로 **오가지 않고** 답할 수 있다.
/// **그 경로가 실제로 있는가** — 없으면 돌려줄 오류를, 있으면 `None` 을 준다.
///
/// `open-file`·`open-here`·`open-browser` 는 경로를 앱에 던지고 곧바로 성공을 돌려줬다.
/// 경로를 잘못 적으면 사람에게만 토스트가 뜨고 부른 쪽은 열린 줄 안다 — 에이전트는
/// 그다음 단계로 넘어가 버린다(2026-09-01 점검).
///
/// 앱에 물어볼 것 없이 여기서 본다. 제어 서버는 앱과 **같은 프로세스**라 상대 경로도
/// 앱이 보는 것과 같은 자리에서 풀린다 — 다른 프로세스였다면 이 검사가 오히려 틀렸을 것이다.
///
/// `want_dir` 이 참이면 폴더여야 한다(`open-here` 는 폴더에서 터미널을 연다).
pub(crate) fn no_such_path(path: &str, want_dir: bool) -> Option<ControlResponse> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Some(err(&format!("경로 없음: {path}")));
    }
    (want_dir && !p.is_dir()).then(|| err(&format!("폴더가 아님: {path}")))
}

pub(crate) fn no_such_pane(panes: &SharedPanes, pane: u64) -> Option<ControlResponse> {
    let known = panes
        .read()
        .map(|m| m.contains_key(&PaneId::new(pane)))
        .unwrap_or(false);
    (!known).then(|| err(&format!("pane {pane} 없음 — `list` 로 번호를 확인할 것")))
}

/// pane의 **터미널 모드**를 그대로 돌려준다 — "왜 휠이 안 돼요" 같은 물음에 추측 없이 답하려고.
///
/// 대체 화면·마우스 보고·DEC 1007·bracketed paste·커서 키 모드는 눈으로 볼 수 없는데,
/// 휠·붙여넣기·키 입력의 동작을 전부 좌우한다. 실제로 2026-08-25에 "예전엔 휠로 이전 내용이
/// 보였는데 안 보인다"는 보고를 받고, 앱이 무엇을 켰는지 몰라 한참 추측해야 했다.
/// 이 값이 있으면 한 번 물어보면 끝난다.
pub(crate) fn pane_modes(panes: &SharedPanes, pane: u64) -> ControlResponse {
    let Ok(map) = panes.read() else {
        return err("pane 레지스트리 잠금 실패");
    };
    let Some(v) = map.get(&nabi_types::PaneId(pane)) else {
        return err("그런 pane이 없습니다");
    };
    let Ok(m) = v.model.lock() else {
        return err("pane 모델 잠금 실패");
    };
    ControlResponse::Modes {
        pane,
        alt_screen: m.alt_screen(),
        mouse_on: m.mouse_on(),
        alt_scroll: m.alt_scroll(),
        bracketed_paste: m.bracketed_paste(),
        app_cursor: m.app_cursor(),
        kitty_keys: m.kitty_keys(),
        scrollback_lines: m.history_size(),
        scroll_offset: m.scrollback_offset(),
        scrollback_wipes: m.scrollback_wipes(),
    }
}
