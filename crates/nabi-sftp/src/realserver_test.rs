//! 실 SSH/SFTP 서버 대상 통합테스트(#[ignore] — 기본 게이트 제외, 실서버 동작 검증용).
//!
//! 실행: 서버 정보를 환경변수로 주고 `cargo test -p nabi-sftp -- --ignored`.
//!   NABI_RT_USER(필수) NABI_RT_KEY(개인키 경로, 필수) NABI_RT_HOST(기본 127.0.0.1) NABI_RT_PORT(기본 22)
//! USER/KEY 미설정 시 조용히 통과(미구성 환경 안전). 상대 경로는 서버가 홈 기준으로 해석한다.

use crate::connect_sftp;
use nabi_fs::RemoteFs;
use nabi_proto::SshParams;

pub(crate) fn params() -> Option<SshParams> {
    let user = std::env::var("NABI_RT_USER").ok()?;
    let host = std::env::var("NABI_RT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("NABI_RT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
    // NABI_RT_PASS가 있으면 비밀번호 인증, 아니면 NABI_RT_KEY 개인키 인증.
    if let Ok(pass) = std::env::var("NABI_RT_PASS") {
        return Some(SshParams::password(host, port, user, pass));
    }
    let key = std::env::var("NABI_RT_KEY").ok()?;
    Some(SshParams::key_file(host, port, user, key, None))
}

/// 옛 SSH 서버(OpenSSH 4.x 등)에 SFTP로 붙는지 — **읽기 전용**이라 남의 서버에 써도 안전하다.
///
/// 터미널 쪽 폴백은 nabi-ssh에서 검증하지만, SFTP는 연결 코드가 따로라 여기서도 확인한다.
#[tokio::test]
#[ignore = "옛 SSH 서버 필요(NABI_OLD_HOST/USER/PASS)"]
async fn realserver_legacy_sftp_connects() {
    let (Ok(host), Ok(user), Ok(pass)) = (
        std::env::var("NABI_OLD_HOST"),
        std::env::var("NABI_OLD_USER"),
        std::env::var("NABI_OLD_PASS"),
    ) else {
        return;
    };
    let port: u16 = std::env::var("NABI_OLD_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
    let p = SshParams::password(host, port, user, pass);
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None)
        .await
        .expect("옛 서버에 SFTP 연결");
    let entries = fs.list_dir(".").await.expect("홈 목록 조회");
    println!("옛 서버 홈 항목 {}개", entries.len());
}

/// 옛 서버(SFTP v3)에서 **다운로드 파이프라인**이 실제로 도는지 — 읽기 전용.
///
/// OpenSSH 4.x의 sftp-server는 `limits@openssh.com`도 `statvfs`도 없다. 우리 파이프라인은
/// 그 확장이 있을 때를 기준으로 만들어졌고, 없을 때의 기본값 경로는 실서버로 확인한 적이
/// 없었다. 남의 운영 서버라 **쓰기는 하지 않는다** — 있는 파일을 받아 크기만 대조한다.
#[tokio::test]
#[ignore = "옛 SSH 서버 필요(NABI_OLD_HOST/USER/PASS)"]
async fn realserver_legacy_download_pipeline() {
    let (Ok(host), Ok(user), Ok(pass)) = (
        std::env::var("NABI_OLD_HOST"),
        std::env::var("NABI_OLD_USER"),
        std::env::var("NABI_OLD_PASS"),
    ) else {
        return;
    };
    let port: u16 = std::env::var("NABI_OLD_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
    let p = SshParams::password(host, port, user, pass);
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("연결");
    // 파도를 여러 번 돌 만큼은 크되 **너무 크지 않은** 파일을 고른다.
    // 남의 운영 서버에서 수백 MB짜리 업무 데이터를 끌어올 이유가 없다(처음에 193MB짜리
    // SQL 덤프를 받아 버렸다). 상한 안에서 가장 큰 것을 쓴다.
    const MAX: u64 = 20 * 1024 * 1024;
    let mut files: Vec<_> = fs
        .list_dir(".")
        .await
        .expect("목록")
        .into_iter()
        .filter(|e| matches!(e.kind, nabi_fs::FileKind::File) && e.size > 0 && e.size <= MAX)
        .collect();
    files.sort_by_key(|e| std::cmp::Reverse(e.size));
    let Some(target) = files.first().cloned() else { return };
    println!("옛 서버 대상: {} ({} 바이트)", target.name, target.size);
    let local = std::env::temp_dir().join(format!("nabi-legacy-dl-{}.bin", std::process::id()));
    let lp = local.to_string_lossy().into_owned();
    let mut seen = 0u64;
    fs.download(&target.name, &lp, 0, |b| seen = b).await.expect("다운로드");
    let got = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&local);
    assert_eq!(got, target.size, "받은 크기가 원격 크기와 달라졌다");
    assert_eq!(seen, target.size, "진행률 합계가 파일 크기와 달라졌다");
}

/// ssh-agent 인증으로 실제 서버에 붙는다(NABI_RT_AGENT=1일 때만).
///
/// 준비: 에이전트를 켜고(`Set-Service ssh-agent -StartupType Manual; Start-Service ssh-agent`)
/// `ssh-add`로 키를 올린 뒤, 그 공개키를 서버 사용자의 authorized_keys에 넣는다.
/// 이 경로는 인프로세스 서버로는 검증할 수 없다 — 에이전트가 실제로 서명해야 한다.
#[tokio::test]
#[ignore = "실 서버 + ssh-agent 필요(NABI_RT_AGENT=1)"]
async fn realserver_agent_auth() {
    if std::env::var("NABI_RT_AGENT").is_err() {
        return;
    }
    // params()를 쓰지 않는다 — 그쪽은 비밀번호나 키가 있어야 Some을 준다(에이전트는 둘 다 없다).
    let Ok(user) = std::env::var("NABI_RT_USER") else { return };
    let host = std::env::var("NABI_RT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("NABI_RT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
    let p = SshParams::agent(host, port, user);
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None)
        .await
        .expect("에이전트 인증으로 연결");
    // 붙기만 한 게 아니라 실제로 쓸 수 있는 세션인지 확인한다.
    fs.list_dir(".").await.expect("목록 조회");
}

/// 실 OpenSSH 서버에서 업로드 원자적 교체(rename-over-existing) 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_upload_atomic_overwrites() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let remote = "nabi_realtest_up.bin";
    let local = std::env::temp_dir().join(format!("nabi-rt-up-{}.bin", std::process::id()));
    // 1) 최초 업로드.
    std::fs::write(&local, b"first-content").unwrap();
    fs.upload(local.to_str().unwrap(), remote, |_| {}).await.expect("upload1");
    assert_eq!(fs.read_file(remote).await.expect("read1"), b"first-content");
    // 2) 기존 위에 재업로드 → 원자적 교체(OpenSSH rename-over-existing 처리 검증).
    std::fs::write(&local, b"second-longer-content").unwrap();
    fs.upload(local.to_str().unwrap(), remote, |_| {}).await.expect("upload2");
    assert_eq!(fs.read_file(remote).await.expect("read2"), b"second-longer-content");
    // 3) 임시 파일 잔여 없어야.
    assert!(fs.read_file(&format!("{remote}.filepart")).await.is_err(), ".filepart 잔여 없어야");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&local);
}

