//! 텔레그램 브리지 — 백그라운드 폴링(수신)·전송(회신) 스레드 + 앱 라우터.
//! 토큰·화이트리스트는 start 시 전달(토큰은 keyring에서). 보안: 화이트리스트 chat만 처리(무인).

use nabi_i18n::tr;
use crate::app::NabiApp;
use crossbeam_channel::{Receiver, Sender};
use nabi_telegram::TgMessage;
use nabi_types::PaneId;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// 명령 전송 후 출력 회신을 기다리는 보류 요청.
struct TgPending {
    chat: i64,
    pane: PaneId,
    deadline: Instant,
}

/// 폴링·전송 워커 핸들. running()이면 동작 중.
#[derive(Default)]
pub(crate) struct TelegramBridge {
    incoming: Option<Receiver<TgMessage>>,
    outgoing: Option<Sender<(i64, String)>>,
    stop: Option<Arc<AtomicBool>>,
    err: Option<Arc<AtomicBool>>, // 폴링 실패(토큰 오류·네트워크) 표시 — 상태바 피드백.
    pending: Vec<TgPending>,
    last_sync: Option<Instant>, // 시작 시도(keyring 조회) 레이트 리밋 기준.
}

impl TelegramBridge {
    pub(crate) fn running(&self) -> bool {
        self.stop.is_some()
    }

    /// 폴링이 실패 중인가(토큰 오류·네트워크 — 상태바 경고용).
    pub(crate) fn has_error(&self) -> bool {
        self.err.as_ref().is_some_and(|e| e.load(Ordering::Relaxed))
    }

    /// 시작 시도(keyring 조회) 시점인가 — 2초마다만 true(매 프레임 keyring 접근 방지).
    fn start_due(&mut self, now: Instant) -> bool {
        if self.last_sync.is_none_or(|t| now.duration_since(t) >= std::time::Duration::from_secs(2)) {
            self.last_sync = Some(now);
            true
        } else {
            false
        }
    }

    /// 폴링·전송 스레드를 시작한다(이미 실행 중이면 먼저 중지). allowed가 비면 호출측에서 막아야 함.
    pub(crate) fn start(&mut self, token: String, poll_secs: u64, allowed: HashSet<i64>, pairing: bool) {
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        let (in_tx, in_rx) = crossbeam_channel::unbounded();
        let (out_tx, out_rx) = crossbeam_channel::unbounded::<(i64, String)>();
        let err = Arc::new(AtomicBool::new(false));
        let (ptoken, pstop, perr) = (token.clone(), stop.clone(), err.clone());
        std::thread::spawn(move || {
            let mut offset = 0i64;
            while !pstop.load(Ordering::Relaxed) {
                match nabi_telegram::get_updates(&ptoken, offset, poll_secs) {
                    Ok((msgs, next)) => {
                        perr.store(false, Ordering::Relaxed); // 정상 → 오류 해제.
                        offset = next;
                        for m in msgs {
                            // 화이트리스트 통과 + (pairing 모드면) 미지 chat도 앱으로 —
                            // 앱이 페어링 코드 발급/무시를 판단한다(C1). 기본은 종전대로 차단.
                            if allowed.contains(&m.chat_id) || pairing {
                                let _ = in_tx.send(m);
                            }
                        }
                    }
                    Err(_) => {
                        perr.store(true, Ordering::Relaxed); // 토큰 오류·네트워크 → 상태바 경고.
                        std::thread::sleep(std::time::Duration::from_secs(3)); // backoff.
                    }
                }
            }
        });
        std::thread::spawn(move || {
            while let Ok((chat, text)) = out_rx.recv() {
                let _ = nabi_telegram::send_message(&token, chat, &text);
            }
        });
        self.incoming = Some(in_rx);
        self.outgoing = Some(out_tx);
        self.stop = Some(stop);
        self.err = Some(err);
    }

    /// 스레드를 중지한다(폴링은 다음 주기에, 전송은 채널 드롭 시 종료).
    pub(crate) fn stop(&mut self) {
        if let Some(s) = self.stop.take() {
            s.store(true, Ordering::Relaxed);
        }
        self.incoming = None;
        self.outgoing = None;
        self.err = None;
    }

    fn try_recv(&self) -> Option<TgMessage> {
        self.incoming.as_ref().and_then(|r| r.try_recv().ok())
    }

    pub(crate) fn reply(&self, chat_id: i64, text: String) {
        if let Some(tx) = &self.outgoing {
            let _ = tx.send((chat_id, text));
        }
    }

    pub(crate) fn push_pending(&mut self, chat: i64, pane: PaneId, deadline: Instant) {
        self.pending.push(TgPending { chat, pane, deadline });
    }

    /// 해당 pane을 기다리던 보류들을 꺼낸다(명령 완료 시 — 정확 경로).
    pub(crate) fn take_for_pane(&mut self, pane: PaneId) -> Vec<i64> {
        let mut out = Vec::new();
        self.pending.retain(|p| if p.pane == pane { out.push(p.chat); false } else { true });
        out
    }

