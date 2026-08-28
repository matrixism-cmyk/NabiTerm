//! 실서버 대상 **연결 재사용** 검증(배치 Y V1·V3) — `#[ignore]`, 기본 게이트 제외.
//!
//! 인프로세스 시험으로는 이것을 증명할 수 없다. 우리가 만든 서버는 우리가 기대하는 대로만
//! 답하는데, 여기서 알고 싶은 것은 **진짜 OpenSSH가 한 연결 위에 SFTP 서브시스템 채널을
//! 두 번 열어 주는가**이기 때문이다. 실제로 이 저장소에서 SFTP 결함은 두 번 다 실서버에서만
//! 드러났다.
//!
//! 실행:
//! ```text
//! NABI_RT_USER=... NABI_RT_KEY=<개인키> cargo test -p nabi-sftp reuse -- --ignored --nocapture
//! ```
//! 환경변수가 없으면 조용히 통과한다(미구성 환경 안전).

use crate::realserver_test::params;
use nabi_fs::RemoteFs;

/// **한 연결에 SFTP를 두 번 연다** — 이 배치의 핵심 주장.
///
/// 첫 연결의 핸들을 그대로 물려 두 번째 SFTP를 열고, 그것으로 실제 목록을 읽는다.
/// 인증은 첫 번째에서 한 번만 일어난다 — 두 번째는 채널만 더 여는 것이라 서버에 세션이
/// 하나로 남는다.
///
/// 이것이 깨지면 사용자에게는 "SFTP 열기(같은 서버)"가 조용히 실패하는 것으로 보인다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_second_sftp_rides_the_first_connection() {
    let Some(p) = params() else { return };
    let kh = crate::sftp_boot::test_known_hosts();

    // 이 시험만의 표식 폴더를 만든다. 홈 목록의 **개수**를 비교하면 같은 서버에서 도는
    // 다른 시험이 파일을 만들고 지우는 동안 값이 흔들린다(처음에 그렇게 적었다가 단독
    // 실행에서만 통과하고 전체 실행에서 깨졌다). 개수가 아니라 **내가 만든 것이 보이는가**로
    // 묻는다 — 그것이 실제로 알고 싶은 것이기도 하다.
    let marker = format!("nabi-reuse-{}", std::process::id());

    let mut first = crate::connect_sftp(&p, kh.clone(), None)
        .await
        .expect("첫 SFTP 연결");
    let _ = first.remove(&marker).await; // 앞선 실행이 남긴 것이 있으면 치운다.
    first.mkdir(&marker).await.expect("첫 연결로 표식 만들기");

    // 첫 연결의 핸들을 물려 두 번째를 연다. 여기서 인증은 일어나지 않는다.
    let reuse = crate::ReusedConn::new(
        first.handle_for_reuse(),
        None,
        nabi_ssh::conns::Who::of(&p),
    );
    let mut second = crate::connect_sftp_reusing(&p, kh, None, Some(reuse))
        .await
        .expect("물려받은 연결 위에 두 번째 SFTP");

    let sees = |v: &Vec<nabi_fs::FileEntry>| v.iter().any(|e| e.name == marker);
    let after = second.list_dir(".").await.expect("두 번째 연결로 목록");
    assert!(sees(&after), "물려받은 연결이 같은 계정의 같은 홈을 봐야 한다");

    // 첫 번째를 놓아도 두 번째는 살아 있어야 한다 — `Arc` 로 든 이유가 이것이다.
    drop(first);
    let still = second.list_dir(".").await.expect("첫 것을 놓아도 두 번째는 산다");
    assert!(sees(&still), "첫 세션을 닫아도 물려받은 연결은 계속 쓴다");

    second.remove(&marker).await.expect("표식 치우기");
}