/// 실 OpenSSH 서버에서 비어있지 않은 폴더의 재귀 삭제 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_remove_recursive() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let dir = "nabi_realtest_rmdir";
    let _ = fs.mkdir(dir).await;
    let src = std::env::temp_dir().join(format!("nabi-rt-rm-{}.bin", std::process::id()));
    std::fs::write(&src, b"x").unwrap();
    fs.upload(src.to_str().unwrap(), &format!("{dir}/a.bin"), |_| {}).await.expect("up");
    fs.remove_recursive(dir).await.expect("rm -r"); // 내용 있어도 삭제.
    let names: Vec<String> = fs.list_dir(".").await.expect("list").into_iter().map(|e| e.name).collect();
    assert!(!names.contains(&dir.to_string()), "재귀 삭제로 폴더가 사라져야: {names:?}");
    let _ = std::fs::remove_file(&src);
}

/// 실 OpenSSH 서버에서 폴더 크기 재귀 계산 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_dir_size() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let dir = "nabi_realtest_szdir";
    let _ = fs.mkdir(dir).await;
    let src = std::env::temp_dir().join(format!("nabi-rt-sz-{}.bin", std::process::id()));
    std::fs::write(&src, b"12345").unwrap(); // 5바이트
    fs.upload(src.to_str().unwrap(), &format!("{dir}/a.bin"), |_| {}).await.expect("up a");
    std::fs::write(&src, b"6789").unwrap(); // 4바이트
    fs.upload(src.to_str().unwrap(), &format!("{dir}/b.bin"), |_| {}).await.expect("up b");
    assert_eq!(fs.dir_stats(dir).await, (2, 0, 9), "파일2·폴더0·5+4바이트");
    let _ = fs.remove(&format!("{dir}/a.bin")).await;
    let _ = fs.remove(&format!("{dir}/b.bin")).await;
    let _ = fs.remove(dir).await;
    let _ = std::fs::remove_file(&src);
}