    /// 기한이 지난 보류들을 꺼낸다(OSC133 없는 셸용 폴백).
    fn take_due(&mut self, now: Instant) -> Vec<(i64, PaneId)> {
        let mut out = Vec::new();
        self.pending.retain(|p| if now >= p.deadline { out.push((p.chat, p.pane)); false } else { true });
        out
    }
}

impl NabiApp {
    /// 설정(enabled)과 워커 상태를 매 프레임 동기화하고, 수신 메시지를 처리한다.
    pub(crate) fn tick_telegram(&mut self) {
        let want = self.config.telegram.enabled && !self.config.telegram.allowed_chats.is_empty();
        if want && !self.telegram.running() && self.telegram.start_due(Instant::now()) {
            // 활성화 전환 — 토큰은 keyring에서만(평문 금지). 토큰 없으면 시작 안 함(2초마다만 조회).
            if let Some(token) = nabi_secret::keyringstore::load_telegram_token() {
                let allowed: HashSet<i64> = self.config.telegram.allowed_chats.iter().copied().collect();
                let pairing = self.config.telegram.dm_policy == "pairing";
                self.telegram.start(token, self.config.telegram.poll_secs.max(1), allowed, pairing);
            }
        } else if !want && self.telegram.running() {
            self.telegram.stop();
        }
        while let Some(m) = self.telegram.try_recv() {
            self.handle_telegram_msg(m);
        }
        // 기한 지난 보류(OSC133 없는 셸) → 현재까지의 출력으로 회신.
        for (chat, pane) in self.telegram.take_due(Instant::now()) {
            self.telegram_send_output(chat, pane);
        }
        self.telegram_heartbeat_tick();
    }

    /// 미지 chat의 페어링 요청 처리(C1 — OpenClaw DM pairing 벤치마킹).
    ///
    /// 만료 코드를 만들어 한 번만 회신하고, 승인 대기 목록에 올린다. 스팸 방어:
    /// 이미 대기 중이면 무응답, 대기 상한 3건(ClawJacked 교훈 — 무한 승인 요청 금지).
    pub(crate) fn telegram_pair_request(&mut self, chat: i64) {
        let now = Instant::now();
        self.telegram_pending.retain(|(_, _, exp)| *exp > now); // 만료 청소.
        if self.telegram_pending.iter().any(|(c, _, _)| *c == chat) {
            return; // 이미 대기 — 재응답 없음(응답 스팸 방지).
        }
        if self.telegram_pending.len() >= 3 {
            return; // 상한 — 오퍼레이터가 비우기 전까지 무시.
        }
        // 코드는 비밀이 아니라 대조용 표식(앱 화면과 상대 메시지의 일치 확인). 6자리 hex.
        let code = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (chat, std::time::SystemTime::now()).hash(&mut h);
            format!("{:06X}", h.finish() & 0xFF_FFFF)
        };
        let exp = now + std::time::Duration::from_secs(3600);
        self.telegram.reply(chat, format!("페어링 코드: {code} — nabiTerm 설정 \u{25b8} 텔레그램에서 이 코드를 확인하고 승인하면 사용할 수 있습니다(1시간 유효)."));
        self.telegram_pending.push((chat, code.clone(), exp));
        self.notify = Some((format!("\u{2708} {} chat {chat} \u{b7} {} {code}", tr(self.lang, "tg.pair.request"), tr(self.lang, "tg.pair.code")), now));
    }

    /// 대상 pane의 출력을 캡처해 텔레그램으로 회신한다(OSC133이면 마지막 명령 출력, 아니면 화면 끝 N줄).
    pub(crate) fn telegram_send_output(&mut self, chat: i64, pane: PaneId) {
        let lines = self.config.telegram.reply_lines.max(1);
        let raw = self.orch.panes.read().ok().and_then(|m| m.get(&pane).cloned())
            .and_then(|v| v.model.lock().ok().map(|md| md.last_command_output().unwrap_or_else(|| md.visible_text(lines))))
            .unwrap_or_default();
        let t = raw.trim_end();
        let n = t.chars().count();
        let text = if n == 0 {
            "(출력 없음)".to_string()
        } else if n > 3500 {
            format!("\u{2026}{}", t.chars().skip(n - 3500).collect::<String>()) // 텔레그램 4096자 제한 — 최근 출력 우선.
        } else {
            t.to_string()
        };
        // 명령 실패(종료코드≠0, OSC133로 알 때)면 표시 — 원격에서 성공/실패 즉시 인지.
        let exit = self.last_exit.get(&pane).filter(|&&e| e != 0).map(|e| format!("\n[exit {e}]")).unwrap_or_default();
        self.telegram.reply(chat, format!("{text}{exit}"));
    }
}
