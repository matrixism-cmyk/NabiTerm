//! 탭 동작(저장 세션 연결 · 복제 · 다른 탭 닫기). 메뉴·팔레트·워크스페이스 공용.

use crate::app::NabiApp;
use nabi_session::{SavedSession, SessionKind};

impl NabiApp {
    /// 저장된 SSH 세션 정보로 Quick Connect 폼을 채우고 연다(비밀번호는 입력받는다).
    pub(crate) fn prefill_ssh(&mut self, host: String, port: u16, user: String) {
        self.quick_connect.host = host; self.quick_connect.port = port.to_string();
        self.quick_connect.user = user;
        self.quick_connect.password.clear();
        self.quick_connect.open = true;
    }

    /// 스니펫 치환용 pane 컨텍스트({{host}}/{{user}}/{{port}}/{{cwd}}). SSH는 출처에서, cwd는 OSC 7.
    pub(crate) fn snippet_vars(&self, pane: nabi_types::PaneId) -> Vec<(&'static str, String)> {
        let cwd = self.cwds.get(&pane).map(|c| crate::workspace::strip_uri_slash(c)).unwrap_or_default();
        let (host, user, port) = match self.pane_origins.get(&pane) {
            Some(SessionKind::Ssh { host, user, port, .. }) => (host.clone(), user.clone(), port.to_string()),
            _ => (String::new(), String::new(), String::new()),
        };
        let sel = self.selection_text().unwrap_or_default(); // {{selection}} = 현재 선택 텍스트.
        vec![("host", host), ("user", user), ("port", port), ("cwd", cwd), ("selection", sel)]
    }

    /// 포커스된 탭을 별도 OS 창으로 분리한다(메뉴·팔레트 공용). 브라우저 탭도 분리 가능(닫으면 재도킹).
    pub(crate) fn tear_off_focused(&mut self) {
        if let Some(p) = self.focused_pane() {
            if let Some(idx) = self.dock.find_tab(&p) {
                self.dock.remove_tab(idx);
            }
            self.floating.push(p);
        }
    }

    /// 포커스된 탭을 "창 안에 띄우기"(메인 창 안 오버레이)로 옮긴다(메뉴·팔레트 공용, P3/P9). 닫으면 재도킹.
    /// 오버레이는 터미널만 렌더하므로 터미널/SSH pane만 대상(그 외는 무시).
    pub(crate) fn dock_float_focused(&mut self) {
        if let Some(p) = self.focused_pane() {
            let is_term = self.orch.panes.read().ok().is_some_and(|m| m.contains_key(&p))
                && !self.browser_tabs.contains_key(&p)
                && !self.editors.contains_key(&p)
                && Some(p) != self.sftp_pane
                && !self.sftp_bg.contains_key(&p);
            if !is_term {
                return;
            }
            if let Some(idx) = self.dock.find_tab(&p) {
                self.dock.remove_tab(idx);
            }
            self.docked_float.push(p);
        }
    }