/// 실 OpenSSH 서버에서 다운로드 시 수정시각 보존(로컬 파일 mtime = 원격 mtime) 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_download_preserves_mtime() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let remote = "nabi_realtest_mtime.bin";
    let src = std::env::temp_dir().join(format!("nabi-rt-mt-src-{}.bin", std::process::id()));
    std::fs::write(&src, b"mtime-test").unwrap();
    fs.upload(src.to_str().unwrap(), remote, |_| {}).await.expect("upload");
    // 원격 파일의 실제 mtime을 홈 목록에서 조회.
    let rmt = fs.list_dir(".").await.expect("list").into_iter()
        .find(|e| e.name == remote).map(|e| e.mtime).expect("entry");
    let dst = std::env::temp_dir().join(format!("nabi-rt-mt-dst-{}.bin", std::process::id()));
    fs.download(remote, dst.to_str().unwrap(), 0, |_| {}).await.expect("download");
    let lmt = std::fs::metadata(&dst).unwrap().modified().unwrap()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(lmt, rmt, "다운로드 파일 mtime이 원격 mtime과 일치해야");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

/// 실 OpenSSH 서버에서 업로드 이어받기(남은 원격 .filepart부터 재개) 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_upload_resumes_filepart() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let remote = "nabi_realtest_upresume.bin";
    let content = b"ABCDEFGHIJKLMNOP"; // 16바이트.
    // 부분 원격 .filepart(앞 6바이트)를 만들어 중단된 업로드를 흉내낸다.
    fs.write_file(&format!("{remote}.filepart"), &content[..6]).await.expect("seed part");
    let local = std::env::temp_dir().join(format!("nabi-rt-upres-{}.bin", std::process::id()));
    std::fs::write(&local, content).unwrap();
    // 전체 로컬을 업로드 → 오프셋 6부터 이어서 올리고 완료 시 remote로 교체.
    fs.upload(local.to_str().unwrap(), remote, |_| {}).await.expect("upload resume");
    assert_eq!(fs.read_file(remote).await.expect("read"), content);
    assert!(fs.read_file(&format!("{remote}.filepart")).await.is_err(), ".filepart 없어야");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&local);
}

/// 실 OpenSSH 서버에서 download 이어받기(.filepart → 완료 rename) 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_download_resume_filepart() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let remote = "nabi_realtest_dl.bin";
    let content = b"0123456789ABCDEF";
    // 원격에 알려진 내용 준비(원자적 업로드로).
    let src = std::env::temp_dir().join(format!("nabi-rt-src-{}.bin", std::process::id()));
    std::fs::write(&src, content).unwrap();
    fs.upload(src.to_str().unwrap(), remote, |_| {}).await.expect("seed upload");
    // 부분 .filepart(앞 6바이트)를 만들고 오프셋 6부터 이어받기.
    let local = std::env::temp_dir().join(format!("nabi-rt-dl-{}.bin", std::process::id()));
    let part = format!("{}.filepart", local.to_str().unwrap());
    std::fs::write(&part, &content[..6]).unwrap();
    fs.download(remote, local.to_str().unwrap(), 6, |_| {}).await.expect("resume dl");
    assert_eq!(std::fs::read(&local).expect("read dl"), content);
    assert!(!std::path::Path::new(&part).exists(), "완료 후 .filepart 없어야");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&local);
    let _ = std::fs::remove_file(&src);
}

