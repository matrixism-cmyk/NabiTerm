//! PTY 출력 리더 스레드.
//!
//! ConPTY 파이프는 동기식이므로 pane마다 블로킹 리더 스레드를 둔다.
//! 읽은 바이트를 오케스트레이터의 출력 버스로 흘려보낸다.

use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_types::PaneId;
use std::io::Read;

/// 출력 버스 용량(청크 수). 폭주 pane이 메모리를 무한히 먹지 않도록 상한을 둔다.
/// 상한에 닿으면 리더가 블록 → 커널 PTY 버퍼가 참 → 자식이 write에서 대기(흐름 제어).
pub const OUT_BUS_CAPACITY: usize = 256;

/// PTY 한 번 읽기 크기. 8KiB는 현대 터미널 기준(64KiB~1MiB)보다 훨씬 작아 폭주 출력에서
/// 청크 수와 깨우기 횟수가 불필요하게 많아진다.
const READ_BUF: usize = 64 * 1024;

/// pane의 PTY 리더 스레드를 띄운다. EOF/에러 시 종료한다.
pub fn spawn_reader(pane: PaneId, mut reader: Box<dyn Read + Send>, out_tx: Sender<(PaneId, Bytes)>) {
    let _ = std::thread::Builder::new()
        .name(format!("pty-reader-{}", pane.get()))
        .spawn(move || {
            let mut buf = [0u8; READ_BUF];
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
