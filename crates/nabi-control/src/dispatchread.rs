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
    // CP-8: 범위/SGR 캡처 — 음수 인덱스는 끝(총 줄 수)에서 거꾸로.
    let text = if start.is_some() || end.is_some() || escapes {
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