/// 미리보기(앞부분만 읽기)를 **실 서버**에 대고 확인한다.
///
/// 인프로세스 서버는 우리가 만든 것이라 우리가 기대하는 대로만 답한다. 부분 읽기는 특히
/// 서버 사정을 타는 곳이다 — 한 번의 read가 요청한 만큼을 다 주지 않고 쪼개서 주는 서버가
/// 흔하다. 그걸 안 겪어 보면 "한 번 읽고 끝"이라는 잘못된 코드가 시험을 통과한다.
#[tokio::test]
#[ignore = "실 SFTP 서버 필요(NABI_RT_USER + KEY/PASS)"]
async fn realserver_preview_reads_only_the_head() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("연결");
    let base = format!("nabi-prev-{}", std::process::id());

    // ① 상한보다 확실히 큰 파일 — 앞부분만 오고, 더 있다고 말해야 한다.
    const MAX: usize = 8 * 1024;
    let big_path = format!("{base}-big.txt");
    let big: Vec<u8> = (0..4000u32).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
    assert!(big.len() > MAX * 2, "시험 파일이 상한보다 커야 뜻이 있다");
    fs.write_file(&big_path, &big).await.expect("업로드");
    let (head, more) = fs.preview(&big_path, MAX).await.expect("미리보기");
    assert_eq!(head.len(), MAX, "상한만큼 채워 오지 않았다(쪼개 주는 서버에서 깨진다)");
    assert_eq!(head, big[..MAX], "앞부분이 원본과 다르다");
    assert!(more, "뒤에 더 있는데 없다고 했다");

    // ② 상한보다 작은 파일 — 통째로 오고, 더 없다고 해야 한다.
    let small_path = format!("{base}-small.txt");
    let small = b"hello remote preview\n".to_vec();
    fs.write_file(&small_path, &small).await.expect("업로드");
    let (all, more2) = fs.preview(&small_path, MAX).await.expect("미리보기");
    assert_eq!(all, small);
    assert!(!more2, "작은 파일인데 더 있다고 했다");

    // ③ **딱 상한만큼인 파일** — 여기가 거짓말하기 쉬운 자리다. "상한을 채웠으니 더 있다"고
    //    넘겨짚으면 이 경우에 "앞부분만"이라는 잘못된 안내가 뜬다.
    let exact_path = format!("{base}-exact.bin");
    let exact = vec![b'x'; MAX];
    fs.write_file(&exact_path, &exact).await.expect("업로드");
    let (got, more3) = fs.preview(&exact_path, MAX).await.expect("미리보기");
    assert_eq!(got.len(), MAX);
    assert!(!more3, "딱 상한만큼인데 '더 있다'고 했다 — 사용자에게 거짓말이 된다");

    // ④ 빈 파일도 터지지 않아야 한다.
    let empty_path = format!("{base}-empty.txt");
    fs.write_file(&empty_path, b"").await.expect("업로드");
    let (none, more4) = fs.preview(&empty_path, MAX).await.expect("미리보기");
    assert!(none.is_empty() && !more4);

    for p in [&big_path, &small_path, &exact_path, &empty_path] {
        let _ = fs.remove(p).await;
    }
}

/// 여유 공간 조회를 **실 서버**에 대고 확인한다.
///
/// 이 값은 서버가 `statvfs` 확장을 지원할 때만 온다. 지원 여부는 서버마다 다르고,
/// 인프로세스 서버로는 "지원한다고 답하게" 만들 수 있어 아무것도 증명하지 못한다.
/// 여기서 확인할 것은 **둘 중 하나로 분명히 답하는가**다 — 그럴듯한 0을 내지 않는가.
#[tokio::test]
#[ignore = "실 SFTP 서버 필요(NABI_RT_USER + KEY/PASS)"]
async fn realserver_free_space_is_known_or_honestly_unknown() {
    let Some(p) = params() else { return };
    let fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("연결");
    let got = fs.free_space(".").await;
    eprintln!("free_space(.) = {got:?}");
    match got {
        // 지원한다면 0일 리 없다 — 0이 오면 그건 "모른다"를 잘못 표현한 것이다.
        Some(n) => assert!(n > 0, "여유를 0으로 답했다 — 모르는 것을 0으로 내면 거짓말이 된다"),
        // 지원하지 않으면 None이어야 한다(이쪽도 올바른 답이다).
        None => eprintln!("이 서버는 statvfs를 지원하지 않는다 — None이 맞는 답이다"),
    }
}

