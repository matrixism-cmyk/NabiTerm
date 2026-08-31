//! 제어 평면 앱 동작(AppCtl) 적용 — controlui.rs에서 분리(라인 한도).

use nabi_i18n::tr;
use nabi_proto::AppCtl;

impl crate::app::NabiApp {
    /// 제어 평면 앱 동작(브라우저/SFTP 탭)을 매 프레임 적용.
    ///
    /// `ctx` 는 화면 캡처가 배율을 알아야 해서 받는다(배치 AN).
    pub(crate) fn drain_control_app(&mut self, ctx: &egui::Context) {
        while let Ok(act) = self.control_app_rx.try_recv() {
            match act {
                AppCtl::Screenshot { pane, out } => {
                    let msg = self.shoot(ctx, pane, out, None);
                    self.notify = Some((msg, std::time::Instant::now()));
                }
                AppCtl::ShotSeq { seq, pane, out } => {
                    // 결과를 **부른 쪽에도** 돌려준다. 화면 토스트만으로는 AI 가 알 수 없다.
                    let msg = self.shoot(ctx, pane, out, Some(seq));
                    self.notify = Some((msg, std::time::Instant::now()));
                }
                AppCtl::OpenBrowser { path } => {
                    let pane = self.open_browser_tab();
                    if let (Some(p), Some(b)) = (path, self.browser_tabs.get_mut(&pane)) {
                        b.path = std::path::PathBuf::from(p);
                    }
                }
                AppCtl::Progress { pane, percent } => {
                    let id = nabi_types::PaneId::new(pane);
                    match percent {
                        Some(p) => {
                            // 말한 쪽이 권위다. 이제 이 pane 은 화면을 읽지 않는다.
                            self.progress_osc.insert(id);
                            self.progress.insert(id, p.min(100));
                        }
                        None => self.forget_progress(id),
                    }
                }
                AppCtl::WebList { seq } => self.control_web_list(seq),
                AppCtl::WebEval { seq, pane, js } => self.control_web_eval(seq, pane, js),
                AppCtl::ShowHistory { pane } => {
                    // 번호를 안 주면 지금 보고 있는 pane 이다 — 사람에게 보여 주는 것이니
                    // 눈앞의 것이 기본이어야 한다.
                    if let Some(p) = pane.map(nabi_types::PaneId::new).or_else(|| self.focused_pane()) {
                        self.open_history_view(p);
                    }
                }
                AppCtl::WebAct { seq, pane, act, arg } => self.control_web_act(seq, pane, act, arg),
                // 묻지 않고 끝낸다. `quit` 이 작업 공간을 저장하고 프로세스를 닫는다.
                // 우리가 사라진 뒤에 우리를 다시 띄우는 도우미를 먼저 걸고 나간다.
                AppCtl::Restart => {
                    crate::relaunch::arm();
                    self.quit();
                }
                AppCtl::Quit => self.quit(),
                AppCtl::SelfUpdate { check } => self.control_self_update(check),
                AppCtl::OpenWeb { url, window } => {
                    // 기본은 탭이다. 메뉴와 같은 곳에 연다 — 부르는 길에 따라 다르게
                    // 열리면 헷갈린다. `--window` 를 준 때만 별도 창으로 띄운다.
                    let u = url.unwrap_or_else(|| crate::webopen::HOME.to_string());
                    match window {
                        true => {
                            if let Some(msg) = crate::webopen::open(self.lang, Some(&u)) {
                                self.notify = Some((msg, std::time::Instant::now()));
                            }
                        }
                        false => {
                            self.open_web_tab(&u);
                        }
                    }
                }
                AppCtl::OpenHere { path } => {
                    self.spawn_local_at(path);
                    self.raise_window = true; // 탐색기에서 부른 것이니 창이 앞으로 와야 한다.
                }
                AppCtl::OpenEditor { path } => {
                    self.open_editor_local(std::path::PathBuf::from(path));
                }
                AppCtl::OpenSftp { session } => {
                    // 저장 세션 이름으로 SFTP 열기(자격증명은 볼트/connect_saved 경유).
                    if let Some(s) =
                        self.sessions.sessions.iter().find(|s| s.name == session).cloned()
                    {
                        let ftp = s.is_ftp;
                        self.open_sftp_saved(s, ftp);
                    } else {
                        self.notify = Some((
                            format!("세션 '{session}' 없음"),
                            std::time::Instant::now(),
                        ));
                    }
                }
                // 다음 PaneSpawned의 도킹 위치(CP-7): 분할은 기존 pending_split 재사용.
                AppCtl::DockNext { dock } => match dock.as_str() {
                    "split-right" => self.pending_split = Some(true),
                    "split-down" => self.pending_split = Some(false),
                    "new-window" => self.control_float = true,
                    _ => {}
                },
                AppCtl::ConnectSession { session } => {
                    if let Some(s) =
                        self.sessions.sessions.iter().find(|s| s.name == session).cloned()
                    {
                        self.connect_saved(s); // 자격증명은 볼트 경유(평문 금지).
                    } else {
                        self.notify =
                            Some((format!("세션 '{session}' 없음"), std::time::Instant::now()));
                    }
                }
                AppCtl::Focus { pane } => {
                    let p = nabi_types::PaneId::new(pane);
                    if let Some(loc) = self.dock.find_tab(&p) {
                        let _ = self.dock.set_active_tab(loc);
                    }
                }
                AppCtl::SetTitle { pane, title } => {
                    let p = nabi_types::PaneId::new(pane);
                    self.tab_names.insert(p, title.clone());
                    // 제어 평면 list/--match에서도 보이도록 레지스트리에 반영.
                    if let Ok(map) = self.orch.panes.read() {
                        if let Some(v) = map.get(&p) {
                            if let Ok(mut u) = v.user_title.lock() {
                                *u = Some(title);
                            }
                        }
                    }
                }
                AppCtl::Notify { from, title, body } => {
                    let who = from.map(|p| format!("pane #{p}: ")).unwrap_or_default();
                    let text = if body.is_empty() {
                        format!("{who}{title}")
                    } else {
                        format!("{who}{title} \u{2014} {body}")
                    };
                    self.notify = Some((text, std::time::Instant::now()));
                }
                AppCtl::SftpCtl { seq, op } => self.on_sftp_ctl(seq, op),
                AppCtl::LayoutExport { seq } => {
                    let json = self.layout_export_json();
                    self.control_events.publish(&nabi_proto::Event::LayoutJson { seq, json });
                }
                AppCtl::ScheduleCreate { name, spec, kind, payload, pane_title } => {
                    let label = if name.is_empty() { spec.clone() } else { name.clone() };
                    match self.add_schedule(name, spec, kind, payload, pane_title) {
                        Ok(()) => self.notify = Some((format!("\u{23f0} {} {label}", tr(self.lang, "sched.registered")), std::time::Instant::now())),
                        Err(e) => self.notify = Some((format!("\u{2715} {} {e}", tr(self.lang, "sched.error")), std::time::Instant::now())),
                    }
                }
                AppCtl::PaneStatus { pane, key, value, ttl_ms } => {
                    let pid = nabi_types::PaneId::new(pane);
                    // TTL(B7): 만료 시각을 따로 적어 두고 tick에서 걷어낸다.
                    match ttl_ms.filter(|_| value.is_some()) {
                        Some(ms) => {
                            self.pane_status_ttl.insert((pid, key.clone()),
                                std::time::Instant::now() + std::time::Duration::from_millis(ms));
                        }
                        None => { self.pane_status_ttl.remove(&(pid, key.clone())); }
                    }
                    self.set_pane_status(pid, key, value);
                }
            }
        }
    }

}

impl crate::app::NabiApp {
    /// 화면을 찍고, 알림에 쓸 한 줄을 돌려준다.
    ///
    /// `seq` 가 있으면 **부른 쪽에도** 결과를 이벤트로 보낸다. 화면 토스트만 내면 사람은
    /// 보지만 AI 에이전트는 못 본다 — 어디에 남았는지도, 왜 실패했는지도 모른 채
    /// "됐다"는 답만 받는다. 실제로 그 상태였고, 제어 동사 전수 스모크가 잡았다.
    fn shoot(
        &mut self,
        ctx: &egui::Context,
        pane: Option<u64>,
        out: Option<String>,
        seq: Option<u64>,
    ) -> String {
        let (msg, path, error) = match self.take_screenshot(ctx, pane, out) {
            Ok(p) => (format!("\u{1f4f7} {}", p.display()), p.display().to_string(), String::new()),
            Err(e) => (format!("\u{1f4f7} {e}"), String::new(), e.to_string()),
        };
        if let Some(seq) = seq {
            self.control_events.publish(&nabi_proto::Event::ShotDone { seq, path, error });
        }
        msg
    }
}
