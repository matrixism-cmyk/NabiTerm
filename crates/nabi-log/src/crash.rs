//! 치명적 크래시 진단 — 동기 파일 기록 + Windows 미처리 예외 필터.
//!
//! tracing 파일 로그는 non_blocking(버퍼)이라 프로세스가 즉시 죽으면 패닉 줄이
//! 플러시 전에 유실된다. 게다가 SIGILL/액세스위반/스택오버플로 같은 네이티브 트랩은
//! Rust 패닉 훅으로 아예 못 잡는다. 여기서 둘 다 동기적으로 `crash.log`에 남겨
//! "로그 없이 즉사" 증상을 다른 머신에서도 진단 가능하게 만든다.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CRASH_FILE: OnceLock<PathBuf> = OnceLock::new();

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `crash.log`에 한 줄을 즉시 추가하고 플러시한다(버퍼 우회). 실패는 조용히 무시.
fn append(line: &str) {
    let Some(path) = CRASH_FILE.get() else { return };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
}

/// 동기 패닉 파일 기록 + (Windows) 네이티브 예외 필터를 설치한다.
///
/// `crash_dir`가 있으면 그 안 `crash.log`에 시작/패닉/네이티브 크래시를 기록한다.
/// tracing 기록도 함께 보내 기존 로그 파일에도 남긴다(가능한 경우).
pub fn install_crash_handler(crash_dir: Option<&Path>) {
    if let Some(dir) = crash_dir {
        let _ = std::fs::create_dir_all(dir);
        let _ = CRASH_FILE.set(dir.join("crash.log"));
    }
    append(&format!(
        "=== nabi v{} 시작 (pid {}, ts {}) ===",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        now_secs()
    ));
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        append(&format!("[PANIC] {info} @ {loc}"));
        tracing::error!(location = %loc, "패닉: {info}");
        prev(info);
    }));
    #[cfg(windows)]
    win::install();
}

/// 예외 코드 → 사람이 읽는 이름(진단용).
#[cfg(windows)]
fn name_for(code: u32) -> &'static str {
    match code {
        0xC000_0005 => "ACCESS_VIOLATION",
        0xC000_001D => "ILLEGAL_INSTRUCTION", // SIGILL — 빌드 머신과 다른 CPU 명령 등.
        0xC000_0096 => "PRIVILEGED_INSTRUCTION",
        0xC00000FD => "STACK_OVERFLOW",
        0xC000_008C => "ARRAY_BOUNDS_EXCEEDED",
        0xC000_0094 => "INT_DIVIDE_BY_ZERO",
        0xC000_008E => "FLT_DIVIDE_BY_ZERO",
        0xC000_0025 => "NONCONTINUABLE_EXCEPTION",
        0x8000_0003 => "BREAKPOINT",
        _ => "UNKNOWN",
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    struct ExceptionRecord {
        exception_code: u32,
        exception_flags: u32,
        exception_record: *mut c_void,
        exception_address: *mut c_void,
        number_parameters: u32,
        exception_information: [usize; 15],
    }

    #[repr(C)]
    struct ExceptionPointers {
        exception_record: *mut ExceptionRecord,
        context_record: *mut c_void,
    }

    // SAFETY: Win32 예외 필터의 ABI 정의(타입 별칭). 호출 규약을 맞추기 위한 선언이다.
    type Filter = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

    extern "system" {
        fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    }

    /// 미처리 네이티브 예외를 crash.log에 기록한 뒤 기존 처리(WER 등)로 넘긴다.
    // SAFETY: OS가 예외 시점에 유효한 포인터를 준다. as_ref로 null을 걸러 역참조하고,
    // 기록만 한 뒤 CONTINUE_SEARCH로 기본 처리에 넘긴다(상태를 바꾸지 않는다).
    unsafe extern "system" fn handler(info: *mut ExceptionPointers) -> i32 {
        if let Some(p) = info.as_ref() {
            if let Some(rec) = p.exception_record.as_ref() {
                let code = rec.exception_code;
                let addr = rec.exception_address as usize;
                super::append(&format!(
                    "[NATIVE CRASH] code=0x{code:08X} ({}) addr=0x{addr:016X} ts {}",
                    super::name_for(code),
                    super::now_secs()
                ));
            }
        }
        0 // EXCEPTION_CONTINUE_SEARCH — 우리는 기록만, 종료는 기본 처리에 맡김.
    }

    pub fn install() {
        // SAFETY: 프로세스 전역 필터를 설치한다. handler는 'static 함수 포인터라 항상 유효하다.
        unsafe {
            SetUnhandledExceptionFilter(Some(handler));
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::name_for;

    #[test]
    fn maps_known_exception_codes() {
        assert_eq!(name_for(0xC000_001D), "ILLEGAL_INSTRUCTION");
        assert_eq!(name_for(0xC000_0005), "ACCESS_VIOLATION");
        assert_eq!(name_for(0xC00000FD), "STACK_OVERFLOW");
        assert_eq!(name_for(0x1234), "UNKNOWN");
    }
}
