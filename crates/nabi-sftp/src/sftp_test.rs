//! SftpFs(SFTP 백엔드) 런타임 검증. 인프로세스 SSH+SFTP 서버 하네스는 sftp_server.rs.

use crate::sftp_boot::connect_fs;
use nabi_fs::RemoteFs;

#[tokio::test]
async fn sftp_list_dir_roundtrip() {
    let mut fs = connect_fs().await;
    let names: Vec<String> = fs
        .list_dir("/")
        .await
        .expect("list_dir")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(names.contains(&"foo.txt".to_string()), "got {names:?}");
    assert!(names.contains(&"bar".to_string()), "got {names:?}");
}

#[tokio::test]
async fn sftp_write_then_read_roundtrip() {
    let mut fs = connect_fs().await;
    fs.write_file("/upload.txt", b"hello sftp").await.expect("write");
    let data = fs.read_file("/upload.txt").await.expect("read");
    assert_eq!(data, b"hello sftp", "round-trip content mismatch");
}

#[tokio::test]
async fn sftp_remove_rename_mkdir() {
    let mut fs = connect_fs().await;
    fs.write_file("/gone.txt", b"x").await.expect("write");
    fs.remove("/gone.txt").await.expect("remove");
    assert!(fs.read_file("/gone.txt").await.is_err(), "removed");
    fs.write_file("/a.txt", b"data").await.expect("write");
    fs.rename("/a.txt", "/b.txt").await.expect("rename");
    assert_eq!(fs.read_file("/b.txt").await.expect("read new"), b"data");
    assert!(fs.read_file("/a.txt").await.is_err(), "old gone");
    fs.mkdir("/newdir").await.expect("mkdir");
}

#[test]
fn throttle_paces_to_limit() {
    use crate::fs::throttle_delay;
    use std::time::Duration;
    assert!(throttle_delay(1000, Duration::from_secs(1), 0).is_none()); // 무제한.
    let d = throttle_delay(1_000_000, Duration::from_millis(100), 1_000_000).unwrap();
    assert!(d >= Duration::from_millis(800) && d <= Duration::from_millis(950), "{d:?}");
    assert!(throttle_delay(1000, Duration::from_secs(5), 1_000_000).is_none()); // 이미 느림.
}

#[tokio::test]
async fn sftp_download_cancels() {
    // 취소 플래그가 켜져 있으면 download가 첫 청크 후 중단(Err)하고, 최종 파일은 만들지 않는다.
    let mut fs = connect_fs().await;
    fs.write_file("/cbig.bin", b"some streamed content").await.unwrap();
    fs.cancel_flag()
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("nabi-cancel-{}.bin", std::process::id()));
    let r = fs.download("/cbig.bin", tmp.to_str().unwrap(), 0, |_| {}).await;
    assert!(r.is_err(), "취소 시 Err 기대: {r:?}");
    assert!(!tmp.exists(), "중단 시 최종 파일은 없어야(부분은 .filepart)");
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(format!("{}.filepart", tmp.to_str().unwrap()));
}

#[tokio::test]
async fn sftp_listing_has_mtime() {
    let mut fs = connect_fs().await;
    let e = fs
        .list_dir("/")
        .await
        .expect("list")
        .into_iter()
        .find(|e| e.name == "foo.txt")
        .expect("foo.txt");
    assert_eq!(e.mtime, 1_700_000_000);
}

#[tokio::test]
async fn sftp_touch_creates_empty() {
    // touch = 빈 파일 쓰기. 쓴 뒤 읽으면 0바이트.
    let mut fs = connect_fs().await;
    fs.write_file("/touched.txt", b"").await.expect("touch");
    assert!(fs.read_file("/touched.txt").await.expect("read").is_empty());
}

#[tokio::test]
async fn sftp_search_finds_matches() {
    let mut fs = connect_fs().await;
    let hits = fs.search("/", "foo", 50).await;
    assert!(hits.iter().any(|p| p == "/foo.txt"), "got {hits:?}");
    assert!(!hits.iter().any(|p| p == "/bar"), "bar는 매치 아님: {hits:?}");
}

#[tokio::test]
async fn sftp_chmod_roundtrip() {
    let mut fs = connect_fs().await;
    fs.chmod("/foo.txt", 0o600).await.expect("chmod");
    let e = fs
        .list_dir("/")
        .await
        .expect("list")
        .into_iter()
        .find(|e| e.name == "foo.txt")
        .expect("foo.txt");
    assert_eq!(e.mode & 0o777, 0o600);
}