/// 실 OpenSSH 서버에서 **서버 안 복사** 검증.
///
/// 인프로세스 서버는 우리가 만든 것이라 우리가 기대하는 대로만 답한다. 실제로 두 번 다
/// 실서버에서만 결함이 나왔다(읽기 길이를 허용치보다 적게 주는 서버, rename 덮어쓰기).
/// 복사는 읽기와 쓰기를 번갈아 하므로 그런 차이가 그대로 드러난다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_copies_within_the_server() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let src = "nabi_realtest_copy_src.bin";
    let dst = "nabi_realtest_copy_dst.bin";
    // 한 조각에 안 들어가는 크기로 — 조각 경계에서 어긋나는지 보려면 이게 필요하다.
    let body: Vec<u8> = (0..900_000u32).map(|i| (i % 253) as u8).collect();
    fs.write_file(src, &body).await.expect("write src");
    let mut last = 0u64;
    let n = fs.copy_remote(src, dst, &mut |b| last = b).await.expect("copy");
    assert_eq!(n as usize, body.len(), "복사한 바이트 수가 다르다");
    assert_eq!(last, n, "진행률이 끝까지 오지 않았다");
    let got = fs.read_file(dst).await.expect("read back");
    assert_eq!(got.len(), body.len(), "길이가 다르다");
    assert_eq!(got, body, "내용이 달라졌다");
    let _ = fs.remove(src).await;
    let _ = fs.remove(dst).await;
}

/// **알던 호스트키가 바뀌면 그렇다고 알려야 한다** — 실서버에서만 볼 수 있는 갈래다.
///
/// 인프로세스 서버로는 흉내 낼 수 없다: `known_hosts`에 **다른 키**를 미리 적어 두고
/// 진짜 서버에 붙어야 러시가 `KeyChanged`를 돌려준다. 지금까지 이 갈래는 조용히
/// 거부만 했고(`Err(_) => Ok(false)`), 그래서 아무도 눈치채지 못했다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_reports_a_changed_host_key() {
    use std::sync::{Arc, Mutex};
    let Some(p) = params() else { return };
    let kh = crate::sftp_boot::test_known_hosts();
    // 이 호스트의 키라고 **거짓으로** 적어 둔다(형식은 맞지만 다른 키).
    let fake = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb7GQ2p7DbFPuhVpzOSVQXHDzHfF1lVMSCUmJ8UN0Rp";
    let line = match p.port {
        22 => format!("{} {fake}
", p.host),
        n => format!("[{}]:{n} {fake}
", p.host),
    };
    std::fs::write(&kh, line).expect("known_hosts 쓰기");

    // 확인기를 달아 무엇이 올라오는지 본다(거부한다 — 진짜로 바꾸지 않는다).
    #[derive(Default)]
    struct Spy(Arc<Mutex<Option<(bool, String)>>>);
    impl nabi_ssh::HostKeyVerify for Spy {
        fn verify(&self, info: nabi_ssh::HostKeyInfo) -> tokio::sync::oneshot::Receiver<bool> {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let old = info.changed.as_ref().map(|c| c.old_fingerprint.clone()).unwrap_or_default();
            *self.0.lock().unwrap() = Some((info.changed.is_some(), old));
            let _ = tx.send(false); // 거부 — 이 시험은 알리는지만 본다.
            rx
        }
    }
    let seen = Arc::new(Mutex::new(None));
    let spy: nabi_ssh::HostKeyVerifier = Arc::new(Spy(seen.clone()));
    let r = connect_sftp(&p, kh.clone(), Some(spy)).await;

    assert!(r.is_err(), "거부했는데 연결이 됐다");
    let got = seen.lock().unwrap().clone();
    let Some((changed, old)) = got else {
        panic!("확인기가 불리지 않았다 — 바뀐 키를 조용히 거부하고 있다");
    };
    assert!(changed, "바뀐 키인데 '처음 보는 서버'로 올라왔다");
    assert!(old.starts_with("SHA256:"), "옛 지문을 못 읽었다: {old:?}");
    let _ = std::fs::remove_file(&kh);
}

/// 실 서버에서 **폴더째 서버 안 복사**(재귀) 검증.
///
/// 파일 하나 복사와 다른 점은 `mkdir`과 목록 읽기가 섞인다는 것이다 — 서버가 이미 있는
/// 폴더에 `mkdir`을 어떻게 답하는지, 목록에 `.`/`..`을 넣는지가 판마다 다르다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_copies_a_whole_folder() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let (src, dst) = ("nabi_realtest_tree", "nabi_realtest_tree_copy");
    // 하위 폴더까지 만들어 재귀가 실제로 도는지 본다(한 겹이면 재귀인지 알 수 없다).
    let _ = fs.mkdir(src).await;
    let _ = fs.mkdir(&format!("{src}/sub")).await;
    let a: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
    fs.write_file(&format!("{src}/a.bin"), &a).await.expect("write a");
    fs.write_file(&format!("{src}/sub/b.txt"), b"hello nabi").await.expect("write b");

    let mut last = 0u64;
    let mut cb = |n: u64| last = n;
    let mut prog = crate::DirProgress { done: 0, skipped: 0, cb: &mut cb };
    fs.copy_dir_remote(src, dst, &mut prog).await.expect("copy dir");

    let got_a = fs.read_file(&format!("{dst}/a.bin")).await.expect("read a");
    assert_eq!(got_a, a, "하위 파일 내용이 달라졌다");
    let got_b = fs.read_file(&format!("{dst}/sub/b.txt")).await.expect("read b");
    assert_eq!(got_b, b"hello nabi", "하위 폴더 파일이 안 왔다");
    assert!(last > 0, "진행률이 한 번도 오지 않았다");

    // **자기 안으로 복사하는 것은 거절해야 한다** — 안 그러면 디스크가 찰 때까지 돈다.
    let inner = format!("{src}/inner");
    let mut noop = |_: u64| {};
    let mut p2 = crate::DirProgress { done: 0, skipped: 0, cb: &mut noop };
    let err = fs.copy_dir_remote(src, &inner, &mut p2).await.expect_err("자기 안으로 복사가 됐다");
    assert_eq!(err, "sftp.copy.intoself");

    for f in [format!("{dst}/sub/b.txt"), format!("{dst}/a.bin"), format!("{src}/sub/b.txt"), format!("{src}/a.bin")] {
        let _ = fs.remove(&f).await;
    }
    for d in [format!("{dst}/sub"), dst.to_string(), format!("{src}/sub"), src.to_string()] {
        let _ = fs.remove(&d).await; // remove가 폴더면 rmdir로 넘어간다(raw.rs).
    }
}

