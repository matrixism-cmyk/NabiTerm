//! pane 하나의 trzsz 상태 — 트리거를 잡고, 사용자에게 묻고, 전송을 끝까지 돌린다.
//!
//! 자리는 라우터의 **VT 주입 직전**이다. 전송 중에는 바이트를 전부 삼켜 화면에 프로토콜이
//! 새지 않게 하고, 회신은 이미 있는 `transport.write` 길로 나간다. 그래서 SSH든 로컬 PTY든
//! 시리얼이든 전송 수단을 가리지 않는다.

use crate::trzszfile::{DiskSource, DiskStorage};
use crossbeam_channel::Sender;
use nabi_proto::{Event, XferDecision, XferMode, XferProgress};
use nabi_trzsz::{Mode, Plan, Session, Step, Trigger, TriggerScanner, UploadItem};
use nabi_types::PaneId;
use std::path::{Path, PathBuf};

/// 한 번의 전송에서 받을 수 있는 파일 수 상한(원격이 끝없이 보내는 것을 막는다).
const MAX_FILES: usize = 2000;

/// pane별 상태. 아무 일도 없을 때는 스캐너 하나뿐이라 비용이 거의 없다.
#[derive(Default)]
pub struct PaneTrzsz {
    scanner: TriggerScanner,
    /// 사용자에게 물어보고 답을 기다리는 중인 트리거.
    asking: Option<Trigger>,
    /// 대답을 기다리는 동안 원격이 보낸 바이트(대답 뒤에 세션에 먹인다).
    parked: Vec<u8>,
    session: Option<Session>,
}

/// 라우터가 한 청크를 처리한 결과.
pub struct Routed {
    /// VT 모델에 넣을 바이트.
    pub display: Vec<u8>,
    /// 원격으로 보낼 바이트.
    pub reply: Vec<u8>,
}

impl PaneTrzsz {
    /// 출력 청크를 거른다. 전송 중이면 화면에는 아무것도 가지 않는다.
    pub fn filter(&mut self, pane: PaneId, chunk: &[u8], tx: &Sender<Event>) -> Routed {
        let scanned = self.scanner.feed(chunk);
        let mut out = Routed { display: scanned.display, reply: Vec::new() };
        if let Some(t) = scanned.trigger {
            // 모드 F는 "원격이 올릴 로컬 파일을 지정"한다 — 물어보지도 않고 거절한다.
            if t.mode == Mode::UploadSpecified {
                self.refuse(pane, &t, tx, &mut out);
                return out;
            }
            let _ = tx.send(Event::TrzszAsk { pane, mode: to_proto(t.mode) });
            self.asking = Some(t);
        }
        if !scanned.rest.is_empty() {
            self.consume(pane, &scanned.rest, tx, &mut out);
        }
        out
    }

    /// 사용자의 결정이 왔다.
    pub fn decide(&mut self, d: &XferDecision, tx: &Sender<Event>) -> Vec<u8> {
        let Some(t) = self.asking.take() else { return Vec::new() };
        let plan = if d.accept { build_plan(&t, d) } else { Ok(Plan::Reject("declined".into())) };
        let plan = plan.unwrap_or_else(|why| {
            let _ = tx.send(done_event(d.pane, false, why, Vec::new()));
            Plan::Reject("declined".into())
        });
        let (session, steps) = Session::new(&t, plan);
        self.session = Some(session);
        let mut out = Routed { display: Vec::new(), reply: Vec::new() };
        self.apply(d.pane, steps, tx, &mut out);
        // 기다리는 동안 쌓인 바이트를 이제 먹인다.
        let parked = std::mem::take(&mut self.parked);
        if !parked.is_empty() {
            self.consume(d.pane, &parked, tx, &mut out);
        }
        out.reply
    }

    /// 사용자가 취소했다.
    pub fn cancel(&mut self, pane: PaneId, tx: &Sender<Event>) -> Vec<u8> {
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        let steps = s.cancel();
        let mut out = Routed { display: Vec::new(), reply: Vec::new() };
        self.apply(pane, steps, tx, &mut out);
        out.reply
    }

    /// 지금 전송 중인가(UI가 오버레이를 그릴지 정한다).
    pub fn busy(&self) -> bool {
        self.session.is_some() || self.asking.is_some()
    }

    fn refuse(&mut self, pane: PaneId, t: &Trigger, tx: &Sender<Event>, out: &mut Routed) {
        let (session, steps) = Session::new(t, Plan::Reject("blocked".into()));
        self.session = Some(session);
        self.apply(pane, steps, tx, out);
        let why = "remote asked to upload files it chose — blocked".to_owned();
        let _ = tx.send(done_event(pane, false, why, Vec::new()));
    }

    fn consume(&mut self, pane: PaneId, bytes: &[u8], tx: &Sender<Event>, out: &mut Routed) {
        if self.session.is_none() {
            // 아직 사용자가 답하지 않았다 — 흘려버리면 프로토콜이 깨진다.
            self.parked.extend_from_slice(bytes);
            return;
        }
        let steps = self.session.as_mut().map(|s| s.on_bytes(bytes)).unwrap_or_default();
        self.apply(pane, steps, tx, out);
    }

