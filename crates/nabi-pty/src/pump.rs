//! PTY 출력 리더 스레드.
//!
//! ConPTY 파이프는 동기식이므로 pane마다 블로킹 리더 스레드를 둔다.
//! 읽은 바이트를 오케스트레이터의 출력 버스로 흘려보낸다.

use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_types::PaneId;
use std::io::Read;

/// pane의 PTY 리더 스레드를 띄운다. EOF/에러 시 종료한다.
pub fn spawn_reader(pane: PaneId, mut reader: Box<dyn Read + Send>, out_tx: Sender<(PaneId, Bytes)>) {
    let _ = std::thread::Builder::new()
        .name(format!("pty-reader-{}", pane.get()))
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if out_tx.send((pane, Bytes::copy_from_slice(&buf[..n]))).is_err() {
                            break;
                        }
                    }
                }
            }
        });
}

/// 자식 프로세스를 wait하는 스레드 — 종료 시 `on_exit(코드)` 호출(자연 종료 통지).
/// ConPTY는 자식이 죽어도 마스터가 살아 있으면 reader EOF가 안 와서 wait로 감지한다.
pub fn spawn_child_waiter(
    pane: PaneId,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    on_exit: Box<dyn FnOnce(Option<i32>) + Send>,
) {
    let _ = std::thread::Builder::new()
        .name(format!("pty-wait-{}", pane.get()))
        .spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code() as i32);
            on_exit(code);
        });
}