/// **공개키 설치가 실제로 되는지** 실서버에서 본다(ssh-copy-id).
///
/// 이 기능은 남의 서버의 `~/.ssh/authorized_keys`를 고친다. 인프로세스로는 흉내 낼 수
/// 없고, 흉내 내 봐야 우리가 기대한 대로만 답한다. 실제로 확인해야 할 것 셋:
/// 파일이 **덧붙여지는가**, 권한이 맞는가, 두 번 넣어도 **한 줄인가**.
///
/// 이 머신의 sshd는 Windows OpenSSH라 기본 셸이 `cmd.exe`일 수 있다. 그러면 이 명령은
/// POSIX 셸을 못 찾아 실패한다 — **조용히 통과시키지 않고 건너뛴다고 말한다.**
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_installs_a_public_key() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    // `cmd.exe`는 "The syntax of the command is incorrect."를 뱉는다 — 비어 있지 않다고
    // POSIX로 착각하면 안 된다(배치 O의 상한 시험에서 겪은 것과 같은 함정).
    // 진짜 uname 답은 한 낱말이다(Linux · Darwin · FreeBSD · MINGW64_NT-10.0 …).
    let (uname, _) = fs.exec_remote("uname -s", 4096).await.unwrap_or_default();
    let u = uname.trim();
    let posix = !u.is_empty() && u.split_whitespace().count() == 1 && u.len() <= 24;
    if !posix {
        eprintln!("POSIX 셸이 아니다(uname={u:?}) — 키 설치 검증을 건너뛴다(조용히 통과시키지 않는다)");
        return;
    }

    // 진짜 authorized_keys를 건드리지 않도록, 시험 전에 백업하고 끝나면 되돌린다.
    let (before, _) = fs.exec_remote(nabi_ssh::copyid::read_command(), 1 << 20).await.expect("read");
    let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb7GQ2p7DbFPuhVpzOSVQXHDzHfF1lVMSCUmJ8UN0Rp nabi-copyid-test";
    assert!(!nabi_ssh::copyid::already_present(&before, key), "시험용 키가 이미 있다");

    let (out, code) = fs.exec_remote(&nabi_ssh::copyid::install_command(key), 1 << 16).await.expect("install");
    assert!(out.contains(nabi_ssh::copyid::OK_MARK), "설치가 끝났다는 표시가 없다: {out:?}");
    assert_eq!(code, Some(0), "설치 명령이 실패했다: {out:?}");

    // 1) 실제로 들어갔나.
    let (after, _) = fs.exec_remote(nabi_ssh::copyid::read_command(), 1 << 20).await.expect("read2");
    assert!(nabi_ssh::copyid::already_present(&after, key), "넣었는데 없다");
    // 2) **덧붙였나** — 앞서 있던 줄이 살아 있어야 한다.
    for line in before.lines().filter(|l| !l.trim().is_empty()) {
        assert!(after.contains(line), "덮어썼다! 사라진 줄: {line}");
    }
    // 3) 권한이 맞나(sshd가 이걸 안 맞추면 거부한다).
    let (perm, _) = fs.exec_remote("stat -c %a ~/.ssh/authorized_keys", 256).await.expect("stat");
    let perm = perm.trim();
    if !perm.is_empty() {
        assert_eq!(perm, "600", "authorized_keys 권한이 600이 아니다");
    }

    // 되돌리기: 시험용 키 줄만 지운다(앞서 있던 것은 그대로 둔다).
    let _ = fs
        .exec_remote("grep -v nabi-copyid-test ~/.ssh/authorized_keys > ~/.ssh/ak.tmp && mv ~/.ssh/ak.tmp ~/.ssh/authorized_keys", 4096)
        .await;
    let (restored, _) = fs.exec_remote(nabi_ssh::copyid::read_command(), 1 << 20).await.expect("read3");
    assert!(!nabi_ssh::copyid::already_present(&restored, key), "시험용 키를 못 지웠다");
}

