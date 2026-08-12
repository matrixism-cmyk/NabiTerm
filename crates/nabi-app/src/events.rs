//! 오케스트레이터 이벤트 처리(pane 생성/종료/출력/클립보드/cwd).

use crate::app::NabiApp;
use nabi_proto::Event;

impl NabiApp {
    pub(crate) fn handle_events(&mut self, ctx: &egui::Context) {
        let focused = self.focused_pane();
        for ev in self.orch.drain_events() {
            self.control_events.publish(&ev); // 제어 평면 Wait/Tail 구독자에 fan-out.
            // SFTP 이벤트는 eventsftp.rs가 처리하고, 아니면 이벤트를 그대로 돌려준다.
            let Some(ev) = self.handle_sftp_event(ev, ctx) else { continue };
            match ev {
                Event::PaneSpawned { pane, seq } => {
                    if self.control_float {
                        self.control_float = false; // 제어 dock=new-window → 별도 OS 창.
                        self.floating.push(pane);
                    } else if let Some(right) = self.pending_split.take() {
                        self.split_pane(pane, right);
                    } else {
                        self.add_pane(pane);
                    }
                    // seq로 미완료 스폰을 조회 — 도착 순서와 무관하게 정확한 출처/명령/레이아웃을 적용.
                    match seq.and_then(|s| self.pending_spawns.remove(&s)) {
                        Some(ps) => {
                            if let Some(b) = &ps.backlog {
                                self.inject_restore_backlog(pane, b); // 로컬 복원 스크롤백(표시 전용).
                            }
                            self.pane_origins.insert(pane, ps.origin);
                            if let Some(cmd) = ps.oncmd.filter(|c| !c.is_empty()) {
                                let mut data = cmd.into_bytes();
                                data.push(b'\r');
                                self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
                            }
                            if let Some(ord) = ps.ordinal {
                                self.layout_arrive(ord, pane); // 분할 레이아웃에 ordinal로 합류.
                            }
                            if let Some(g) = ps.float_geom {
                                // 분리 OS 창으로 복원: 도크에서 빼 floating으로 + 위치·크기 적용(P10).
                                if let Some(idx) = self.dock.find_tab(&pane) {
                                    self.dock.remove_tab(idx);
                                }
                                self.floating.push(pane);
                                self.floating_geom.insert(pane, g);
                            }
                        }
                        // 제어 평면(nabi cli spawn) 등 앱 pending이 아닌 pane → 기본 셸 출처(복원용).
                        None => {
                            let shell = self.config.terminal.default_shell.clone();
                            self.pane_origins.insert(pane, nabi_session::SessionKind::Local { shell });
                        }
                    }
                }
                Event::PaneOutput { pane } => {
                    if focused != Some(pane) {
                        self.activity.insert(pane);
                    }
                    ctx.request_repaint();
                }
                Event::PaneExited { pane, .. } => {
                    if let Some(idx) = self.dock.find_tab(&pane) {
                        self.dock.remove_tab(idx);
                    }
                    self.last_grid.remove(&pane);
                    self.tab_names.remove(&pane);
                    self.tab_colors.remove(&pane);
                    self.last_bell.remove(&pane);
                    self.broadcast_group.remove(&pane);
                    self.wheel_keys.remove(&pane);
                    // 출처를 "닫힌 세션" 스택에 적재(실수로 닫은 탭 재열기용, 최근 16개).
                    if let Some(kind) = self.pane_origins.remove(&pane) {
                        self.closed_sessions.push(kind);
                        let n = self.closed_sessions.len();
                        if n > 16 {
                            self.closed_sessions.drain(0..n - 16);
                        }
                    }
                    self.cwds.remove(&pane);
                    self.activity.remove(&pane);
                    self.last_exit.remove(&pane);
                    self.cmd_start.remove(&pane);
                    self.last_duration.remove(&pane);
                    self.progress.remove(&pane);
                    self.pane_font.remove(&pane);
                    self.server_stats.remove(&pane);
                    self.pane_status.remove(&pane);
                    self.ssh_connect_time.remove(&pane);
                    self.ssh_alert_on.remove(&pane);
                    self.ctx_alert_on.remove(&pane);
                    self.blocked_alert.remove(&pane);
                    if self.selection.is_some_and(|s| s.pane == pane) {
                        self.selection = None;
                    }
                }
                Event::SshDisconnected { pane, message } => {
                    self.server_stats.remove(&pane);
                    // P7: 자동 재접속은 "안정적으로 연결됐다 끊긴 경우(≥20s)"만 — 즉시 실패(인증/서버
                    // 오류)는 모달로 띄워 무한 재시도 루프를 방지한다. ssh_connect_time 재사용.
                    let stable = self.ssh_connect_time.get(&pane).is_some_and(|t| t.elapsed().as_secs() >= 20);
                    self.ssh_connect_time.remove(&pane);
                    if self.config.terminal.auto_reconnect && stable {
                        self.notify = Some((format!("\u{21bb} {message}"), std::time::Instant::now()));
                        self.do_reconnect(pane);
                    } else {
                        self.reconnect_ask = Some((pane, message)); // pane 유지(마지막 화면 보존).
                    }
                    ctx.request_repaint();
                }
                Event::ServerStats { pane, stats } => {
                    // 리소스 임계(90%) 진입 시 1회 토스트(idle→alert 전이만, 중복 억제).
                    let now_alert = stats.alert(self.config.terminal.ssh_stats_alert_pct as f32);
                    let summary = stats.summary();
                    if now_alert && !self.ssh_alert_on.insert(pane, now_alert).unwrap_or(false) {
                        self.notify = Some((format!("\u{26a0} 서버 리소스 임계: {summary}"), std::time::Instant::now()));
                    } else if !now_alert {
                        self.ssh_alert_on.insert(pane, false);
                    }
                    self.server_stats.insert(pane, *stats);
                    self.ssh_connect_time.entry(pane).or_insert_with(std::time::Instant::now);
                }
                Event::HostKeyPrompt { id, host, port, algorithm, fingerprint } => {
                    self.hostkey_prompt = Some((id, host, port, algorithm, fingerprint));
                    ctx.request_repaint();
                }
                // 원격(OSC 52)이 로컬 클립보드에 쓰려 한다 — 설정대로 처리한다.
                // 예전에는 제약도 알림도 없이 그대로 덮어썼다.
                Event::ClipboardCopy { text } => {
                    let mode =
                        crate::osc52policy::Osc52Mode::from_u8(self.config.terminal.osc52_mode);
                    let (apply, tell) = crate::osc52policy::decide(mode);
                    if apply {
                        self.record_clip(&text);
                        if tell {
                            let p = crate::osc52policy::preview(&text, 40);
                            let head = nabi_i18n::tr(self.lang, "osc52.wrote");
                            self.notify = Some((
                                format!("\u{1f4cb} {head} \u{2014} {p}"),
                                std::time::Instant::now(),
                            ));
                        }
                        ctx.copy_text(text);
                    }
                }
                Event::CwdChanged { pane, path } => {
                    let dir = crate::workspace::strip_uri_slash(&path); // E1 zoxide 디렉터리 기록(20s 디바운스 저장)
                    crate::dirjump::record(&mut self.config.terminal.dir_visits, &dir, chrono::Local::now().timestamp(), 500);
                    if self.dir_save_at.elapsed().as_secs() >= 20 { let _ = nabi_config::save(&self.config_path, &self.config); self.dir_save_at = std::time::Instant::now(); }
                    self.cwds.insert(pane, path);
                }
                Event::CommandLine { pane, cmd } => {
                    self.run_cmd.insert(pane, cmd); // 실행 시작 — 종료(133;D) 시 제거.
                }
                Event::CommandStarted { pane } => {
                    self.cmd_start.insert(pane, std::time::Instant::now());
                }
                Event::CommandBlock { pane, block } => {
                    let dur = self.cmd_start.remove(&pane).map(|s| s.elapsed());
                    if let Some(d) = dur { self.last_duration.insert(pane, d.as_millis()); }
                    if let Some(code) = block.exit_code {
                        self.last_exit.insert(pane, code);
                        // 비포커스 pane: 실패 또는 오래 걸린(≥10s) 명령 완료를 토스트+작업표시줄 attention으로 알림(E3).
                        if focused != Some(pane) && (code != 0 || dur.is_some_and(|d| d.as_secs() >= 10)) {
                            let title = self.orch.panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.title.clone())).unwrap_or_default();
                            let msg = if code == 0 { format!("\u{2713} {title}") } else { format!("{title} {} (exit {code})", nabi_i18n::tr(self.lang, "notify.bgfail")) };
                            self.notify = Some((msg, std::time::Instant::now()));
                            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
                        }
                    }
                    self.record_cmd_history(pane, block.exit_code.unwrap_or(0)); // E5 히스토리.
                    self.fulfill_telegram(pane); // 텔레그램 보류 요청에 명령 출력 회신.
                    // 명령이 끝나면 진행률 표시도 정리(미해제 시퀀스 대비).
                    self.progress.remove(&pane);
                    self.run_cmd.remove(&pane); // 명령 종료 — 실행 중 명령에서 제거.
                }
                Event::Notify { pane, text } => {
                    // pane 제목 귀속(어느 탭/세션의 알림인지) — 에이전트 완료/입력대기 식별.
                    let title = self.orch.panes.read().ok()
                        .and_then(|m| m.get(&pane).map(|v| v.title.clone())).unwrap_or_default();
                    let body = if title.is_empty() { text } else { format!("[{title}] {text}") };
                    self.notify = Some((body, std::time::Instant::now()));
                    // 비포커스(다른 탭/창) pane의 알림은 작업표시줄 attention으로 승격.
                    if focused != Some(pane) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                            egui::UserAttentionType::Informational,
                        ));
                    }
                    ctx.request_repaint();
                }
                Event::Progress { pane, percent } => match percent {
                    Some(p) => {
                        self.progress.insert(pane, p);
                    }
                    None => {
                        self.progress.remove(&pane);
                    }
                },
                Event::ControlOsc { pane, verb, json } => {
                    self.handle_control_osc(pane, &verb, &json);
                }
                Event::ForwardStarted { id, message } => {
                    self.notify =
                        Some((format!("\u{1f513} {message}"), std::time::Instant::now()));
                    self.forward.active.push((id, message));
                    ctx.request_repaint();
                }
                Event::Error { message } | Event::SpawnFailed { message, .. } => {
                    self.notify = Some((message, std::time::Instant::now()));
                    ctx.request_repaint();
                }
                _ => {}
            }
        }
    }
}
