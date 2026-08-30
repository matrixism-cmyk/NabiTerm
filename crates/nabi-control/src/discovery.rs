//! 실행 중인 nabiTerm을 **밖에서 찾는 길** — 탐색기 우클릭 같은 새 프로세스가 쓴다.
//!
//! 제어 파이프 이름에는 PID가 들어가고 토큰은 매 실행 무작위다. 자식 셸은 환경 변수로
//! 물려받지만, 탐색기가 새로 띄운 프로세스는 그 어느 것도 모른다.
//!
//! 그래서 앱이 뜰 때 설정 폴더에 **접속 정보 한 줄**을 남긴다. 정상 종료 때 지우고,
//! 남아 있더라도 그 PID가 살아 있는지 확인하고 쓴다(강제 종료 뒤의 낡은 파일 방지).
//!
//! ## 보안
//!
//! 파일은 사용자 설정 폴더에 있다 — 같은 사용자로 도는 프로세스만 읽을 수 있고, 그건
//! 환경 변수로 물려주던 것과 **정확히 같은 경계**다. 새로 열리는 구멍이 없다.

use std::path::{Path, PathBuf};

/// 접속 정보 파일 이름.
fn path(dir: &Path) -> PathBuf {
    dir.join("control.addr")
}

/// 지금 프로세스의 접속 정보를 남긴다(앱 시작 시 1회).
pub fn write(dir: &Path, pipe: &str, token: &str) {
    let _ = std::fs::create_dir_all(dir);
    let body = format!("{}\n{pipe}\n{token}\n", std::process::id());
    // 삼킴: 못 남기면 탐색기 '여기서 열기'만 이 창을 못 찾는다. 프로그램은 그대로 돈다.
    let _ = std::fs::write(path(dir), body);
}

/// 접속 정보를 지운다(정상 종료).
pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
}

/// 살아 있는 인스턴스의 (파이프, 토큰). 없거나 죽은 PID면 None.
pub fn read(dir: &Path) -> Option<(String, String)> {
    let raw = std::fs::read_to_string(path(dir)).ok()?;
    let (pid, pipe, token) = parse(&raw)?;
    alive(pid).then_some((pipe, token))
}

/// 파일 내용을 (pid, pipe, token)으로 가른다. 형식이 아니면 None.
pub fn parse(raw: &str) -> Option<(u32, String, String)> {
    let mut it = raw.lines();
    let pid = it.next()?.trim().parse().ok()?;
    let pipe = it.next()?.trim().to_string();
    let token = it.next()?.trim().to_string();
    (!pipe.is_empty() && !token.is_empty()).then_some((pid, pipe, token))
}

/// 그 PID가 아직 살아 있는가.
#[cfg(windows)]
fn alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: 핸들을 열어 종료 코드만 묻고 바로 닫는다. 실패하면 죽은 것으로 본다.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(h);
        ok
    }
}

#[cfg(not(windows))]
fn alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nabi-disc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_live_instance_can_be_found() {
        let d = tmp("live");
        write(&d, r"\.\pipe\x", "deadbeef");
        let got = read(&d).expect("지금 프로세스는 살아 있다");
        assert_eq!(got, (r"\.\pipe\x".to_string(), "deadbeef".to_string()));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 강제 종료로 파일이 남아도 **죽은 PID면 쓰지 않는다** — 없는 곳에 붙으려 하면 안 된다.
    #[test]
    fn a_stale_file_from_a_dead_process_is_ignored() {
        let d = tmp("stale");
        // 존재할 리 없는 PID(윈도우 PID는 4의 배수이고 이 값은 범위 밖에 가깝다).
        std::fs::write(path(&d), "4294967294\npipe-that-does-not-exist\ntok\n").unwrap();
        assert!(read(&d).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_clean_exit_leaves_nothing_behind() {
        let d = tmp("clean");
        write(&d, "p", "t");
        clear(&d);
        assert!(read(&d).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn junk_is_not_mistaken_for_an_address() {
        assert!(parse("").is_none());
        assert!(parse("not a pid\npipe\ntok").is_none());
        assert!(parse("123\n\n\n").is_none(), "빈 파이프·토큰은 주소가 아니다");
        assert!(parse("123\npipe").is_none(), "줄이 모자라면 주소가 아니다");
    }
}
