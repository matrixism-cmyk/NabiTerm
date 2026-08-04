//! 실 SSH/SFTP 서버 대상 통합테스트(#[ignore] — 기본 게이트 제외, 실서버 동작 검증용).
//!
//! 실행: 서버 정보를 환경변수로 주고 `cargo test -p nabi-sftp -- --ignored`.
//!   NABI_RT_USER(필수) NABI_RT_KEY(개인키 경로, 필수) NABI_RT_HOST(기본 127.0.0.1) NABI_RT_PORT(기본 22)
//! USER/KEY 미설정 시 조용히 통과(미구성 환경 안전). 상대 경로는 서버가 홈 기준으로 해석한다.

use crate::connect_sftp;
use nabi_fs::RemoteFs;
use nabi_proto::SshParams;

fn params() -> Option<SshParams> {
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

/// 실 OpenSSH 서버에서 업로드 원자적 교체(rename-over-existing) 검증.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_upload_atomic_overwrites() {
    let Some(p) = params() else { return };
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
    let mut fs = connect_sftp(&p, crate::sftp_server::test_known_hosts(), None).await.expect("connect");
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
