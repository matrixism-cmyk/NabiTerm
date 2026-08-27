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
    /// 기록을 시작한 순간. `.cast` 의 경과초는 이 시각을 기준으로 잰다.
    pub began: Instant,
    /// asciinema `.cast` 로 남기는가(설정). 꺼져 있으면 지금까지처럼 줄만 적는다.
    pub cast: bool,
    /// 기록 중인 파일 경로. 멈출 때 **되읽어 확인**하는 데 쓴다.
    pub path: std::path::PathBuf,
}

impl NabiApp {
    /// 포커스 pane의 세션 로깅을 토글한다(켜기=파일 선택 후 이후 출력 기록, 끄기=중지).
    pub(crate) fn toggle_session_log(&mut self) {
        let Some(pane) = self.focused_pane() else { return };
        if let Some(log) = self.session_logs.remove(&pane) {
            // 멈추는 김에 **방금 쓴 것을 되읽는다.** 기록이 못 읽히는 파일이었다는 사실을
            // 나중에 재생하려는 순간에 알게 되면 그때는 이미 늦다 — 그 자리에서 확인한다.
            let msg = match log.cast {
                true => self.verify_cast(&log.path),
                false => tr(self.lang, "log.stopped").to_string(),
            };
            self.notify = Some((msg, Instant::now()));
            return;
        }
        let Some(path) = rfd::FileDialog::new().set_file_name("session.log").save_file() else { return };
        self.start_session_log(pane, &path);
    }

    /// 그 pane의 로그를 이 파일로 시작한다. 손으로 켜는 길과 **자동으로 켜지는 길**이
    /// 여기서 만난다 — 시작 방법이 둘이면 한쪽만 고쳐지는 일이 생긴다.
    pub(crate) fn start_session_log(&mut self, pane: PaneId, path: &std::path::Path) {
        let Ok(mut file) = std::fs::File::create(path) else { return };
        let last = self.pane_marker(pane);
        // 확장자가 .cast 면 설정과 무관하게 그 형식으로 남긴다 — 사용자가 파일 이름으로
        // 이미 뜻을 밝혔는데 설정이 그것을 뒤집으면 놀랄 일이다.
        let by_ext = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("cast"));
        let cast = by_ext || self.config.terminal.session_log_cast;
        if cast {
            let (cols, rows) = self.pane_size(pane);
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "{}", crate::sessioncast::header(cols, rows, secs));
        }
        self.session_logs.insert(
            pane,
            SessionLog { file, last, began: Instant::now(), cast, path: path.to_path_buf() },
        );
        self.notify = Some((tr(self.lang, "log.started").to_string(), Instant::now()));
    }

    /// 방금 쓴 `.cast` 를 되읽어 몇 개 사건·몇 초인지 알려 준다.
    ///
    /// 이것이 되읽기 코드의 첫 사용처다. 읽을 수 있음을 **그 자리에서** 보여 주는 것이
    /// 요점이라, 못 읽으면 못 읽는다고 말한다 — 조용히 넘어가면 확인한 뜻이 없다.
    fn verify_cast(&self, path: &std::path::Path) -> String {
        let Ok(text) = std::fs::read_to_string(path) else {
            return tr(self.lang, "log.stopped").to_string();
        };
        let ev = crate::sessioncastread::parse_cast(&text);
        match ev.is_empty() {
            true => tr(self.lang, "log.cast.empty").to_string(),
            false => format!(
                "{} ({}, {:.0}s)",
                tr(self.lang, "log.stopped"),
                ev.len(),
                crate::sessioncastread::duration(&ev)
            ),
        }
    }

    /// pane 터미널의 현재 크기(열, 행). 모델이 없으면 흔한 기본값.
    ///
    /// `.cast` 머리글에 넣는다 — 재생기가 이 크기로 화면을 잡아야 줄바꿈이 원본과 같아진다.
    fn pane_size(&self, pane: PaneId) -> (u16, u16) {
        self.orch.panes.read().ok()
            .and_then(|m| m.get(&pane).cloned())
            .and_then(|v| v.model.lock().ok().map(|md| { let s = md.size(); (s.cols(), s.rows()) }))
            .unwrap_or((80, 24))
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
                    match log.cast {
                        // 시각을 함께 적는다. 줄 단위라 도중에 죽어도 거기까지가 유효하다.
                        true => {
                            let el = log.began.elapsed().as_secs_f64();
                            let _ = writeln!(log.file, "{}", crate::sessioncast::event(el, &format!("{joined}\r\n")));
                        }
                        false => {
                            let _ = writeln!(log.file, "{joined}");
                        }
                    }
                    log.last = cur;
                }
            }
            true
        });
    }
}
