//! 비밀 메모리 와이프 헬퍼.

pub use zeroize::Zeroizing;

/// drop 시 메모리가 와이프되는 비밀 문자열.
pub type SecretString = Zeroizing<String>;
