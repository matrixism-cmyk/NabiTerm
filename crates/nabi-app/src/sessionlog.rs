//! 세션 로깅 — 터미널 출력을 파일로 자동 기록(MobaXterm/PuTTY식). 시작 시점 이후의 신규 줄만
//! line_marker 델타로 추출해 매 프레임 append한다(스크롤백 한도와 무관, 기존 tail 패턴 재사용).

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;
use std::io::Write;
use std::time::Instant;

/// 한 pane의 활성 로그(파일 핸들 + 마지막으로 기록한 절대 줄 마커).
pub(crate) struct SessionLog {
    pub file: std::fs::File,
    pub last: usize,
}

impl NabiApp {
    /// 포커스 pane의 세션 로깅을 토글한다(켜기=파일 선택 후 이후 출력 기록, 끄기=중지).
    pub(crate) fn toggle_session_log(&mut self) {
        let Some(pane) = self.focused_pane() else { return };
        if self.session_logs.remove(&pane).is_some() {
            self.notify = Some((tr(self.lang, "log.stopped").to_string(), Instant::now()));
            return;
        }
        let Some(path) = rfd::FileDialog::new().set_file_name("session.log").save_file() else { return };
        self.start_session_log(pane, &path);
    }

    /// 그 pane의 로그를 이 파일로 시작한다. 손으로 켜는 길과 **자동으로 켜지는 길**이
    /// 여기서 만난다 — 시작 방법이 둘이면 한쪽만 고쳐지는 일이 생긴다.
    pub(crate) fn start_session_log(&mut self, pane: PaneId, path: &std::path::Path) {
        let Ok(file) = std::fs::File::create(path) else { return };
        let last = self.pane_marker(pane);
        self.session_logs.insert(pane, SessionLog { file, last });
        self.notify = Some((tr(self.lang, "log.started").to_string(), Instant::now()));
    }

    /// pane 모델의 현재 절대 줄 마커(없으면 0).
    fn pane_marker(&self, pane: PaneId) -> usize {
        self.orch.panes.read().ok()
            .and_then(|m| m.get(&pane).cloned())
            .and_then(|v| v.model.lock().ok().map(|md| md.line_marker()))
            .unwrap_or(0)
    }

    /// 매 프레임: 활성 로그마다 신규 줄을 파일에 append. pane이 사라지면 로그 종료.
    pub(crate) fn flush_session_logs(&mut self) {
        if self.session_logs.is_empty() {
            return;
        }
        let redact_on = self.config.terminal.redact_logs;
        let Some(panes) = self.orch.panes.read().ok() else { return };
        self.session_logs.retain(|pane, log| {
            let Some(v) = panes.get(pane) else { return false };
            if let Ok(md) = v.model.lock() {
                let cur = md.line_marker();
                if cur > log.last {
                    let lines = md.lines_abs_text(log.last, cur);
                    // 터미널 출력에는 붙여넣은 토큰·명령줄 비밀번호가 지나간다. 파일에
                    // 닿기 전에 가린다(설정으로 끌 수 있다 — 원문이 필요한 진단도 있다).
                    let joined = match redact_on {
                        true => lines.iter().map(|l| crate::redact::line_full(l)).collect::<Vec<_>>().join("\r\n"),
                        false => lines.join("\r\n"),
                    };
                    let _ = writeln!(log.file, "{joined}");
                    log.last = cur;
                }
            }
            true
        });
    }
}
