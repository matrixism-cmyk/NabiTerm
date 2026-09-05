//! 하단 상태바: 활성 세션 제목·세션 수·그리드 크기(다국어).

use crate::app::NabiApp;
use crate::statusfmt::{human_duration, short_path};
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        if !self.config.appearance.show_statusbar {
            return;
        }
        // 재접속 중이면 그것부터 보인다 — 지금 무슨 일이 벌어지는지가 가장 급한 정보다.
        crate::reconnect::reconnect_bar(self, ui);
        let lang = self.lang;
        let focused = self.focused_pane();
        let title = if self.sftp_pane.is_some() && focused == self.sftp_pane {
            self.sftp.host.clone() // SFTP 탭 포커스 시 호스트 표시.
        } else {
            focused
                .and_then(|p| {
                    self.orch
                        .panes
                        .read()
                        .ok()
                        .and_then(|m| m.get(&p).map(|v| v.title.clone()))
                })
                .unwrap_or_else(|| tr(lang, "status.nosession").to_owned())
        };
        let cues_on = self.config.appearance.symbol_cues;
        let offline = self.config.terminal.offline_mode;
        let count = self.dock.iter_all_tabs().count(); let broadcast = self.broadcast; let tg_on = self.telegram.running(); let tg_err = self.telegram.has_error(); // 원격 제어 활성/오류(보안·피드백).
        let is_ssh = focused
            .and_then(|p| self.pane_origins.get(&p))
            .is_some_and(|k| matches!(k, nabi_session::SessionKind::Ssh { .. }));
        // 이 pane이 어느 표식의 세션인지 — 매 프레임 다시 본다(세션 표식을 고치면 바로 반영).
        let tag = focused.map(|p| self.pane_tag(p)).unwrap_or_default();
        // SSH 서버 통계(MobaXterm식) — 채워진 값이 있을 때만. 90% 초과면 빨강. 연결 지속시간 + OS/커널 툴팁.
        let stats = focused.and_then(|p| self.server_stats.get(&p)).filter(|s| !s.is_empty()).cloned();
        let conn = focused.and_then(|p| self.ssh_connect_time.get(&p))
            .map(|t| format!("\u{23f1}{}", nabi_proto::stats::human_uptime(t.elapsed().as_secs())));
        // 서버 통계가 없어도 **연결 유지 시간은 보여 준다** — 통계 폴링은 서버 쪽 사정으로
        // 자주 비고, 그때마다 "얼마나 붙어 있었나"까지 같이 사라지면 안 된다.
        let stats_txt = match (stats.as_ref(), conn.as_deref()) {
            (Some(s), Some(c)) => Some(format!("{} \u{00b7} {c}", s.summary())),
            (Some(s), None) => Some(s.summary()),
            (None, c) => c.map(str::to_string),
        };
        let stats_tip = stats.as_ref().map(|s| s.detail()).filter(|d| !d.is_empty());
        let stats_alert = stats.as_ref().is_some_and(|s| s.alert(self.config.terminal.ssh_stats_alert_pct as f32));
        // AI 도구 상태: 발행값(pane_status) 우선, 없으면 셸통합 run_cmd 자동 감지(🤖+경과).
        let ai = focused.and_then(|p| crate::aistatus::ai_display(
            self.pane_status.get(&p), self.run_cmd.get(&p).map(|s| s.as_str()),
            self.cmd_start.get(&p).map(|t| t.elapsed()),
            // 진행률도 이 줄에 함께 보여 준다 — 모르면 무작정 기다리게 된다(사용자 요청).
            self.progress.get(&p).copied(),
        ));
        let cwd = focused.and_then(|p| self.cwds.get(&p).cloned());
        let exit = focused.and_then(|p| self.last_exit.get(&p).copied());
        let dur = focused.and_then(|p| self.last_duration.get(&p).copied());
        let prog = focused.and_then(|p| self.progress.get(&p).copied());
        let dims = focused
            .and_then(|p| self.last_grid.get(&p).copied())
            .map(|g| format!("{}×{}", g.cols(), g.rows()));
        let encoding = self.config.terminal.encoding.clone();
        // P4: 보이는 화면의 치환문자(U+FFFD) 비율로 인코딩 오류 감지 → 전환 제안 칩.
        let enc_suggest = focused.and_then(|p| self.enc_suggestion(p, &encoding)); let clock = self.config.appearance.show_clock.then(|| chrono::Local::now().format("%H:%M").to_string());
        let group_n = self.broadcast_group.len();
        let watch = self.sync_watch_label(); // 원격 최신유지 칩(S6-54).
        // 실패 명령 AI 인계(사전 캡처 — 렌더 클로저 안에서 self 재차용 금지).
        let ai_pane = focused.and_then(|p| self.find_ai_pane(p));
        let mut do_handoff: Option<nabi_types::PaneId> = None;
        let mut do_copy_prompt = false;
        let mut stop_watch = false;
        let mut toggle_rec = false; // REC 배지를 눌렀는가 — 켜고 끄는 하나의 스위치다.
        let mut want_dims: Option<(u16, u16)> = None; // 크기 칩에서 고른 격자.
        let mut goto_tab: Option<nabi_types::PaneId> = None; // 탭 목록에서 고른 탭.
        let tab_list = self.all_tab_names(); // 가변 차용 전에 미리 모은다.
        let sel_info = self.selection.filter(|s| !s.is_empty()).map(|s| {
            let (sr, sc, er, ec) = s.span();
            if s.rect {
                format!("\u{2b1b} {}\u{00d7}{}", er - sr + 1, ec - sc + 1)
            } else {
                format!("\u{2702} {}", er - sr + 1)
            }
        });
        let zoom = focused
            .and_then(|p| self.pane_font.get(&p).copied())
            .filter(|z| (*z - self.font_size).abs() > 0.5);
        let fwd_n = self.forward.active.len();
        let (local_ip, public_ip) = (self.net_info.local.clone(), self.net_info.public());
        // 활성 전송(업/다운로드) 수 + 전체 진행률 — 활성 + 배경 원격 패널 합산.
        let (mut xfers, mut tb, mut ts) = (0usize, 0u64, 0u64);
        for t in self
            .sftp
            .transfers
            .iter()
            .chain(self.sftp_bg.values().flat_map(|p| p.transfers.iter()))
        {
            if !t.state.finished() {
                xfers += 1;
                if t.size > 0 {
                    tb += t.bytes;
                    ts += t.size;
                }
            }
        }
        let xpct = tb.saturating_mul(100).checked_div(ts).unwrap_or(0);
        let mut open_browser: Option<u8> = None; // cwd 열기: 0=탭,1=창,2=사이드바.
        let mut open_fwd = false;
        let mut set_enc: Option<String> = None;
        let mut focus_sftp = false;
        let mut jump_fail = false;
        // 실패한 명령 수 — 가변 차용 전에 미리 센다(칩은 아래에서 그린다).
        let failed = focused
            .and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()))
            .and_then(|v| v.model.lock().ok().map(|md| md.failed_count()))
            .unwrap_or(0);

        let sbar = egui::Frame::NONE
            .fill(crate::theme_ui::STATUS_FILL)
            .inner_margin(egui::Margin::symmetric(8, 2));
        egui::Panel::bottom("statusbar").frame(sbar).show(ui, |ui| {
            // 배경이 하드코딩 네이비라 텍스트도 밝게 강제(테마 무관 가독성).
            ui.visuals_mut().override_text_color = Some(crate::theme_ui::TEXT_BRIGHT);
            // 좁은 창에서 오른쪽 칩이 그냥 잘려 나가던 것을 단계로 접는다(statusfit).
            let fit = crate::statusfit::tier(ui.available_width());
            // 접힌 칩은 버리지 않고 여기 담아 ⋯ 안에서 보여 준다.
            let mut folded: Vec<(egui::Color32, String)> = Vec::new();
            ui.horizontal(|ui| {
                // 연결 종류별 색 점: SSH=시안, 로컬=녹색.
                let dc = if is_ssh { crate::theme_ui::SESS_SSH } else { crate::theme_ui::SESS_LOCAL };
                // 서버 이름이 길면 뒤쪽 칩(IP·시계)이 통째로 밀려 나갔다 — 이름은 줄이고
                // 전체는 마우스를 올리면 보여 준다(사용자 보고 2026-09-05).
                let shown = crate::statusfmt::elide(&title, 24);
                let r = ui.colored_label(dc, format!("\u{25cf} {shown}"));
                if shown != title {
                    r.on_hover_text(&title);
                }
                if is_ssh { crate::statuschips::ssh_badge(ui, lang, focused); }
                // 이 pane이 파일로 기록되는 중인지. 자동으로 켜질 수 있으므로 늘 보여 준다.
                let rec = focused.and_then(|p| self.session_logs.get(&p));
                if crate::statuschips::rec_badge(ui, lang, rec.is_some(), rec.is_some_and(|l| l.cast)) {
                    toggle_rec = true; // 켜져 있으면 멈추고, 꺼져 있으면 시작한다(아래에서).
                }
                if crate::statuschips::failed_badge(ui, lang, failed) {
                    jump_fail = true; // 누르면 실패한 자리로 간다(아래에서 처리).
                }
                // 표식(운영/스테이징/개발) — 색만이 아니라 글자로도 적는다. 지금 어디에
                // 명령을 치고 있는지가 상태바에서 늘 보여야 한다.
                if tag != nabi_session::SessionTag::None {
                    let (r, g, b) = tag.rgb();
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(r, g, b), tr(lang, tag.key()));
                }
                ui.separator();
                // 세션 수를 보여 주기만 하던 자리다. 탭이 많아 오른쪽 밖으로 나간
                // 탭은 굴려서 찾아야 했는데(사용자 보고 2026-09-05), 여기서 고르면
                // 폭과 상관없이 한 번에 간다.
                ui.menu_button(format!("{}: {count}", tr(lang, "status.sessions")), |ui| {
                    ui.set_min_width(220.0);
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (p, name) in &tab_list {
                            if ui.button(name).clicked() {
                                goto_tab = Some(*p);
                                ui.close();
                            }
                        }
                    });
                })
                .response
                .on_hover_text(tr(lang, "status.tablist"));
                if xfers > 0 {
                    ui.separator();
                    let pct = if ts > 0 { format!(" {xpct}%") } else { String::new() };
                    let lbl = egui::RichText::new(format!("\u{21c5} {xfers}{pct}"))
                        .color(crate::theme_ui::ACCENT);
                    if ui
                        .selectable_label(false, lbl)
                        .on_hover_text(tr(lang, "sftp.transfers"))
                        .clicked()
                    {
                        focus_sftp = true; // 클릭 시 SFTP 탭으로 점프.
                    }
                }
                if let Some(d) = &dims {
                    ui.separator();
                    // 크기를 보여 주기만 하던 자리다. 눌러서 바꿀 수 있게 했다
                    // (사용자 요청 2026-09-05). 격자를 직접 바꿔 봐야 다음 프레임에
                    // 덮이므로, 고른 크기에 맞게 **창을** 옮긴다(statusdims).
                    ui.menu_button(d, |ui| {
                        ui.set_min_width(120.0);
                        for (c, rr) in crate::statusdims::PRESETS {
                            if ui.button(format!("{c}\u{00d7}{rr}")).clicked() {
                                want_dims = Some((c, rr));
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr(lang, "status.dims.hint"));
                }
                if let Some(s) = &stats_txt {
                    ui.separator(); let c = if stats_alert { crate::theme_ui::ERR } else { crate::theme_ui::SESS_SSH };
                    // 색만으로는 경고가 전달되지 않는 사용자가 있다 — 켜면 기호가 붙는다.
                    let mk = crate::cues::cue(cues_on && stats_alert, crate::cues::WARN);
                    let r = ui.colored_label(c, format!("{mk}\u{1f5a5} {s}")); // OS·커널·접속자·RTT는 툴팁.
                    if let Some(t) = &stats_tip { r.on_hover_text(t); }
                }
                if let Some(a) = &ai {
                    ui.separator(); // AI 정보 강조(전용 색) + 사용률 발행 시 컨텍스트 게이지.
                    ui.colored_label(crate::theme_ui::ACCENT, &a.label).on_hover_text(&a.tip);
                    // 컨텍스트 사용률 게이지 — 다단계 색(정상 ACCENT / 80%↑ 앰버 / 95%↑ 빨강).
                    if let Some(g) = a.gauge {
                        let fill = match crate::aistatus::context_tier(g) { 2 => crate::theme_ui::ERR, 1 => crate::theme_ui::BROADCAST, _ => crate::theme_ui::ACCENT };
                        ui.add(egui::ProgressBar::new(g).desired_width(46.0).fill(fill).text(format!("{:.0}%", g * 100.0)));
                    }
                }
                ui.separator();
                // 인코딩 메뉴(에디터와 동일 목록 공유, SSOT) + 깨짐 감지 시 전환 제안 칩.
                if let Some(e) = self.encoding_controls(ui, &encoding, enc_suggest) { set_enc = Some(e); }
                if let Some(code) = exit {
                    ui.separator();
                    if code == 0 {
                        ui.colored_label(crate::theme_ui::OK, "\u{2713}");
                    } else {
                        // 실패 칩 = 메뉴: AI pane에 컨텍스트 인계 / 프롬프트 복사(2026 벤치마킹).
                        let chip = egui::RichText::new(format!("\u{2717} {code}")).color(crate::theme_ui::ERR);
                        ui.menu_button(chip, |ui| {
                            if let Some(ai) = ai_pane {
                                if ui.button(tr(lang, "handoff.ask")).clicked() { do_handoff = Some(ai); ui.close(); }
                            } else {
                                ui.weak(tr(lang, "handoff.noai"));
                            }
                            if ui.button(tr(lang, "handoff.copy")).clicked() { do_copy_prompt = true; ui.close(); }
                        });
                    }
                }
                if let Some(ms) = dur {
                    ui.separator();
                    ui.label(human_duration(ms));
                }
                if let Some(pct) = prog {
                    ui.separator();
                    ui.label(format!("\u{23f3} {pct}%"));
                }
                if let Some(wl) = &watch {
                    ui.separator();
                    if ui.selectable_label(false, wl).on_hover_text(tr(lang, "watch.stophint")).clicked() {
                        stop_watch = true;
                    }
                }
                if let Some(c) = &cwd {
                    ui.separator();
                    // 클릭하면 탭이나 새 창으로 연다(기본은 탭).
                    //
                    // 사이드바로 여는 항목은 뺐다. 사이드바는 더 이상 어디서도 쓰지 않는데
                    // 메뉴에만 남아 있어서, 눌러 보고 나서야 아무 데도 안 뜬다는 것을 알게 됐다.
                    ui.menu_button(short_path(c), |ui| {
                        for (key, m) in [("status.opentab", 0u8), ("status.openwin", 1)] {
                            if ui.button(tr(lang, key)).clicked() { open_browser = Some(m); ui.close(); }
                        }
                        ui.separator();
                        if ui.button(tr(lang, "status.copypath")).clicked() { ui.ctx().copy_text(crate::workspace::strip_uri_slash(c)); ui.close(); } // cwd 경로 복사.
                    })
                    .response
                    .on_hover_text(format!("{}\n{c}", tr(lang, "status.opencwd")));
                }
                if broadcast {
                    ui.separator();
                    let label = if group_n > 0 {
                        format!("📢 {} ({group_n})", tr(lang, "status.broadcast"))
                    } else {
                        format!("📢 {}", tr(lang, "status.broadcast"))
                    };
                    if fit.shows(crate::statusfit::Tier::Wide) {
                        ui.colored_label(crate::theme_ui::BROADCAST, label);
                    } else {
                        folded.push((crate::theme_ui::BROADCAST, label));
                    }
                }
                if tg_on {
                    ui.separator();
                    let (c, sfx) = match tg_err {
                        true => (crate::theme_ui::ERR, " (!)"),
                        false => (crate::theme_ui::BROADCAST, ""),
                    };
                    let hint = if tg_err { "tg.r.connerr" } else { "tg.grantall.hint" };
                    let label = format!("\u{2708} {}{sfx}", tr(lang, "settings.sec.telegram"));
                    if fit.shows(crate::statusfit::Tier::Wide) {
                        ui.colored_label(c, label).on_hover_text(tr(lang, hint));
                    } else {
                        folded.push((c, label));
                    }
                }
                let plain = crate::theme_ui::TEXT_BRIGHT;
                if let Some(info) = &sel_info {
                    if fit.shows(crate::statusfit::Tier::Wide) {
                        ui.separator();
                        ui.label(info);
                    } else {
                        folded.push((plain, info.clone()));
                    }
                }
                if let Some(z) = zoom {
                    let zl = format!("\u{1f50d} {}pt", z as i32);
                    if fit.shows(crate::statusfit::Tier::Wide) {
                        ui.separator();
                        ui.label(zl);
                    } else {
                        folded.push((plain, zl));
                    }
                }
                if fwd_n > 0 {
                    ui.separator();
                    if ui
                        .selectable_label(false, format!("\u{1f513} {fwd_n}"))
                        .on_hover_text(tr(lang, "fwd.title"))
                        .clicked()
                    {
                        open_fwd = true;
                    }
                }
                if fit.shows(crate::statusfit::Tier::Full) {
                    // 오프라인 모드는 **보여야** 한다 — 켠 줄 모르면 "왜 새 판이 안 뜨지"가 된다.
                    if offline {
                        ui.separator();
                        ui.weak(format!("\u{2708} {}", tr(lang, "status.offline")))
                            .on_hover_text(tr(lang, "settings.offlinehint"));
                    }
                    crate::netinfo::ip_status(ui, lang, &local_ip, &public_ip); // NIC/공인 IP.
                } else if !local_ip.is_empty() {
                    folded.push((plain, local_ip.clone()));
                }
                if let Some(c) = &clock {
                    let cl = format!("\u{1f550} {c}");
                    if fit.shows(crate::statusfit::Tier::Full) {
                        ui.separator();
                        ui.label(cl);
                    } else {
                        folded.push((plain, cl));
                    }
                }
                // 클립보드 기록(Win+V) — 맨 오른쪽에 늘 둔다. 접지 않는 까닭은,
                // 좁을수록 붙여넣을 것을 고르는 일이 더 잦기 때문이다.
                ui.separator();
                if ui
                    .add(egui::Label::new("\u{1f4cb}").sense(egui::Sense::click()))
                    .on_hover_text(tr(lang, "status.clipboard"))
                    .clicked()
                {
                    crate::statusclip::open_clipboard_history();
                }
                // 접힌 것을 버리지 않는다 — 한 번 눌러 볼 수 있어야 접은 것이다.
                if !folded.is_empty() {
                    ui.separator();
                    ui.menu_button("\u{22ef}", |ui| {
                        for (c, t) in &folded {
                            ui.colored_label(*c, t);
                        }
                    })
                    .response
                    .on_hover_text(tr(lang, "status.folded"));
                }
            });
        });
        if clock.is_some() { ctx.request_repaint_after(std::time::Duration::from_secs(1)); } // 시계 1Hz
        if open_fwd {
            self.open_forward();
        }
        if let (Some(mode), Some(c)) = (open_browser, cwd) {
            // OSC 7 cwd는 "/C:/.." 형태일 수 있어 앞 슬래시 제거(Windows 경로화).
            let path = std::path::PathBuf::from(crate::workspace::strip_uri_slash(&c));
            self.open_browser_path(path, mode);
        }
        if let Some(e) = set_enc {
            self.apply_encoding(e);
        }
        if let (Some(ai), Some(p)) = (do_handoff, focused) {
            self.handoff_failure_to_ai(p, ai);
        }
        if let (true, Some(p)) = (do_copy_prompt, focused) {
            if let Some(prompt) = self.failure_context(p) {
                ctx.copy_text(prompt);
            }
        }
        if stop_watch {
            self.sync_watch = None; // 상태바 칩 클릭 = 최신유지 중지.
        }
        // REC 배지는 **켜고 끄는 하나의 스위치**다(사용자 요청 2026-09-05).
        //
        // 끌 때는 손으로 켠 길과 같은 함수를 쓴다. 켤 때는 자동 자리에 바로 시작한다 —
        // 상태바의 작은 배지를 눌렀는데 파일 저장 창이 튀어나오면 그건 스위치가 아니다.
        // 기록은 pane 마다 따로 잡히므로(session_logs 는 PaneId 로 찾는다) 이 스위치도
        // 지금 보고 있는 pane 하나에만 걸린다.
        if toggle_rec {
            match focused.is_some_and(|p| self.session_logs.contains_key(&p)) {
                true => self.toggle_session_log(),
                false => self.start_rec_here(),
            }
        }
        if let Some(want) = want_dims {
            self.resize_window_for_grid(ctx, want);
        }
        if let Some(p) = goto_tab {
            self.focus_tab(p);
        }
        if jump_fail {
            self.jump_failed(true); // 칩을 누르면 다음 실패한 명령으로.
        }
        if focus_sftp {
            if let Some(p) = self.sftp_pane {
                if let Some(loc) = self.dock.find_tab(&p) {
                    let _ = self.dock.set_active_tab(loc);
                }
            }
        }
    }

    /// 선택한 인코딩을 설정에 저장하고 열린 모든 pane에 즉시 적용한다.
    fn apply_encoding(&mut self, label: String) {
        self.config.terminal.encoding = label.clone();
        self.save_config();
        let mut panes: Vec<_> = self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
        panes.extend(self.floating.iter().copied());
        for pane in panes {
            self.orch
                .send(nabi_proto::Command::SetEncoding { pane, label: label.clone() });
        }
    }
}
