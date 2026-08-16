//! 실 서버 대상 파이프라인·확장 검증(B1/B2/B3). 기본 게이트 제외(#[ignore]).
//!
//! 실행: `NABI_RT_USER=… NABI_RT_PASS=… cargo test -p nabi-sftp -- --ignored`
//! 인프로세스 서버는 우리가 만든 것이라 "우리가 기대하는 대로" 답한다. 여기서는 진짜
//! OpenSSH sftp-server가 어떤 확장을 주고 어떤 한도를 말하는지 실제로 확인한다.

use crate::realserver_test::params;
use crate::{connect_sftp, SftpFs};
use nabi_fs::RemoteFs;

/// 위치마다 값이 다른 패턴 — 순서 어긋남·구멍이 바로 드러난다.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

async fn open_fs() -> Option<SftpFs> {
    let p = params()?;
    connect_sftp(&p, crate::sftp_boot::test_known_hosts(), None).await.ok()
}

/// 서버가 실제로 알려주는 확장·한도를 기록한다(추측이 아니라 관측).
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_reports_extensions() {
    let Some(fs) = open_fs().await else { return };
    let f = fs.raw.feat();
    println!(
        "확장: posix_rename={} fsync={} statvfs={} · 청크 읽기 {} 쓰기 {}",
        f.posix_rename,
        f.fsync,
        f.statvfs,
        fs.raw.read_chunk(),
        fs.raw.write_chunk()
    );
    // OpenSSH 8.5+ 라면 limits로 청크를 알려준다. 아니어도 보수적 기본값으로 동작해야 한다.
    assert!(fs.raw.read_chunk() >= 4096 && fs.raw.write_chunk() >= 4096);
}

