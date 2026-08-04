//! Result/Option에 메시지 컨텍스트를 붙이는 작은 헬퍼.

use crate::kind::SharedError;

/// `.ctx("...")`로 에러에 설명을 덧붙인다.
pub trait Contextual<T> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, SharedError>;
}

impl<T, E: std::fmt::Display> Contextual<T> for Result<T, E> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, SharedError> {
        self.map_err(|e| SharedError::Msg(format!("{}: {}", msg.into(), e)))
    }
}

impl<T> Contextual<T> for Option<T> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, SharedError> {
        self.ok_or_else(|| SharedError::Msg(msg.into()))
    }
}
