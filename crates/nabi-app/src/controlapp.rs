//! 제어 평면 앱 동작(AppCtl) 적용 — controlui.rs에서 분리(라인 한도).

use nabi_proto::AppCtl;

impl crate::app::NabiApp {
    /// 제어 평면 앱 동작(브라우저/SFTP 탭)을 매 프레임 적용.
    pub(crate) fn drain_control_app(&mut self) {
        while let Ok(act) = self.control_app_rx.try_recv() {
            match act {
                AppCtl::OpenBrowser { path } => {
                    let pane = self.open_browser_tab();
                    if let (Some(p), Some(b)) = (path, self.browser_tabs.get_mut(&pane)) {
                        b.path = std::path::PathBuf::from(p);
                    }
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
                        Ok(()) => self.notify = Some((format!("\u{23f0} 스케줄 등록: {label}"), std::time::Instant::now())),
                        Err(e) => self.notify = Some((format!("\u{2715} 스케줄 오류: {e}"), std::time::Instant::now())),
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
