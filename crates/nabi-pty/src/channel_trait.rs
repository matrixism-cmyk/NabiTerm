//! 로컬 PTY와 원격 SSH 채널이 공유하는 바이트 채널 추상화.

use nabi_types::GridSize;
use std::io;

/// pane의 입력 싱크 + 리사이즈. 출력은 스폰 시 등록한 출력 버스로 흘러간다.
pub trait ByteChannel: Send {
    /// 입력 바이트를 자식(셸/원격)에 쓴다.
    fn write(&mut self, data: &[u8]) -> io::Result<()>;
    /// 터미널 크기를 변경한다(ConPTY resize / SSH window_change).
    fn resize(&mut self, size: GridSize) -> io::Result<()>;
}
