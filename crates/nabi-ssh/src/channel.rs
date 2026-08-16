//! SSH용 ByteChannel 구현(입력을 tokio 태스크로 전달).

use nabi_pty::ByteChannel;
use nabi_types::GridSize;
use std::io;
use tokio::sync::mpsc::UnboundedSender;

/// SSH 태스크로 보내는 입력 메시지.
pub enum SshInput {
    Data(Vec<u8>),
    Resize(u16, u16),
}

/// 원격 pane의 입력 싱크. 출력은 별도 출력 버스로 흐른다.
pub struct SshChannel {
    tx: UnboundedSender<SshInput>,
}

impl SshChannel {
    pub fn new(tx: UnboundedSender<SshInput>) -> Self {
        Self { tx }
    }
}

impl ByteChannel for SshChannel {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.tx
            .send(SshInput::Data(data.to_vec()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, nabi_i18n::trc("net.chan.closed")))
    }

    fn resize(&mut self, size: GridSize) -> io::Result<()> {
        self.tx
            .send(SshInput::Resize(size.cols(), size.rows()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, nabi_i18n::trc("net.chan.closed")))
    }
}
