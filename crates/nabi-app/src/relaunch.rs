//! **껐다 다시 켜기** — 우리가 사라진 뒤에 우리를 다시 띄운다.
//!
//! ## 왜 도우미가 필요한가
//!
//! 자기 자신을 다시 띄우려면 먼저 죽어야 하는데, 죽은 뒤에는 아무것도 할 수 없다.
//! 그래서 **먼저 도우미를 하나 걸어 두고** 나간다. 도우미는 우리 PID 가 사라지기를
//! 기다렸다가 같은 exe 를 다시 시작한다.
//!
//! 업데이트 설치가 이미 같은 길을 쓴다(`nabi_release::run_after_exit`) — 인스톨러는
//! 실행 중인 나비텀이 있으면 멈추기 때문이다. 여기서는 인스톨러 대신 우리 자신을 띄운다.
//!
//! ## 왜 먼저 죽어야 하는가
//!
//! 두 판이 겹쳐 뜨면 설정·작업 공간을 같은 파일에 쓰면서 서로 덮는다. 나가는 쪽이
//! 마지막에 저장하므로, 새로 뜬 쪽이 먼저 읽었다면 **방금 한 일이 사라진다.**

/// 도우미 모드의 낱말. `main` 이 이 인자를 보면 GUI 를 띄우지 않고 [`wait_and_start`] 로 간다.
pub(crate) const RELAUNCH_AFTER: &str = "--relaunch-after";

/// 우리가 사라진 뒤 우리를 다시 띄우도록 도우미를 걸어 둔다.
///
/// 실패하면 조용히 넘어간다 — 그 경우 그냥 종료된다. 다시 켜지지 않는 것은 불편이지만,
/// 여기서 종료를 막으면 사용자가 끝내려던 일 자체가 안 된다.
pub(crate) fn arm() {
    let Ok(me) = std::env::current_exe() else { return };
    let mut cmd = std::process::Command::new(&me);
    cmd.args([RELAUNCH_AFTER, &std::process::id().to_string()]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // 작업 폴더를 명시한다 — 우리 CWD 가 사라진 폴더나 UNC 면 자식 만들기가 실패한다.
    if let Some(dir) = me.parent() {
        cmd.current_dir(dir);
    }
    // 삼킴: 못 걸어도 종료는 진행한다. 위 문단 참고.
    let _ = cmd.spawn();
}

/// 도우미 모드: `pid` 가 끝나기를 기다렸다가 우리를 다시 띄운다.
pub(crate) fn wait_and_start(pid_arg: &str) -> i32 {
    // 기다림에 끝을 둔다 — 그 PID 가 영영 안 죽으면 도우미가 영원히 남는다.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    if let Ok(pid) = pid_arg.parse::<u32>() {
        while nabi_release::process_alive(pid) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
    }
    // 파일 손잡이가 풀릴 짬을 준다 — 방금 나간 판이 설정을 쓰는 중일 수 있다.
    std::thread::sleep(std::time::Duration::from_millis(400));
    let Ok(me) = std::env::current_exe() else { return 1 };
    match std::process::Command::new(me).spawn() {
        Ok(_) => 0,
        Err(_) => 1,
    }
}
