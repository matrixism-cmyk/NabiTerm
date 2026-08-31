//! 제어평면 SFTP(S6-55) — `nabi cli sftp-list/get/put`를 현재 열린 SFTP 연결로 실행.
//!
//! 요청은 seq 상관으로 오고, 완료는 `Event::SftpCtlDone{seq,…}`로 EventHub에 회신한다
//! (dispatch가 그 이벤트를 기다린다 — LayoutExport와 같은 패턴). 전송은 일반 전송 큐를
//! 그대로 타므로 UI 큐·전송 히스토리에도 똑같이 보인다(별도 은닉 경로 없음).

use crate::app::NabiApp;
use nabi_proto::{Command, SftpCtlOp};
use std::collections::HashMap;

/// 진행 중인 제어평면 SFTP 요청의 상관 상태.
#[derive(Default)]
pub struct CtlSftp {
    /// 목록 대기: (원격 경로, 요청 seq) — 같은 경로 동시 요청도 각자 회신받게 Vec.
    pub list: Vec<(String, u64)>,
    /// 전송 대기: 큐 xfer id → 요청 seq. TransferDone(xfer)이 오면 회신.
    pub xfers: HashMap<u64, u64>,
}

impl NabiApp {
    /// AppCtl::SftpCtl 진입점 — 열린 연결이 없으면 즉시 실패 회신.
    pub(crate) fn on_sftp_ctl(&mut self, seq: u64, op: SftpCtlOp) {
        // 여는 것은 **연결이 있기 전에** 하는 일이라 아래 검사보다 앞에 온다.
        if let SftpCtlOp::Open { session } = op {
            match self.sessions.sessions.iter().find(|s| s.name == session).cloned() {
                Some(s) => {
                    let ftp = s.is_ftp;
                    self.open_sftp_saved(s, ftp);
                    // 탭이 열렸다는 뜻이지 붙었다는 뜻은 아니다 — 붙었는지는 `sftp-list` 가 안다.
                    self.sftp_ctl_reply(seq, true, format!("세션 '{session}' 탭을 열었습니다"));
                }
                None => {
                    let names: Vec<&str> =
                        self.sessions.sessions.iter().map(|s| s.name.as_str()).take(8).collect();
                    self.sftp_ctl_reply(
                        seq,
                        false,
                        format!("세션 '{session}' 없음 — 저장된 이름: {}", names.join(", ")),
                    );
                }
            }
            return;
        }
        // 목록 보기는 연결이 없어도 답할 수 있다(빈 배열) — 아래 고르기보다 앞에 온다.
        if let SftpCtlOp::Tabs = op {
            let items: Vec<String> = self
                .sftp_tabs()
                .iter()
                .map(|(p, s)| {
                    format!(
                        r#"{{"pane":{},"host":{},"path":{},"connected":{}}}"#,
                        p,
                        crate::webctl::json_str(&s.host),
                        crate::webctl::json_str(&s.path),
                        s.id.is_some()
                    )
                })
                .collect();
            self.sftp_ctl_reply(seq, true, format!("[{}]", items.join(",")));
            return;
        }
        // 어느 탭에 시킬지 먼저 고른다. 못 고르면 **왜 못 골랐는지** 말한다.
        let want = match &op {
            SftpCtlOp::List { pane, .. } | SftpCtlOp::Get { pane, .. } | SftpCtlOp::Put { pane, .. } => *pane,
            _ => None,
        };
        let id = match self.pick_sftp_id(want) {
            Ok(id) => id,
            Err(msg) => {
                self.sftp_ctl_reply(seq, false, msg);
                return;
            }
        };
        match op {
            // 위에서 처리하고 돌아갔다 — 여기 오지 않는다.
            SftpCtlOp::Open { .. } | SftpCtlOp::Tabs => {}
            SftpCtlOp::List { path, .. } => {
                // 같은 경로를 UI가 동시에 요청하는 드문 경우, 제어 회신을 우선한다.
                self.ctl_sftp.list.push((path.clone(), seq));
                self.orch.send(Command::SftpList { id, path });
            }
            SftpCtlOp::Get { remote, local, .. } => {
                let name = crate::sftppath::remote_basename(&remote).to_string();
                let x = self.push_xfer(name, false, 0, |xfer| Command::SftpDownload {
                    id, xfer, remote, local, resume: 0,
                });
                self.ctl_sftp.xfers.insert(x, seq);
            }
            SftpCtlOp::Put { local, remote, .. } => {
                let meta = std::fs::metadata(&local);
                let Ok(meta) = meta else {
                    self.sftp_ctl_reply(seq, false, format!("로컬 파일 없음: {local}"));
                    return;
                };
                let name = crate::sftppath::remote_basename(&remote).to_string();
                let x = self.push_xfer(name, true, meta.len(), |xfer| Command::SftpUpload { id, xfer, local, remote });
                self.ctl_sftp.xfers.insert(x, seq);
            }
        }
    }