#[tokio::test]
async fn sftp_upload_dir_recurses() {
    let mut fs = connect_fs().await;
    let base = std::env::temp_dir().join(format!("nabi-sftp-uldir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("sub")).unwrap();
    std::fs::write(base.join("a.txt"), b"AA").unwrap();
    std::fs::write(base.join("sub").join("b.txt"), b"BB").unwrap();
    fs.upload_dir(&base, "/up").await.expect("upload_dir");
    assert_eq!(fs.read_file("/up/a.txt").await.expect("a"), b"AA");
    assert_eq!(fs.read_file("/up/sub/b.txt").await.expect("b"), b"BB");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn sftp_download_dir_recurses() {
    let mut fs = connect_fs().await;
    let dst = std::env::temp_dir().join(format!("nabi-sftp-dldir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    fs.download_dir("/", &dst).await.expect("download_dir");
    assert_eq!(std::fs::read(dst.join("foo.txt")).expect("foo"), b"foo");
    assert_eq!(std::fs::read(dst.join("bar")).expect("bar"), b"bar");
    let _ = std::fs::remove_dir_all(&dst);
}

#[tokio::test]
async fn sftp_download_streamed_reports_progress() {
    let mut fs = connect_fs().await;
    fs.write_file("/big.bin", b"streamed content").await.expect("write");
    let tmp = std::env::temp_dir().join("nabi_sftp_dl_test.bin");
    let mut last = 0u64;
    fs.download("/big.bin", tmp.to_str().unwrap(), 0, |b| last = b)
        .await
        .expect("download");
    assert_eq!(std::fs::read(&tmp).expect("read tmp"), b"streamed content");
    assert!(last > 0, "progress reported");
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn sftp_download_resumes_partial() {
    let mut fs = connect_fs().await;
    // /foo.txt = "foo". 부분 .filepart("fo")가 있으면 오프셋 2부터 이어받아 "foo" 완성 후 rename.
    let tmp = std::env::temp_dir().join(format!("nabi-resume-{}.txt", std::process::id()));
    let part = format!("{}.filepart", tmp.to_str().unwrap());
    std::fs::write(&part, b"fo").unwrap();
    fs.download("/foo.txt", tmp.to_str().unwrap(), 2, |_| {})
        .await
        .expect("resume");
    assert_eq!(std::fs::read(&tmp).expect("read"), b"foo");
    assert!(!std::path::Path::new(&part).exists(), "완료 후 .filepart는 rename되어 사라져야");
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn sftp_download_atomic_rename() {
    // 새 다운로드는 .filepart에 쓴 뒤 원자적으로 최종 파일로 rename하고 임시 파일은 남기지 않는다.
    let mut fs = connect_fs().await;
    let tmp = std::env::temp_dir().join(format!("nabi-atomic-{}.txt", std::process::id()));
    let part = format!("{}.filepart", tmp.to_str().unwrap());
    fs.download("/foo.txt", tmp.to_str().unwrap(), 0, |_| {})
        .await
        .expect("download");
    assert_eq!(std::fs::read(&tmp).expect("read"), b"foo");
    assert!(!std::path::Path::new(&part).exists(), ".filepart 잔여물 없어야");
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn sftp_chmod_recursive_applies_to_children() {
    // 재귀 chmod: 루트에 적용하면 하위 foo.txt·bar 권한도 바뀐다.
    let mut fs = connect_fs().await;
    fs.chmod_recursive("/", 0o640).await.expect("chmod -R");
    for e in fs.list_dir("/").await.expect("list") {
        assert_eq!(e.mode & 0o777, 0o640, "{} 권한", e.name);
    }
}

#[tokio::test]
async fn sftp_dir_stats_counts_and_sums() {
    // 루트에 foo.txt("foo"=3) + bar("bar"=3), 둘 다 파일. (파일2, 폴더0, 6바이트).
    let mut fs = connect_fs().await;
    assert_eq!(fs.dir_stats("/").await, (2, 0, 6));
}

#[tokio::test]
async fn sftp_upload_atomic_replaces_existing() {
    // 업로드는 .filepart에 올린 뒤 기존 대상을 원자적으로 교체하고 임시 파일을 남기지 않는다.
    let mut fs = connect_fs().await;
    let tmp = std::env::temp_dir().join(format!("nabi-upatom-{}.txt", std::process::id()));
    std::fs::write(&tmp, b"NEWDATA").unwrap();
    fs.upload(tmp.to_str().unwrap(), "/foo.txt", |_| {})
        .await
        .expect("upload");
    assert_eq!(fs.read_file("/foo.txt").await.expect("read"), b"NEWDATA");
    assert!(fs.read_file("/foo.txt.filepart").await.is_err(), ".filepart 잔여 없어야");
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn sftp_upload_streamed_reports_progress() {
    let mut fs = connect_fs().await;
    let tmp = std::env::temp_dir().join("nabi_sftp_up_test.bin");
    std::fs::write(&tmp, b"upload streamed").expect("write tmp");
    let mut last = 0u64;
    fs.upload(tmp.to_str().unwrap(), "/up.bin", |b| last = b)
        .await
        .expect("upload");
    assert_eq!(fs.read_file("/up.bin").await.expect("read back"), b"upload streamed");
    assert!(last > 0, "progress reported");
    let _ = std::fs::remove_file(&tmp);
}
