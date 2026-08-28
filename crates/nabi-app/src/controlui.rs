//! 제어 평면 승인 UI — ask 모드에서 외부 pane의 제어 요청을 사용자가 승인/거부.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// OSC 7771 in-band 제어 처리 — opt-in(control_allow_osc) + 로컬 pane 한정.
    /// fire-and-forget verb만 허용(질의류는 무시). json 본문은 {"path":..} 등 단순 객체.
    pub(crate) fn handle_control_osc(&mut self, pane: nabi_types::PaneId, verb: &str, json: &str) {
        if !self.config.terminal.control_allow_osc {
            tracing::warn!(target: "control", verb, "OSC 7771 무시(control_allow_osc=false)");
            return;
        }
        // 원격(SSH) pane이 보낸 OSC는 무조건 차단(출력 위장 방지).
        if matches!(self.pane_origins.get(&pane), Some(nabi_session::SessionKind::Ssh { .. })) {
            tracing::warn!(target: "control", verb, "원격 pane OSC 7771 차단");
            return;
        }
        let field = |k: &str| -> Option<String> {
            serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .and_then(|v| v.get(k).and_then(|x| x.as_str()).map(String::from))
        };
        tracing::info!(target: "control", verb, "OSC 7771 처리");
        match verb {
            "spawn" => {
                let shell = field("shell").unwrap_or_else(|| self.config.terminal.default_shell.clone());
                let s = crate::workspace::shell_from_str(&shell);
                self.spawn_local_cwd(s, None, field("cwd"));
            }
            "open-browser" => {
                let p = self.open_browser_tab();
                if let (Some(path), Some(b)) = (field("path"), self.browser_tabs.get_mut(&p)) {
                    b.path = std::path::PathBuf::from(path);
                }
            }
            "open-sftp" => {
                if let Some(name) = field("session") {
                    if let Some(s) = self.sessions.sessions.iter().find(|s| s.name == name).cloned() {
                        let ftp = s.is_ftp;
                        self.open_sftp_saved(s, ftp);
                    }
                }
            }
            // pane 커스텀 상태 키-값(AI 도구가 토큰/모델 등 발행 → 상태바·탭 표시).
            "status-set" => {
                if let (Some(k), Some(v)) = (field("key"), field("value")) {
                    self.set_pane_status(pane, k, Some(v));
                }
            }
            "status-clear" => self.set_pane_status(pane, field("key").unwrap_or_default(), None),
            other => tracing::warn!(target: "control", verb = other, "OSC 7771 미지원 verb"),
        }
    }

    /// pane 커스텀 상태를 설정/삭제한다(OSC·제어평면 공용 SSOT). value=None이면 삭제,
    /// key가 빈 문자열이면 그 pane 상태 전체를 비운다.
    pub(crate) fn set_pane_status(&mut self, pane: nabi_types::PaneId, key: String, value: Option<String>) {
        let state_key = key == "state" || key.is_empty(); // 전체 삭제도 state 제거를 포함.
        match value {
            Some(v) => {
                self.pane_status.entry(pane).or_default().insert(key, v);
            }
            None if key.is_empty() => {
                self.pane_status.remove(&pane);
                self.ctx_alert_on.remove(&pane);
            }
            None => {
                if let Some(m) = self.pane_status.get_mut(&pane) {
                    m.remove(&key);
                }
            }
        }
        // 컨텍스트 사용률 80% 진입 시 1회 경고(컴팩션 임박, idle→alert 전이만).
        let gauge = self.pane_status.get(&pane)
            .and_then(|m| m.get("tokens")).and_then(|t| crate::aistatus::parse_token_usage(t));
        let now_alert = crate::aistatus::context_alert(gauge, 0.8);
        if now_alert && !self.ctx_alert_on.insert(pane, now_alert).unwrap_or(false) {
            let pct = (gauge.unwrap_or(0.0) * 100.0) as u32;
            self.notify = Some((format!("\u{26a0} {pct}% {}", tr(self.lang, "ai.context.near")), std::time::Instant::now()));
        } else if !now_alert {
            self.ctx_alert_on.insert(pane, false);
        }
        // 훅이 발행한 상태 변화도 제어 평면에 합성 이벤트로 흘린다(agent wait/구독 — B1).
        // 발행 값(자유 문자열)을 표준 어휘로 접어 넣는다(agent_state와 같은 판정).
        if state_key {
            let st = self.pane_status.get(&pane)
                .map(|m| crate::aistatus::agent_state(m, self.cmd_start.contains_key(&pane)))
                .unwrap_or(0);
            let name = match st { 2 => "blocked", 1 => "working", _ => "idle" };
            self.control_events.publish(&nabi_proto::Event::AgentStatus { pane, state: name });
        }
        // 비포커스 에이전트가 "입력 대기(blocked)"로 전이하면 1회 토스트(멀티에이전트 — 입력 필요 알림).
        let blocked = self.pane_status.get(&pane)
            .is_some_and(|m| crate::aistatus::agent_state(m, false) == 2);
        if blocked && !self.blocked_alert.insert(pane, true).unwrap_or(false) {
            if self.focused_pane() != Some(pane) {
                let title = self.orch.panes.read().ok()
                    .and_then(|m| m.get(&pane).map(|v| v.title.clone())).unwrap_or_default();
                self.notify = Some((format!("\u{23f8} {title}: {}", tr(self.lang, "ai.waiting.input")), std::time::Instant::now()));
                if self.config.terminal.agent_sound {
                    crate::bell::system_beep(); // A7 — 화면 감지 경로와 동일 정책.
                }
            }
        } else if !blocked {
            self.blocked_alert.insert(pane, false);
        }
    }



    /// 매 프레임: 승인 요청을 받아 대기 표시하고, 승인/거부 다이얼로그를 그린다.
    /// verb 그룹(act/inject)별 별도 승인(CP-7) — 요청 그룹을 명시한다.
    pub(crate) fn show_control_approval(&mut self, ctx: &egui::Context) {
        // "켬"이면 아무것도 묻지 않는다.
        //
        // 정책은 이미 "켬"에서 전부 허용하므로 새 요청은 오지 않는다. 문제는 **묻는 모드일
        // 때 들어와 줄 서 있던 옛 요청**이다. 설정을 "켬"으로 바꿔도 그 요청이 채널에 남아
        // 있다가 뒤늦게 창을 띄웠다. 사용자가 보기에는 "켜 놨는데 한 번은 묻는" 것이 된다.
        //
        // 그래서 켬으로 바뀌면 줄 서 있던 것까지 비운다. 켬 상태에서는 허락을 받을 이유가
        // 없으니 버려도 잃는 것이 없다.
        if self.control_policy.mode() == nabi_control::policy::Mode::On {
            while self.control_ask_rx.try_recv().is_ok() {}
            self.control_pending = None;
            return;
        }
        // 새 승인 요청 수신(중복은 무시 — 첫 요청만 대기).
        if self.control_pending.is_none() {
            if let Ok(req) = self.control_ask_rx.try_recv() {
                self.control_pending = Some(req);
            }
        }
        let Some((pane, group)) = self.control_pending else { return };
        let lang = self.lang;
        let group_key = match group {
            nabi_control::policy::Group::Inject => "control.group.inject",
            _ => "control.group.act",
        };
        let (mut approve, mut deny) = (false, false);
        egui::Window::new(tr(lang, "control.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])
            .show(ctx, |ui| {
                let who = if pane == 0 {
                    tr(lang, "control.unknownpane").to_string()
                } else {
                    format!("pane #{pane}")
                };
                ui.label(format!("{} {}", who, tr(lang, "control.requests")));
                // 요청 verb 그룹 명시("pane 3이 입력 주입을 요청").
                ui.label(format!("\u{2192} {}", tr(lang, group_key)));
                ui.label(tr(lang, "control.hint"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "control.approve")).clicked() {
                        approve = true;
                    }
                    if ui.button(tr(lang, "control.deny")).clicked() {
                        deny = true;
                    }
                });
            });
        if approve {
            self.control_policy.approve(pane, group);
            self.notify = Some((
                format!("{} #{pane} {}", tr(lang, "control.title"), tr(lang, "control.approved")),
                std::time::Instant::now(),
            ));
            self.control_pending = None;
        } else if deny {
            self.control_pending = None; // 미승인 유지(다음 요청 시 다시 물음).
        }
    }
}

/// 설정 > 동작의 승인 현황 + 취소(revoke) UI(세션 한정 승인).
pub(crate) fn approvals_ui(
    ui: &mut egui::Ui,
    policy: &nabi_control::policy::ControlPolicy,
    lang: nabi_i18n::Lang,
) {
    let list = policy.approvals();
    ui.label(tr(lang, "control.approvals"));
    if list.is_empty() {
        ui.weak("\u{2014}");
        ui.end_row();
        return;
    }
    ui.vertical(|ui| {
        for (pane, g) in list {
            ui.horizontal(|ui| {
                let key = match g {
                    nabi_control::policy::Group::Inject => "control.group.inject",
                    _ => "control.group.act",
                };
                ui.label(format!("pane #{pane} \u{2014} {}", tr(lang, key)));
                if ui.small_button(tr(lang, "control.revoke")).clicked() {
                    policy.revoke(pane, g);
                }
            });
        }
    });
    ui.end_row();
}
