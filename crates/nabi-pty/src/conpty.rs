//! ConPTY 로컬 셸 핸들 + 스폰.

use crate::channel_trait::ByteChannel;
use crate::spawn::build_command;
use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_proto::ShellKind;
use nabi_types::{GridSize, PaneId};
use portable_pty::{native_pty_system, MasterPty, PtySize};
use std::io::{self, Write};

/// 살아있는 로컬 ConPTY 셸. 입력 쓰기와 리사이즈를 제공한다.
/// 자식 핸들은 waiter 스레드가 소유(종료 감지) — 마스터 드롭 시 자식도 HUP으로 종료된다.
pub struct LocalPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
}

/// 로컬 셸을 스폰하고 리더+waiter 스레드를 띄운다. 출력은 `out_tx`로 흘러간다.
/// `on_exit`는 셸 종료 시 종료 코드와 함께 호출된다(PaneExited 통지).
pub fn spawn_local(
    pane: PaneId,
    shell: &ShellKind,
    size: GridSize,
    out_tx: Sender<(PaneId, Bytes)>,
    cwd: Option<&str>,
    on_exit: Box<dyn FnOnce(Option<i32>) + Send>,
) -> io::Result<LocalPty> {
    // 스폰 전에 셸 실행파일 존재를 확인 — 없으면 ConPTY 행(60초 타임아웃) 대신 즉시 명확한 에러.
    if crate::spawn::resolve_shell(shell).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("셸 실행파일을 찾을 수 없습니다: {} (설치되어 있지 않음)", crate::spawn::program_name(shell)),
        ));
    }
    let sys = native_pty_system();
    let pair = sys.openpty(to_pty_size(size)).map_err(other)?;
    let mut cmd = build_command(shell);
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    // 제어 평면 자기 식별(파이프·토큰은 부모 프로세스 env로 상속됨).
    cmd.env("NABI_PANE_ID", pane.get().to_string());
    let child = pair.slave.spawn_command(cmd).map_err(other)?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().map_err(other)?;
    let writer = pair.master.take_writer().map_err(other)?;
    crate::pump::spawn_reader(pane, reader, out_tx);
    crate::pump::spawn_child_waiter(pane, child, on_exit);
    Ok(LocalPty {
        master: pair.master,
        writer,
    })
}

impl ByteChannel for LocalPty {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }

    fn resize(&mut self, size: GridSize) -> io::Result<()> {
        self.master.resize(to_pty_size(size)).map_err(other)
    }
}

fn to_pty_size(s: GridSize) -> PtySize {
    PtySize {
        rows: s.rows(),
        cols: s.cols(),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn other(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
