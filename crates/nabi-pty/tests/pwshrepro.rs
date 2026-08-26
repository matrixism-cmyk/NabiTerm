//! PowerShell 7이 목록에는 뜨는데 열리지 않는 문제 재현(사용자 보고 2026-08-26).
//!
//! 스토어판 PowerShell은 `%LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe`에
//! **앱 실행 별칭**(재분석 지점)으로 놓인다. 파일처럼 보이지만 진짜 실행 파일이 아니다.
//! 탐지는 통과하고 스폰만 실패하는지, 실패하면 어떤 오류인지 눈으로 본다.

#[test]
#[ignore = "이 PC의 설치 상태를 그대로 본다(진단용)"]
fn what_happens_when_we_open_powershell_7() {
    let shell = nabi_proto::ShellKind::Pwsh;
    let found = nabi_pty::resolve_shell(&shell);
    println!("resolve_shell(Pwsh) = {found:?}");
    if let Some(p) = &found {
        println!("  metadata          = {:?}", std::fs::metadata(p).map(|m| m.len()));
        println!("  symlink_metadata  = {:?}", std::fs::symlink_metadata(p).map(|m| m.len()));
    }

    let (tx, _rx) = crossbeam_channel::unbounded();
    let size = nabi_types::GridSize::new(80, 24);
    let r = nabi_pty::spawn_local(
        nabi_types::PaneId::new(1),
        &shell,
        size,
        tx,
        None,
        Box::new(|_| {}),
    );
    match r {
        Ok(_) => println!("스폰 성공"),
        Err(e) => println!("스폰 실패: {e}   raw_os_error={:?}", e.raw_os_error()),
    }
}
