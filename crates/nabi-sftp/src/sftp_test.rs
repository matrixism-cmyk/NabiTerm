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
    let tmp = crate::sftp_boot::tmp_path("cancel.bin");
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
    let base = crate::sftp_boot::tmp_path("sftp-uldir");
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
    let dst = crate::sftp_boot::tmp_path("sftp-dldir");
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
    let tmp = crate::sftp_boot::tmp_path("resume.txt");
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
    let tmp = crate::sftp_boot::tmp_path("atomic.txt");
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
    let tmp = crate::sftp_boot::tmp_path("upatom.txt");
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

/// 확장이 하나도 없는 옛 서버(순정 SFTP v3)에서도 전송이 온전히 도는가.
///
/// 사용자 서버가 OpenSSH 4.3이라 이 갈래가 실제로 쓰인다. 그런데 우리 테스트 서버는
/// 늘 확장을 광고해서, **확장이 없을 때의 기본값 경로는 어디에서도 검증된 적이 없었다.**
/// 실서버로는 다운로드만 확인했다(남의 운영 서버에 쓸 수 없어서). 여기서 업로드까지 본다.
#[tokio::test]
async fn bare_server_upload_download_roundtrip() {
    let mut fs = crate::sftp_boot::connect_bare_fs().await;
    // 정말 확장 없는 서버에 붙었는지 먼저 못 박는다 — 아니면 이 테스트는 일반 서버를
    // 한 번 더 도는 것일 뿐이고, 검증하려던 갈래는 그대로 미검증으로 남는다.
    let feat = fs.raw.feat();
    assert!(!feat.posix_rename && !feat.statvfs && !feat.fsync, "확장 없는 서버여야 한다: {feat:?}");
    assert!(feat.read_len.is_none() && feat.write_len.is_none(), "limits도 없어야 한다");
    // 여러 파도를 돌도록 넉넉히(확장이 없으면 청크 크기는 기본값으로 정해진다).
    let data = vec![b'q'; 300 * 1024];
    let local = crate::sftp_boot::tmp_path("bare.bin");
    std::fs::write(&local, &data).unwrap();
    let lp = local.to_string_lossy().into_owned();

    // posix-rename 없이도 원자적 교체(.filepart → 대상)가 끝까지 가야 한다.
    let mut seen = 0u64;
    fs.upload(&lp, "/bare.bin", |b| seen = b).await.expect("업로드");
    assert_eq!(seen, data.len() as u64, "진행률 합계가 파일 크기와 같아야 한다");
    assert_eq!(fs.read_file("/bare.bin").await.expect("읽기"), data, "내용이 같아야 한다");
    assert!(fs.read_file("/bare.bin.filepart").await.is_err(), ".filepart 잔여 없어야");

    // 되받아서 크기·내용 확인(다운로드 파이프라인의 확장 없는 경로).
    let back = crate::sftp_boot::tmp_path("bare-back.bin");
    fs.download("/bare.bin", back.to_str().unwrap(), 0, |_| {}).await.expect("다운로드");
    assert_eq!(std::fs::read(&back).unwrap(), data);
    let _ = std::fs::remove_file(&local);
    let _ = std::fs::remove_file(&back);
}

/// statvfs가 없으면 여유 공간 확인을 **건너뛰고** 전송이 진행돼야 한다.
/// (확인 못 했다고 막아 버리면 옛 서버에는 아무것도 못 올린다.)
#[tokio::test]
async fn bare_server_skips_free_space_check() {
    let mut fs = crate::sftp_boot::connect_bare_fs().await;
    assert!(!fs.raw.feat().statvfs, "statvfs 없는 서버여야 의미가 있다");
    let local = crate::sftp_boot::tmp_path("bare-sp.bin");
    std::fs::write(&local, b"small").unwrap();
    fs.upload(local.to_str().unwrap(), "/sp.bin", |_| {}).await.expect("공간 확인 없이 업로드");
    assert_eq!(fs.read_file("/sp.bin").await.unwrap(), b"small");
    let _ = std::fs::remove_file(&local);
}

/// **서버 안에서 복사** — 내용이 그대로 옮겨져야 한다.
#[tokio::test]
async fn sftp_copies_within_the_server() {
    let mut fs = connect_fs().await;
    let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    fs.write_file("/big.bin", &body).await.expect("write");
    let mut seen = 0u64;
    let n = fs
        .copy_remote("/big.bin", "/big-copy.bin", &mut |p| seen = p)
        .await
        .expect("copy");
    assert_eq!(n as usize, body.len(), "복사한 바이트 수가 다르다");
    assert_eq!(seen, n, "진행률이 끝까지 오지 않았다");
    let got = fs.read_file("/big-copy.bin").await.expect("read back");
    assert_eq!(got, body, "내용이 달라졌다");
}

/// 원본이 없으면 **대상 파일을 만들지 않는다** — 빈 파일이 남으면 성공으로 오해한다.
#[tokio::test]
async fn a_missing_source_leaves_no_target() {
    let mut fs = connect_fs().await;
    let r = fs.copy_remote("/없는파일.bin", "/should-not-exist.bin", &mut |_| {}).await;
    assert!(r.is_err(), "없는 파일을 복사했다고 한다");
    assert!(
        fs.read_file("/should-not-exist.bin").await.is_err(),
        "실패했는데 대상 파일이 남았다"
    );
}

/// 빈 파일도 복사된다(0바이트가 오류로 취급되면 안 된다).
#[tokio::test]
async fn an_empty_file_copies_too() {
    let mut fs = connect_fs().await;
    fs.write_file("/empty.txt", b"").await.expect("write");
    let n = fs.copy_remote("/empty.txt", "/empty2.txt", &mut |_| {}).await.expect("copy");
    assert_eq!(n, 0);
    assert_eq!(fs.read_file("/empty2.txt").await.expect("read"), Vec::<u8>::new());
}

/// 재귀 찾기가 **글로브를 안다** — 찾기 창과 같은 규칙을 쓰는지 확인한다(배치 AD).
///
/// 예전에는 이 경로만 `name.contains(needle)` 이었다. 그래서 `*.txt` 를 치면 찾기 창은
/// `.txt` 파일들을 찾아 주는데 도구막대는 **언제나 아무것도 못 찾았다** — 이름에 별표가
/// 든 파일은 없으니까. 사용자에게는 둘 다 "서버에서 이름으로 찾기"였다.
#[tokio::test]
async fn recursive_search_understands_globs_like_the_find_window() {
    let mut fs = connect_fs().await;
    let hits = fs.search("/", "*.txt", 50).await;
    assert!(
        hits.iter().any(|p| p.ends_with("foo.txt")),
        "글로브가 통해야 한다 — 옛 규칙이면 여기서 빈 목록이 온다: {hits:?}"
    );
}

/// 평범한 낱말 찾기는 그대로 동작한다(글로브를 넣으면서 깨뜨리지 않았는지).
#[tokio::test]
async fn recursive_search_still_matches_a_plain_word() {
    let mut fs = connect_fs().await;
    let hits = fs.search("/", "foo", 50).await;
    assert!(hits.iter().any(|p| p.contains("foo")), "got {hits:?}");
}