/// 파도를 여러 번 도는 크기의 왕복 — 파이프라인이 내용을 어긋내지 않는지 본다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_multi_wave_roundtrip() {
    let Some(mut fs) = open_fs().await else { return };
    let remote = "nabi_realtest_wave.bin";
    let mb: usize = std::env::var("NABI_RT_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    let data = pattern(mb * 1024 * 1024 + 777); // 기본 6MB + 어중간한 꼬리.
    let src = std::env::temp_dir().join(format!("nabi-rt-wave-{}.bin", std::process::id()));
    std::fs::write(&src, &data).unwrap();

    let t0 = std::time::Instant::now();
    fs.upload(src.to_str().unwrap(), remote, |_| {}).await.expect("upload");
    let up = t0.elapsed();
    let dst = std::env::temp_dir().join(format!("nabi-rt-wave-dl-{}.bin", std::process::id()));
    let t1 = std::time::Instant::now();
    fs.download(remote, dst.to_str().unwrap(), 0, |_| {}).await.expect("download");
    let down = t1.elapsed();
    let mb = data.len() as f64 / 1_048_576.0;
    println!("업로드 {:.1}MB/s · 다운로드 {:.1}MB/s", mb / up.as_secs_f64(), mb / down.as_secs_f64());

    assert_eq!(std::fs::read(&dst).unwrap(), data, "왕복 내용 불일치");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

/// 해시 검증을 켠 왕복(T2-1) — 원격에 해시 명령이 있으면 대조, 없으면(Windows OpenSSH)
/// 조용히 건너뛰고 성공해야 한다. 두 경로 모두 "전송이 실패하면 안 된다"가 검증 대상.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_roundtrip_with_hash_verify() {
    let Some(mut fs) = open_fs().await else { return };
    crate::hashcheck::SFTP_VERIFY_HASH.store(true, std::sync::atomic::Ordering::Relaxed);
    let remote = "nabi_realtest_hash.bin";
    let data = pattern(2 * 1024 * 1024 + 13);
    let src = std::env::temp_dir().join(format!("nabi-rt-hash-{}.bin", std::process::id()));
    std::fs::write(&src, &data).unwrap();
    fs.upload(src.to_str().unwrap(), remote, |_| {}).await.expect("해시 검증 업로드");
    let dst = std::env::temp_dir().join(format!("nabi-rt-hash-dl-{}.bin", std::process::id()));
    fs.download(remote, dst.to_str().unwrap(), 0, |_| {}).await.expect("해시 검증 다운로드");
    assert_eq!(std::fs::read(&dst).unwrap(), data);
    // 원격 해시 명령 가용 여부를 기록해 둔다(관측 — 서버마다 다르다).
    let rh = crate::hashcheck::remote_sha256(&fs.handle, remote).await;
    println!("원격 sha256 명령: {}", if rh.is_some() { "가용(대조 수행됨)" } else { "없음(크기 비교 폴백)" });
    crate::hashcheck::SFTP_VERIFY_HASH.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

/// 구멍 난 임시 파일에서 이어올리기 — 조용한 데이터 손상을 막는지 본다(B3).
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_resume_repairs_hole() {
    let Some(mut fs) = open_fs().await else { return };
    let remote = "nabi_realtest_hole.bin";
    let chunk = fs.raw.write_chunk();
    let wave = crate::pipeline::depth() * chunk;
    let data = pattern(wave * 2);
    let src = std::env::temp_dir().join(format!("nabi-rt-hole-{}.bin", std::process::id()));
    std::fs::write(&src, &data).unwrap();
    // 앞 한 파도는 정상, 그 뒤 절반은 0 — 중단된 파이프라인 업로드가 남길 수 있는 모습.
    let mut broken = data[..wave].to_vec();
    broken.extend(std::iter::repeat_n(0u8, wave / 2));
    fs.write_file(&format!("{remote}.filepart"), &broken).await.expect("part 준비");

    fs.upload(src.to_str().unwrap(), remote, |_| {}).await.expect("upload");
    assert_eq!(fs.read_file(remote).await.expect("read"), data, "구멍이 복구돼야 한다");
    let _ = fs.remove(remote).await;
    let _ = std::fs::remove_file(&src);
}

/// 관측 기록(2026-08-05, OpenSSH_for_Windows_9.5p2): 열린 핸들의 `fsetstat(size)`를 거부한다.
///
/// 이어올리기를 "자르고 이어 쓰기"로 구현하면 이 서버에서 통째로 실패한다. 그래서
/// [`crate::pipeline::resume_offset`]로 되감아 다시 쓰는 방식을 쓴다. 이 테스트는 그
/// 전제(자르기는 못 믿는다 / 되감아 쓰기는 통한다)가 계속 성립하는지 확인한다.
#[tokio::test]
#[ignore = "실 서버 필요(NABI_RT_USER/KEY 환경변수)"]
async fn realserver_resume_does_not_need_truncate() {
    let Some(mut fs) = open_fs().await else { return };
    let remote = "nabi_realtest_trunc.bin";
    fs.write_file(remote, &pattern(100_000)).await.expect("seed");
    let flags = russh_sftp::protocol::OpenFlags::WRITE | russh_sftp::protocol::OpenFlags::CREATE;
    let h = fs.raw.open(remote, flags).await.expect("open WRITE|CREATE");
    let truncate_ok = fs.raw.truncate(&h, 40_000).await.is_ok();
    // 관측: Windows OpenSSH 9.5p2는 실제로는 잘라 놓고 상태만 "Failure"로 준다.
    // 상태를 믿고 실패로 처리하면 이어올리기가 통째로 막힌다 — 그래서 자르기에 의존하지 않는다.
    println!("이 서버의 fsetstat(size) 지원: {truncate_ok}");
    // 자르기 지원 여부와 무관하게, 오프셋 쓰기는 되어야 한다(되감아 쓰기의 전제).
    fs.raw.write_at(&h, 40_000, vec![9u8; 1000]).await.expect("offset write");
    let _ = fs.raw.close(&h).await;
    let size = fs.raw.stat(remote).await.expect("stat").size.expect("size");
    println!("자르기 후 크기: {size}");
    // 자르기가 통했으면 41,000, 아니면 원래 100,000 — 어느 쪽이든 오프셋 쓰기는 반영돼야 한다.
    assert!(size >= 41_000, "오프셋 쓰기가 반영되지 않았다: {size}");
    let _ = fs.remove(remote).await;
}