    fn apply(&mut self, pane: PaneId, steps: Vec<Step>, tx: &Sender<Event>, out: &mut Routed) {
        for s in steps {
            match s {
                Step::Write(b) => out.reply.extend_from_slice(&b),
                Step::Progress(p) => {
                    let progress = XferProgress {
                        index: p.index,
                        count: p.count,
                        name: p.name,
                        done: p.done,
                        total: p.total,
                    };
                    let _ = tx.send(Event::TrzszProgress { pane, progress });
                }
                Step::Done { summary, names } => {
                    let _ = tx.send(done_event(pane, true, summary, names));
                }
                Step::Failed(why) => {
                    let _ = tx.send(done_event(pane, false, why, Vec::new()));
                }
            }
        }
        if self.session.as_ref().is_some_and(Session::is_ended) {
            self.session = None;
            self.parked.clear();
            self.scanner.resume();
        }
    }
}

fn done_event(pane: PaneId, ok: bool, message: String, names: Vec<String>) -> Event {
    Event::TrzszDone { pane, ok, message, names }
}

fn to_proto(m: Mode) -> XferMode {
    match m {
        Mode::Download => XferMode::Download,
        Mode::Upload => XferMode::Upload,
        Mode::UploadDir => XferMode::UploadDir,
        Mode::UploadSpecified => XferMode::UploadSpecified,
    }
}

/// 사용자의 결정을 실제 계획으로 바꾼다(파일을 여기서 연다).
fn build_plan(t: &Trigger, d: &XferDecision) -> Result<Plan, String> {
    if t.mode.is_upload() {
        let mut items = Vec::new();
        for p in &d.upload {
            items.push(open_item(p)?);
        }
        if items.is_empty() {
            return Err("no files chosen to upload".into());
        }
        return Ok(Plan::Upload(items));
    }
    let dir: PathBuf = d.save_dir.clone().ok_or("no save folder chosen")?;
    Ok(Plan::Download(Box::new(DiskStorage::new(dir, MAX_FILES))))
}

fn open_item(path: &Path) -> Result<UploadItem, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("bad path: {}", path.display()))?;
    let (size, source) = DiskSource::open(path)?;
    Ok(UploadItem { name, size, source: Box::new(source) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chan() -> (Sender<Event>, crossbeam_channel::Receiver<Event>) {
        crossbeam_channel::unbounded()
    }

    #[test]
    fn ordinary_output_passes_through_untouched() {
        let (tx, rx) = chan();
        let mut p = PaneTrzsz::default();
        let r = p.filter(PaneId(1), b"$ ls\r\nfile\r\n", &tx);
        assert_eq!(r.display, b"$ ls\r\nfile\r\n");
        assert!(r.reply.is_empty());
        assert!(rx.try_recv().is_err());
        assert!(!p.busy());
    }

    #[test]
    fn a_trigger_asks_the_user_and_hides_the_magic() {
        let (tx, rx) = chan();
        let mut p = PaneTrzsz::default();
        let r = p.filter(PaneId(1), b"ok\n::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n", &tx);
        assert_eq!(r.display, b"ok\n");
        assert!(matches!(rx.try_recv(), Ok(Event::TrzszAsk { mode: XferMode::Download, .. })));
        assert!(p.busy(), "대답을 기다리는 동안도 전송 중이다");
    }

    /// 원격이 "네 로컬 파일 이걸 올려라"라고 하는 모드는 물어보지도 않는다.
    #[test]
    fn the_remote_chosen_upload_mode_is_blocked_outright() {
        let (tx, rx) = chan();
        let mut p = PaneTrzsz::default();
        let r = p.filter(PaneId(1), b"::TRZSZ:TRANSFER:F:1.1.8:1755780000000\n", &tx);
        assert!(!r.reply.is_empty(), "원격에 거절을 알려야 한다");
        let evs: Vec<_> = rx.try_iter().collect();
        assert!(
            evs.iter().any(|e| matches!(e, Event::TrzszDone { ok: false, .. })),
            "차단 사실을 사용자에게 알린다: {evs:?}"
        );
        assert!(!evs.iter().any(|e| matches!(e, Event::TrzszAsk { .. })), "묻지 않는다");
    }

    #[test]
    fn bytes_that_arrive_before_the_answer_are_kept() {
        let (tx, _rx) = chan();
        let mut p = PaneTrzsz::default();
        p.filter(PaneId(1), b"::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n", &tx);
        let r = p.filter(PaneId(1), b"#CFG:eJx\n", &tx);
        assert!(r.display.is_empty(), "전송 중에는 화면에 새지 않는다");
        assert!(!p.parked.is_empty(), "대답 전에 온 프레임을 버리면 프로토콜이 깨진다");
    }

    #[test]
    fn declining_ends_the_session_and_output_resumes() {
        let (tx, _rx) = chan();
        let mut p = PaneTrzsz::default();
        p.filter(PaneId(1), b"::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n", &tx);
        let reply = p.decide(&XferDecision::reject(PaneId(1)), &tx);
        assert!(!reply.is_empty(), "거절도 원격에 알려야 한다");
        assert!(!p.busy());
        assert_eq!(p.filter(PaneId(1), b"$ ", &tx).display, b"$ ", "화면 출력이 돌아온다");
    }

    #[test]
    fn accepting_a_download_without_a_folder_fails_cleanly() {
        let (tx, rx) = chan();
        let mut p = PaneTrzsz::default();
        p.filter(PaneId(1), b"::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n", &tx);
        p.decide(&XferDecision { pane: PaneId(1), accept: true, save_dir: None, upload: Vec::new() }, &tx);
        let evs: Vec<_> = rx.try_iter().collect();
        assert!(evs.iter().any(|e| matches!(e, Event::TrzszDone { ok: false, .. })), "{evs:?}");
        assert!(!p.busy(), "실패해도 pane이 전송 상태로 남으면 안 된다");
    }
}
