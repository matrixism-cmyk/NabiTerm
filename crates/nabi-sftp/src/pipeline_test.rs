//! 파이프라인 전송·확장(B1/B2/B3) 검증 — 인프로세스 서버가 OpenSSH 확장을 광고하는 상태.
//!
//! 서버가 알려주는 청크 한도가 8KB, 파이프라인 깊이가 64라 한 파도는 512KB다.
//! 그보다 큰 파일을 써서 **파도가 여러 번 도는 경로**를 실제로 지나가게 한다.

use crate::pipeline::depth;
use crate::sftp_boot::connect_fs;
use crate::sftp_serverext::{TEST_FRAGMENT, TEST_FREE_BLOCKS, TEST_WRITE_LEN};
use nabi_fs::RemoteFs;

/// 한 파도 크기(서버가 알려준 한도 × 깊이).
fn wave() -> usize {
    depth() * TEST_WRITE_LEN as usize
}

/// 위치마다 값이 다른 패턴 — 순서가 어긋나거나 구멍이 생기면 바로 드러난다.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("nabi-pipe-{tag}-{}.bin", std::process::id()))
}

#[tokio::test]
async fn multi_wave_upload_download_roundtrip() {
    // 파도 3개 분량 + 어중간한 꼬리 — 마지막 파도가 꽉 차지 않는 경계도 함께 본다.
    let data = pattern(wave() * 3 + 1234);
    let src = tmp_path("src");
    std::fs::write(&src, &data).unwrap();
    let mut fs = connect_fs().await;
    let mut seen = 0u64;
    fs.upload(src.to_str().unwrap(), "/big.bin", |b| seen = b).await.expect("upload");
    assert_eq!(seen, data.len() as u64, "진행률이 전체 크기까지 보고돼야");
    assert_eq!(fs.read_file("/big.bin").await.expect("read"), data, "업로드 내용 불일치");

    let dst = tmp_path("dst");
    fs.download("/big.bin", dst.to_str().unwrap(), 0, |_| {}).await.expect("download");
    assert_eq!(std::fs::read(&dst).unwrap(), data, "다운로드 내용 불일치");
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&dst);
}

#[tokio::test]
async fn resume_repairs_holey_part_file() {
    // 파이프라인 업로드가 중간에 끊기면 임시 파일 꼬리에 구멍(0바이트)이 남을 수 있다.
    // 이어올리기가 그 구간을 다시 보내지 않으면 파일이 조용히 망가진다.
    let data = pattern(wave() * 2);
    let src = tmp_path("holesrc");
    std::fs::write(&src, &data).unwrap();
    let mut fs = connect_fs().await;
    // 앞 한 파도는 제대로, 그 뒤 절반은 0으로 채워진 "구멍 난" 임시 파일을 만들어 둔다.
    let mut broken = data[..wave()].to_vec();
    broken.extend(std::iter::repeat_n(0u8, wave() / 2));
    fs.write_file("/hole.bin.filepart", &broken).await.expect("part 준비");

    fs.upload(src.to_str().unwrap(), "/hole.bin", |_| {}).await.expect("upload");
    assert_eq!(fs.read_file("/hole.bin").await.expect("read"), data, "구멍이 복구돼야 한다");
}

#[tokio::test]
async fn rename_over_existing_target_succeeds() {
    // SFTP v3 rename은 대상이 있으면 실패한다. posix-rename 확장이 있으면 원자적으로 덮어쓴다.
    let mut fs = connect_fs().await;
    fs.write_file("/src.txt", b"NEW").await.expect("write src");
    fs.write_file("/dst.txt", b"OLD").await.expect("write dst");
    fs.rename("/src.txt", "/dst.txt").await.expect("원자적 교체");
    assert_eq!(fs.read_file("/dst.txt").await.expect("read"), b"NEW");
    assert!(fs.read_file("/src.txt").await.is_err(), "원본은 사라져야");
}

#[tokio::test]
async fn upload_refuses_when_remote_is_full() {
    // statvfs로 미리 확인해, 전송 도중 공간 부족으로 죽는 대신 시작 전에 알려 준다.
    let free = TEST_FREE_BLOCKS * TEST_FRAGMENT;
    let src = tmp_path("toobig");
    std::fs::write(&src, vec![7u8; free as usize + 1]).unwrap();
    let mut fs = connect_fs().await;
    let err = fs.upload(src.to_str().unwrap(), "/toobig.bin", |_| {}).await.expect_err("거부 기대");
    assert!(err.contains("여유 공간"), "공간 부족을 알려야: {err}");
    let _ = std::fs::remove_file(&src);
}

#[tokio::test]
async fn pipeline_does_not_waste_read_requests() {
    // 서버가 한도(8KB)보다 짧게(3KB) 주는데도 그 길이에 맞추지 못하면, 파도마다 앞 조각
    // 하나만 쓸모 있어 요청이 깊이배로 늘어난다(실제로 겪은 회귀 — 처리량 1/25).
    use crate::sftp_serverext::SHORT_READ_CAP_FOR_TEST as CAP;
    let chunks = 400;
    let data = pattern(CAP * chunks);
    let mut fs = connect_fs().await;
    fs.write_file("/eff.bin", &data).await.expect("write");
    let before = fs.raw.read_requests();
    assert_eq!(fs.read_file("/eff.bin").await.expect("read"), data);
    let used = fs.raw.read_requests() - before;
    assert!(used < chunks as u64 * 2, "읽기 요청 {used}개는 조각 {chunks}개에 비해 과하다");
}

#[tokio::test]
async fn server_limits_shrink_the_chunk() {
    // 청크를 추측하지 않고 서버가 알려준 한도를 쓴다(limits@openssh.com).
    let fs = connect_fs().await;
    assert_eq!(fs.raw.write_chunk(), TEST_WRITE_LEN as usize);
    assert!(fs.raw.feat().posix_rename && fs.raw.feat().fsync && fs.raw.feat().statvfs);
    assert_eq!(fs.free_space("/").await, Some(TEST_FREE_BLOCKS * TEST_FRAGMENT));
}
