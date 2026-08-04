//! 여러 크레이트가 공유하는 횡단 에러 변형.

use thiserror::Error;

/// 횡단 공유 에러. 도메인별 에러는 각 크레이트에서 별도 정의한다.
#[derive(Debug, Error)]
pub enum SharedError {
    #[error("I/O 오류: {0}")]
    Io(#[from] std::io::Error),

    #[error("채널이 닫힘")]
    ChannelClosed,

    #[error("작업이 취소됨")]
    Cancelled,

    #[error("{0}")]
    Msg(String),
}

pub type SharedResult<T> = Result<T, SharedError>;

impl SharedError {
    pub fn msg(s: impl Into<String>) -> Self {
        SharedError::Msg(s.into())
    }
}
