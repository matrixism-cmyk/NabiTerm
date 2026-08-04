//! nabi-osc — OSC 7(작업 디렉터리) / OSC 133(명령 경계) 검출 탭.
//!
//! VT 파서와 병렬로 raw 바이트를 스캔해 명령 블록·cwd를 추출한다.
//! OSC 133은 best-effort(원격 셸 설정 의존) — 누락에 견고해야 한다.

mod base64;
mod oscparse;
pub mod scanner;

pub use scanner::{OscEvent, OscScanner};
