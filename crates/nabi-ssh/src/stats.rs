//! 원격 서버 리소스 통계 주기 폴러.
//!
//! 대화형 셸과 **별도의 exec 채널**을 주기적으로 열어 `/proc` 통계를 읽는다(터미널
//! 출력은 오염시키지 않음). 파싱은 nabi-proto의 순수 함수가 담당.

use crate::handler::ClientHandler;
use crossbeam_channel::Sender;
use nabi_proto::statsparse::{parse_blob, PollSample, STAT_CMD};
use nabi_proto::Event;
use nabi_types::PaneId;
use russh::client;
use russh::ChannelMsg;
use std::sync::Arc;
use std::time::Duration;

/// `interval`마다 통계를 수집·전송한다. 채널 열기가 실패하면(연결 종료) 조용히 끝난다.
/// handle은 대화형 세션과 공유(Arc) — 메서드가 &self라 추가 채널을 열 수 있다.
pub(crate) async fn poll_loop(
    pane: PaneId,
    handle: Arc<client::Handle<ClientHandler>>,
    tx: Sender<Event>,
    interval: Duration,
) {
    let mut prev: Option<PollSample> = None;
    let secs = interval.as_secs().max(1);
    loop {
        tokio::time::sleep(interval).await;
        let t0 = std::time::Instant::now();
        let raw = match run_once(&handle).await {
            Some(r) => r,
            None => return, // 채널 열기/실행 실패 = 연결 종료로 간주.
        };
        let rtt = t0.elapsed().as_millis().min(u32::MAX as u128) as u32; // 통계 채널 왕복(응답 지연).
        if raw.trim().is_empty() {
            continue; // /proc 없는 서버(비-리눅스) 등 — 조용히 스킵.
        }
        let (mut stats, sample) = parse_blob(&raw, prev, secs);
        stats.rtt_ms = Some(rtt);
        prev = Some(sample);
        if tx.send(Event::ServerStats { pane, stats: Box::new(stats) }).is_err() {
            return; // UI 종료.
        }
    }
}

/// exec 채널을 한 번 열어 STAT_CMD 출력을 모은다.
async fn run_once(handle: &client::Handle<ClientHandler>) -> Option<String> {
    let mut channel = handle.channel_open_session().await.ok()?;
    channel.exec(true, STAT_CMD).await.ok()?;
    let mut buf = Vec::new();
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { data }) => buf.extend_from_slice(&data),
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}