    /// SftpListing 이벤트가 제어 요청의 회신인가 — 맞으면 JSON으로 회신하고 true(UI 갱신 생략).
    pub(crate) fn sftp_ctl_take_listing(&mut self, path: &str, entries: &[nabi_proto::SftpEntry]) -> bool {
        let (hit, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ctl_sftp.list).into_iter().partition(|(p, _)| p == path);
        self.ctl_sftp.list = rest;
        if hit.is_empty() {
            return false;
        }
        let data = serde_json::to_string(entries).unwrap_or_else(|_| "[]".into());
        for (_, seq) in hit {
            self.sftp_ctl_reply(seq, true, data.clone());
        }
        true
    }

    /// TransferDone이 제어 요청의 전송인가 — 맞으면 결과를 회신한다(히스토리 기록과 병행).
    pub(crate) fn sftp_ctl_take_xfer(&mut self, xfer: u64, ok: bool, message: &str, name: &str) {
        if let Some(seq) = self.ctl_sftp.xfers.remove(&xfer) {
            let data = if ok { format!("완료: {name}") } else { message.to_string() };
            self.sftp_ctl_reply(seq, ok, data);
        }
    }

    /// SFTP 오류 발생 시: 대기 중인 제어 목록 요청을 실패 회신하고, 동기화 스캔 대기도 푼다.
    /// (오류 이벤트에는 경로가 없어 개별 상관이 불가 — 짧은 수명의 대기라 일괄 해제가 안전.)
    pub(crate) fn sftp_ctl_fail_pending(&mut self, message: &str) {
        for (_, seq) in std::mem::take(&mut self.ctl_sftp.list) {
            self.sftp_ctl_reply(seq, false, message.to_string());
        }
        // (list는 Vec — take로 비웠으니 신규 요청은 이후 자유롭게 쌓인다.)
        if let Some(dlg) = &mut self.sync_dlg {
            dlg.pending = None;
        }
    }

    /// 열려 있는 SFTP 탭들 — `(도킹 pane 번호, 패널)`.
    ///
    /// 활성 패널은 `self.sftp` 에, 나머지는 `self.sftp_bg` 에 있다(포커스가 옮겨 갈 때 서로
    /// 자리를 바꾼다). 두 곳을 다 봐야 열린 것을 다 센다.
    fn sftp_tabs(&self) -> Vec<(u64, &crate::sftppanel::SftpPanel)> {
        let mut v: Vec<(u64, &crate::sftppanel::SftpPanel)> = Vec::new();
        if let Some(p) = self.sftp_pane.filter(|_| self.sftp.open) {
            v.push((p.get(), &self.sftp));
        }
        v.extend(self.sftp_bg.iter().filter(|(_, s)| s.open).map(|(p, s)| (p.get(), s)));
        v.sort_by_key(|(p, _)| *p);
        v
    }

    /// 어느 SFTP 탭에 시킬 것인가.
    ///
    /// **번호를 안 주면 열린 것이 하나일 때만** 그것을 쓴다. 웹 탭(`web-eval`)이 이미 쓰는
    /// 규칙과 같게 맞췄다 — 규칙이 갈라지면 부르는 쪽이 둘을 따로 외워야 한다.
    ///
    /// 예전에는 번호를 받지 않고 **포커스가 있는 패널**에 시켰다. 사람에게는 자연스럽지만
    /// 부르는 쪽이 에이전트면 포커스가 어디인지 알 수 없다 — 서버 두 곳에 붙어 있으면
    /// 엉뚱한 쪽에 파일을 올리게 된다.
    fn pick_sftp_id(&self, pane: Option<u64>) -> Result<nabi_proto::SftpId, String> {
        let tabs = self.sftp_tabs();
        let panel = match pane {
            Some(want) => tabs
                .iter()
                .find(|(p, _)| *p == want)
                .map(|(_, s)| *s)
                .ok_or_else(|| format!("pane {want} 은 SFTP 탭이 아닙니다 — `sftp-tabs` 로 번호를 확인하세요"))?,
            None if tabs.len() == 1 => tabs[0].1,
            None if tabs.is_empty() => {
                return Err("열린 SFTP 연결이 없습니다 — 먼저 SFTP 탭을 연결하세요".into())
            }
            None => {
                let ns: Vec<String> = tabs.iter().map(|(p, _)| p.to_string()).collect();
                return Err(format!(
                    "SFTP 탭이 {}개 열려 있습니다 — `--pane` 으로 지목하세요(번호: {})",
                    tabs.len(),
                    ns.join(", ")
                ));
            }
        };
        panel.id.ok_or_else(|| "그 SFTP 탭은 아직 연결되지 않았습니다".into())
    }

    /// 앱이 한 일의 결과를 부른 쪽에 돌려준다(`seq` 가 없으면 아무도 안 기다리므로 넘어간다).
    ///
    /// 토스트는 사람이 보는 것이고 이것은 부른 쪽이 보는 것이다. 둘은 대신할 수 없다 —
    /// 사람만 보는 실패는 에이전트에게 성공과 구별되지 않는다.
    pub(crate) fn ctl_reply(&self, seq: Option<u64>, ok: bool, data: String) {
        if let Some(seq) = seq {
            self.control_events.publish(&nabi_proto::Event::CtlResult { seq, ok, data });
        }
    }

    fn sftp_ctl_reply(&self, seq: u64, ok: bool, data: String) {
        self.control_events.publish(&nabi_proto::Event::SftpCtlDone { seq, ok, data });
    }
}