/// 실 서버에서 **명령 한 줄 실행** — 인프로세스 서버는 exec 를 구현하지 않아 여기서만 볼 수 있다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_runs_a_command() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let (out, code) = fs.exec_remote("echo nabi-exec-ok", 64 * 1024).await.expect("exec");
    assert!(out.contains("nabi-exec-ok"), "출력이 안 왔다: {out:?}");
    assert_eq!(code, Some(0), "성공한 명령의 종료 코드가 0이 아니다");
}

/// **표준오류도 함께 모아야 한다** — 실패했을 때 할 말은 대개 그쪽에 있다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_exec_keeps_stderr_and_exit_code() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    // 없는 파일을 보면 셸이 표준오류로 말하고 0이 아닌 코드로 끝난다.
    let (out, code) = fs.exec_remote("ls /nabi-no-such-path-xyz", 64 * 1024).await.expect("exec");
    assert!(!out.trim().is_empty(), "표준오류를 버렸다 — 화면이 비어 보인다");
    assert_ne!(code, Some(0), "실패한 명령이 0으로 끝났다: {out:?}");
}

/// **많이 뱉는 명령에 우리가 죽지 않아야 한다.** 상한에서 끊고 끊겼다고 말한다.
///
/// 많이 뱉게 하는 방법이 셸마다 다르다. 이 시험은 POSIX 셸에서만 뜻이 있으므로,
/// 아니면 **건너뛴다고 말하고** 넘어간다 — 조용히 통과시키면 검증한 척이 된다
/// (이 머신의 서버는 Windows OpenSSH라 기본 셸이 cmd.exe다).
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_exec_stops_at_the_cap() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.expect("connect");
    let (probe, _) = fs.exec_remote("uname -s", 4096).await.expect("probe");
    if !probe.to_lowercase().contains("linux") && !probe.to_lowercase().contains("darwin") {
        eprintln!("POSIX 셸이 아니다({}) — 상한 시험 건너뜀", probe.trim());
        return;
    }
    let (out, _) = fs.exec_remote("yes nabi | head -c 300000", 4096).await.expect("exec");
    assert!(out.len() < 20_000, "상한을 넘겨 모았다: {}바이트", out.len());
    assert!(out.contains("exec.truncated"), "잘렸는데 말하지 않았다");
}