/// 재사용하지 않는 **기존 경로가 그대로 동작하는가**(V3 — 회귀 확인).
///
/// 되던 것이 깨지면 그것은 이 배치가 만든 회귀다. 재사용은 얹은 길일 뿐이고, 물려줄
/// 연결이 없을 때는 예전과 완전히 같아야 한다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_fresh_connection_still_works() {
    let Some(p) = params() else { return };
    let kh = crate::sftp_boot::test_known_hosts();
    let mut fs = crate::connect_sftp_reusing(&p, kh, None, None)
        .await
        .expect("물려줄 연결이 없으면 새로 연결한다");
    fs.list_dir(".").await.expect("새 연결로 목록");
}

/// **점프 호스트를 거쳐도 연결을 물려받는가**(배치 Y V2 — 그때 못 하고 남겨 둔 것).
///
/// 배치 Y 에서는 "서버가 둘 필요한데 이 PC 에는 OpenSSH 가 하나뿐"이라 못 했다고 적었다.
/// 두 번째 sshd 를 다른 포트로 띄우면 **진짜 두 홉**이 된다 — 경유지와 목적지가 서로 다른
/// 프로세스다. 이 시험이 확인하는 것은 **점프 호스트를 거쳐도 연결을 물려받아 두 번째 SFTP 를
/// 열 수 있는가**다. "터널이 한 벌인가"까지는 확인하지 못했다(아래 주석 참조).
///
/// 실행(경유 22 → 목적지 2222):
/// ```text
/// NABI_RT_USER=... NABI_RT_KEY=<개인키> NABI_JUMP_PORT=2222 ///   cargo test -p nabi-sftp jump -- --ignored --nocapture
/// ```
/// `NABI_JUMP_PORT` 가 없으면 조용히 통과한다(두 번째 서버가 없는 환경 안전).
#[tokio::test]
#[ignore = "두 번째 sshd 필요(NABI_JUMP_PORT)"]
async fn realserver_jump_host_reuses_one_tunnel() {
    let Some(base) = params() else { return };
    let Ok(port) = std::env::var("NABI_JUMP_PORT") else { return };
    let Ok(port) = port.parse::<u16>() else { return };

    // 목적지는 2222, 경유지는 원래 params(22).
    let mut target = base.clone();
    target.port = port;
    target.jump = Some(Box::new(base.clone()));

    let kh = crate::sftp_boot::test_known_hosts();
    let mut first = crate::connect_sftp(&target, kh.clone(), None)
        .await
        .expect("점프 호스트를 거친 첫 SFTP");
    let marker = format!("nabi-jump-{}", std::process::id());
    let _ = first.remove(&marker).await;
    first.mkdir(&marker).await.expect("첫 연결로 표식 만들기");

    // 목적지 핸들과 점프 핸들을 함께 물려준다.
    //
    // ⚠️ **이 시험은 점프 핸들이 꼭 필요한지는 증명하지 못한다.** 일부러 `None` 을 넣어
    // 봤는데도 통과했다 — russh 의 배경 태스크가 세션을 붙들고 있어서 `Handle` 을 놓아도
    // 터널이 곧바로 끊기지 않는 것으로 보인다. 그러니 "빠뜨리면 끊긴다"고 단정하지 않는다.
    //
    // 그래도 함께 물려준다. 수명이 구현 세부에 기대고 있을 뿐이고, 그 세부는 바뀔 수 있다.
    let reuse = crate::ReusedConn::new(
        first.handle_for_reuse(),
        first.jump_for_reuse(),
        nabi_ssh::conns::Who::of(&target),
    );
    let mut second = crate::connect_sftp_reusing(&target, kh, None, Some(reuse))
        .await
        .expect("물려받은 연결 위에 두 번째 SFTP");

    let after = second.list_dir(".").await.expect("두 번째 연결로 목록");
    assert!(
        after.iter().any(|e| e.name == marker),
        "물려받은 연결이 같은 목적지를 봐야 한다(터널이 살아 있어야 한다)"
    );

    // 첫 세션을 놓아도 물려받은 쪽은 계속 쓴다(직접 연결 때와 같다).
    drop(first);
    let still = second.list_dir(".").await.expect("첫 것을 놓아도 터널은 산다");
    assert!(still.iter().any(|e| e.name == marker));

    second.remove(&marker).await.expect("표식 치우기");
}