    /// 포커스된 pane의 터미널을 리셋한다(RIS — 손상된 화면 복구).
    pub(crate) fn reset_focused(&mut self) {
        if let Some(p) = self.focused_pane() {
            if let Some(view) = self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()) {
                if let Ok(mut model) = view.model.lock() {
                    model.reset();
                }
            }
        }
    }

    /// 현재 선택 영역의 텍스트(없으면 None).
    pub(crate) fn selection_text(&self) -> Option<String> {
        let sel = self.selection?;
        let (sr, sc, er, ec) = sel.span();
        let view = self.orch.panes.read().ok().and_then(|m| m.get(&sel.pane).cloned())?;
        let model = view.model.lock().ok()?;
        let rows = model.render_rows(&self.theme);
        let wrapped: Vec<bool> = (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
        let text = crate::selection::extract_selection(&rows, sr, sc, er, ec, &wrapped, sel.rect);
        (!text.is_empty()).then_some(text)
    }

    /// 현재 선택 영역을 클립보드에 복사한다(Ctrl+Shift+C).
    pub(crate) fn copy_selection(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.selection_text() {
            self.record_clip(&text); ctx.copy_text(text); // F1 히스토리 기록 후 복사.
        }
    }

    /// 포커스된 pane의 화면 전체를 선택하고 텍스트를 돌려준다(Select All).
    pub(crate) fn select_all_focused(&mut self) -> Option<String> {
        let p = self.focused_pane()?;
        let view = self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned())?;
        let model = view.model.lock().ok()?;
        let rows = model.render_rows(&self.theme);
        if rows.is_empty() {
            return None;
        }
        let last_r = rows.len() - 1;
        let last_c = rows[last_r].len().saturating_sub(1);
        let wrapped: Vec<bool> = (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
        self.selection = Some(crate::selection::Sel {
            pane: p,
            ar: 0,
            ac: 0,
            hr: last_r,
            hc: last_c,
            rect: false,
        });
        Some(crate::selection::extract_selection(
            &rows, 0, 0, last_r, last_c, &wrapped, false,
        ))
    }

    /// 포커스된 터미널의 출력(히스토리+화면)을 파일로 저장한다(세션 로그).
    pub(crate) fn save_focused_output(&mut self) {
        let p = match self.focused_pane() {
            Some(p) => p,
            None => return,
        };
        let text = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).cloned())
            .and_then(|v| v.model.lock().ok().map(|md| md.dump_text(1_000_000)));
        let Some(text) = text else { return };
        if let Some(path) = rfd::FileDialog::new().set_file_name("terminal-output.txt").save_file() {
            let msg = match std::fs::write(&path, text) {
                Ok(()) => format!("\u{2713} {}", path.display()),
                Err(e) => format!("\u{2715} {e}"),
            };
            self.notify = Some((msg, std::time::Instant::now()));
        }
    }

    /// 포커스된 탭만 남기고 나머지를 닫는다.
    pub(crate) fn close_other_tabs(&mut self) {
        if let Some(keep) = self.focused_pane() {
            let others: Vec<nabi_types::PaneId> = self
                .dock
                .iter_all_tabs()
                .map(|(_, p)| *p)
                .filter(|p| *p != keep)
                .collect();
            for p in others {
                self.orch.send(nabi_proto::Command::ClosePane { pane: p });
            }
        }
    }

    /// 포커스된 탭을 동일한 출처(로컬 셸/SSH)로 복제한다.
    pub(crate) fn duplicate_focused(&mut self) {
        if let Some(p) = self.focused_pane() {
            self.duplicate_pane(p);
        }
    }

    /// 지정 pane과 같은 출처(셸 종류 또는 SSH 세션)로 탭을 하나 더 연다.
    /// 팔레트(DuplicateTab)와 탭 우클릭 메뉴가 이 한 곳을 공유한다(SSOT).
    pub(crate) fn duplicate_pane(&mut self, p: nabi_types::PaneId) {
        if let Some(kind) = self.pane_origins.get(&p).cloned() {
            self.connect_saved(SavedSession {
                name: String::new(),
                folder: None,
                kind,
                on_connect: None,
                cwd: None,
                is_ftp: false,
                open_sftp: false,
            });
        }
    }

    /// 가장 최근에 닫힌 탭을 같은 출처(로컬 셸/SSH)로 다시 연다(실수로 닫음 복구).
    pub(crate) fn reopen_closed(&mut self) {
        if let Some(kind) = self.closed_sessions.pop() {
            self.connect_saved(SavedSession {
                name: String::new(),
                folder: None,
                kind,
                on_connect: None,
                cwd: None,
                is_ftp: false,
                open_sftp: false,
            });
        }
    }

    /// 저장 세션을 연다(로컬은 즉시, SSH는 볼트로 자동연결 또는 Quick Connect 프리필).
    pub(crate) fn connect_saved(&mut self, s: SavedSession) {
        // D4: 명명된 세션의 마지막 접속 시각 기록(목록에 상대시간 표시).
        if !s.name.is_empty() { self.config.terminal.last_connected.insert(s.name.clone(), chrono::Local::now().timestamp()); let _ = nabi_config::save(&self.config_path, &self.config); }
        if s.is_ftp {
            self.open_sftp_saved(s, true); // FTP 세션은 FTP 브라우저로.
            return;
        }
        // "SFTP도 함께 열기" 세션이면 미리 복제(아래에서 kind가 move됨) — SSH 연결 성공 후 SFTP도 연다.
        let also_sftp = s.open_sftp.then(|| s.clone());
        let oncmd = s.on_connect;
        // 저장된 cwd가 아직 실재 디렉터리일 때만 사용(그 사이 삭제됐으면 기본).
        let saved_cwd = s.cwd.filter(|d| std::path::Path::new(d).is_dir());
        match s.kind {
            SessionKind::Local { shell } => {
                self.spawn_local_cwd(crate::workspace::shell_from_str(&shell), oncmd, saved_cwd)
            }
            SessionKind::Ssh {
                host,
                port,
                user,
                credential_ref,
                key_path,
                jump,
            } => {
                let sb = self.config.terminal.scrollback;
                let enc = self.config.terminal.encoding.clone();
                let (params, cred, kp) = if let Some(pw) =
                    credential_ref.as_ref().and_then(|k| self.vault_get(k))
                {
                    let p = nabi_proto::SshParams::password(host.clone(), port, user.clone(), pw);
                    (p, credential_ref.clone(), key_path.clone())
                } else if let Some(k) = key_path.clone() {
                    let p =
                        nabi_proto::SshParams::key_file(host.clone(), port, user.clone(), k.clone(), None);
                    (p, None, Some(k))
                } else {
                    self.prefill_ssh(host, port, user);
                    return;
                };
                // ProxyJump(B4): 저장된 점프 호스트가 있으면 같은 인증으로 경유.
                let params = match jump.as_deref().and_then(crate::qcparse::parse_connect) {
                    Some(jp) => { let mut j = nabi_proto::SshParams::password(jp.host, jp.port.unwrap_or(22), jp.user.unwrap_or_else(|| user.clone()), String::new()); j.auth = params.auth.clone(); params.with_jump(j) }
                    None => params,
                };
                let origin = SessionKind::Ssh { host, port, user, credential_ref: cred, key_path: kp, jump };
                let seq = self.register_spawn(origin, oncmd);
                self.orch.send(nabi_proto::Command::ConnectSsh {
                    params,
                    size: nabi_types::GridSize::default(),
                    scrollback: sb,
                    encoding: enc,
                    stats_secs: self.config.terminal.ssh_stats_secs,
                    reply_seq: Some(seq),
                });
                // 터미널 연결과 동시에 SFTP 브라우저도 연다(저장 세션 설정).
                if let Some(ss) = also_sftp {
                    self.open_sftp_saved(ss, false);
                }
            }
        }
    }
}
