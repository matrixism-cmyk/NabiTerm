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
    let reuse = crate::ReusedConn {
        handle: first.handle_for_reuse(),
        jump: None,
        who: nabi_ssh::conns::Who::of(&p),
    };
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
