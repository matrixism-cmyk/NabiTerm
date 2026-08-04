//! 하단 상태바: 활성 세션 제목·세션 수·그리드 크기(다국어).

use crate::app::NabiApp;
use crate::statusfmt::{human_duration, short_path};
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        if !self.config.appearance.show_statusbar {
            return;
        }
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
        let count = self.dock.iter_all_tabs().count(); let broadcast = self.broadcast; let tg_on = self.telegram.running(); let tg_err = self.telegram.has_error(); // 원격 제어 활성/오류(보안·피드백).
        let is_ssh = focused
            .and_then(|p| self.pane_origins.get(&p))
            .is_some_and(|k| matches!(k, nabi_session::SessionKind::Ssh { .. }));
        // SSH 서버 통계(MobaXterm식) — 채워진 값이 있을 때만. 90% 초과면 빨강. 연결 지속시간 + OS/커널 툴팁.
        let stats = focused.and_then(|p| self.server_stats.get(&p)).filter(|s| !s.is_empty()).cloned();
        let conn = focused.and_then(|p| self.ssh_connect_time.get(&p))
            .map(|t| format!(" \u{00b7} \u{23f1}{}", nabi_proto::stats::human_uptime(t.elapsed().as_secs())));
        let stats_txt = stats.as_ref().map(|s| s.summary() + conn.as_deref().unwrap_or(""));
        let stats_tip = stats.as_ref().map(|s| s.detail()).filter(|d| !d.is_empty());
        let stats_alert = stats.as_ref().is_some_and(|s| s.alert(self.config.terminal.ssh_stats_alert_pct as f32));
        // AI 도구 상태: 발행값(pane_status) 우선, 없으면 셸통합 run_cmd 자동 감지(🤖+경과).
        let ai = focused.and_then(|p| crate::aistatus::ai_display(
            self.pane_status.get(&p), self.run_cmd.get(&p).map(|s| s.as_str()),
            self.cmd_start.get(&p).map(|t| t.elapsed()),
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
            if !t.done {
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

        let sbar = egui::Frame::none()
            .fill(crate::theme_ui::STATUS_FILL)
            .inner_margin(egui::Margin::symmetric(8.0, 2.0));
        egui::TopBottomPanel::bottom("statusbar").frame(sbar).show(ctx, |ui| {
            // 배경이 하드코딩 네이비라 텍스트도 밝게 강제(테마 무관 가독성).
            ui.visuals_mut().override_text_color = Some(crate::theme_ui::TEXT_BRIGHT);
            ui.horizontal(|ui| {
                // 연결 종류별 색 점: SSH=시안, 로컬=녹색.
                let dc = if is_ssh { crate::theme_ui::SESS_SSH } else { crate::theme_ui::SESS_LOCAL };
                ui.colored_label(dc, format!("\u{25cf} {title}"));
                if is_ssh {
                    ui.colored_label(crate::theme_ui::ACCENT, "SSH");
                }
                ui.separator();
                ui.label(format!("{}: {count}", tr(lang, "status.sessions")));
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
                if let Some(d) = &dims { ui.separator(); ui.label(d); }
                if let Some(s) = &stats_txt {
                    ui.separator(); let c = if stats_alert { crate::theme_ui::ERR } else { crate::theme_ui::SESS_SSH };
                    let r = ui.colored_label(c, format!("\u{1f5a5} {s}")); // OS·커널·접속자·RTT는 툴팁.
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
                        ui.colored_label(crate::theme_ui::ERR, format!("\u{2717} {code}"));
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
                if let Some(c) = &cwd {
                    ui.separator();
                    // 클릭 시 탭/창/사이드바 중 선택해서 열기(기본 탭).
                    ui.menu_button(short_path(c), |ui| {
                        for (key, m) in [("status.opentab", 0u8), ("status.openwin", 1), ("status.openside", 2)] {
                            if ui.button(tr(lang, key)).clicked() { open_browser = Some(m); ui.close_menu(); }
                        }
                        ui.separator();
                        if ui.button(tr(lang, "status.copypath")).clicked() { ui.ctx().copy_text(crate::workspace::strip_uri_slash(c)); ui.close_menu(); } // cwd 경로 복사.
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
                    ui.colored_label(crate::theme_ui::BROADCAST, label);
                }
                if tg_on {
                    ui.separator();
                    let (c, sfx) = match tg_err {
                        true => (crate::theme_ui::ERR, " (!)"),
                        false => (crate::theme_ui::BROADCAST, ""),
                    };
                    let hint = if tg_err { "tg.r.connerr" } else { "tg.grantall.hint" };
                    let label = format!("\u{2708} {}{sfx}", tr(lang, "settings.sec.telegram"));
                    ui.colored_label(c, label).on_hover_text(tr(lang, hint));
                }
                if let Some(info) = &sel_info {
                    ui.separator();
                    ui.label(info);
                }
                if let Some(z) = zoom {
                    ui.separator();
                    ui.label(format!("\u{1f50d} {}pt", z as i32));
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
                crate::netinfo::ip_status(ui, lang, &local_ip, &public_ip); // NIC/공인 IP.
                if let Some(c) = &clock { ui.separator(); ui.label(format!("\u{1f550} {c}")); }
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
        if focus_sftp {
            if let Some(p) = self.sftp_pane {
                if let Some(loc) = self.dock.find_tab(&p) {
                    self.dock.set_active_tab(loc);
                }
            }
        }
    }

    /// 선택한 인코딩을 설정에 저장하고 열린 모든 pane에 즉시 적용한다.
    fn apply_encoding(&mut self, label: String) {
        self.config.terminal.encoding = label.clone();
        let _ = nabi_config::save(&self.config_path, &self.config);
        let mut panes: Vec<_> = self.dock.iter_all_tabs().map(|(_, p)| *p).collect();
        panes.extend(self.floating.iter().copied());
        for pane in panes {
            self.orch
                .send(nabi_proto::Command::SetEncoding { pane, label: label.clone() });
        }
    }
}
