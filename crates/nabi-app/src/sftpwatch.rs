//! 원격 최신유지(S6-54) — 로컬 폴더를 폴링 감시해 변경/추가 파일을 자동 업로드한다.
//!
//! WinSCP "Keep remote directory up to date" 상당. 파일 감시 의존성 없이 5초 폴링
//! (스냅샷 diff는 syncplan::plan 재사용 — 새 파일=Copy, 변경=Update). 삭제는 전파하지
//! 않는다(v1 안전 — 지우는 일은 사람이 명시적으로).

use crate::app::NabiApp;
use crate::syncplan::{plan, to_map, walk_local, SyncAction, SyncBy};
use nabi_proto::Command;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// 감시 대상 파일 수 상한 — 폴링 스캔이 프레임을 잡아먹지 않게.
const WATCH_CAP: usize = 20_000;
const INTERVAL: Duration = Duration::from_secs(5);

pub struct SyncWatch {
    pub local: String,
    pub remote: String,
    /// 마지막 스냅샷(상대경로 → 크기·mtime).
    last: BTreeMap<String, (u64, u64)>,
    next_scan: Instant,
    /// 이 감시가 올린 누적 파일 수(상태 표시).
    pub pushed: usize,
}

impl SyncWatch {
    pub fn new(local: String, remote: String) -> Self {
        // 시작 스냅샷 = 현재 상태(이후 "변화"만 올린다 — 시작 시 전체 업로드가 필요하면
        // 동기화 실행을 먼저 쓰라는 설계).
        let last = to_map(&walk_local(std::path::Path::new(&local)));
        SyncWatch { local, remote, last, next_scan: Instant::now() + INTERVAL, pushed: 0 }
    }
}

impl NabiApp {
    /// 매 프레임: 감시가 켜져 있으면 주기 스캔 → 변경분 업로드 큐잉.
    pub(crate) fn sync_watch_tick(&mut self, ctx: &egui::Context) {
        let Some(w) = &mut self.sync_watch else { return };
        // 연결이 끊겼으면 감시도 멈춘다(무의미한 큐잉 방지).
        let Some(id) = self.sftp.id else {
            self.sync_watch = None;
            return;
        };
        if Instant::now() < w.next_scan {
            ctx.request_repaint_after(INTERVAL); // 유휴 시에도 주기 도래 시 깨어나게.
            return;
        }
        w.next_scan = Instant::now() + INTERVAL;
        let cur_list = walk_local(std::path::Path::new(&w.local));
        if cur_list.len() > WATCH_CAP {
            self.notify = Some((nabi_i18n::tr(self.lang, "watch.toobig").to_string(), Instant::now()));
            self.sync_watch = None;
            return;
        }
        let cur = to_map(&cur_list);
        // 새 파일=Copy, 변경=Update(±2초 관용) — 동기화 계획 로직 그대로(SSOT).
        let acts = plan(&cur, &w.last, SyncBy::SizeAndTime, false);
        w.last = cur;
        let (lroot, rroot) = (w.local.clone(), w.remote.trim_end_matches('/').to_string());
        let rels: Vec<String> = acts
            .iter()
            .filter(|a| matches!(a, SyncAction::Copy(_) | SyncAction::Update(_)))
            .map(|a| a.path().to_string())
            .collect();
        if rels.is_empty() {
            return;
        }
        if let Some(w) = &mut self.sync_watch {
            w.pushed += rels.len();
        }
        for rel in rels {
            let lpath = std::path::Path::new(&lroot).join(rel.replace('/', "\\"));
            let size = std::fs::metadata(&lpath).map(|m| m.len()).unwrap_or(0);
            let (l, rpath) = (lpath.to_string_lossy().into_owned(), format!("{rroot}/{rel}"));
            self.push_xfer(rel, true, size, |xfer| Command::SftpUpload { id, xfer, local: l, remote: rpath });
        }
        ctx.request_repaint();
    }

    /// 상태바 표시용 요약: "감시 중 · 올린 N".
    pub(crate) fn sync_watch_label(&self) -> Option<String> {
        let w = self.sync_watch.as_ref()?;
        Some(format!("\u{1f441} {} \u{00b7} {}", nabi_i18n::tr(self.lang, "watch.on"), w.pushed))
    }
}
