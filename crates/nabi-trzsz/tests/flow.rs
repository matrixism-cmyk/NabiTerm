//! 전체 흐름 시험 — **가짜 원격**을 세워 트리거부터 `#EXIT`까지 돌린다.
//!
//! 단위 시험은 프레임 하나하나를 보지만, 실제로 깨지는 곳은 늘 순서와 되돌아오는 값이다.
//! 여기서는 원격 역할을 우리가 맡아 프로토콜을 반대편에서 말해 본다.

mod harness;

use harness::{drive, trigger, MemSource, MemStorage, Remote};
use nabi_trzsz::{Plan, Session, UploadItem};

#[test]
fn downloads_one_file() {
    let store = MemStorage::new();
    let files = store.shared();
    let mut remote = Remote::sender(vec![("report.txt".into(), b"hello trzsz".to_vec())]);
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(store)));
    let (names, err) = drive(s, first, &mut remote);

    assert_eq!(err, None);
    assert_eq!(names, vec!["report.txt".to_string()]);
    let got = files.borrow();
    assert_eq!(got[0].1, b"hello trzsz");
    assert!(got[0].2, "정상 종료면 파일을 살려 둬야 한다");
    assert!(remote.saw_exit, "원격은 #EXIT를 받아야 끝난다");
}

#[test]
fn downloads_several_files_including_an_empty_one() {
    let store = MemStorage::new();
    let files = store.shared();
    let big = vec![7u8; 5000]; // 여러 청크로 쪼개진다
    let mut remote = Remote::sender(vec![
        ("a.bin".into(), big.clone()),
        ("empty.txt".into(), Vec::new()),
        ("b.txt".into(), b"tail".to_vec()),
    ]);
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(store)));
    let (names, err) = drive(s, first, &mut remote);

    assert_eq!(err, None);
    assert_eq!(names, vec!["a.bin", "empty.txt", "b.txt"]);
    let got = files.borrow();
    assert_eq!(got[0].1, big);
    assert!(got[1].1.is_empty());
    assert_eq!(got[2].1, b"tail");
    assert!(got.iter().all(|f| f.2), "셋 다 살아 있어야 한다");
}

#[test]
fn a_corrupted_download_is_deleted_not_kept() {
    let store = MemStorage::new();
    let files = store.shared();
    let mut remote = Remote::sender(vec![("x.bin".into(), b"0123456789".to_vec())]);
    remote.corrupt_md5 = true;
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(store)));
    let (names, err) = drive(s, first, &mut remote);

    assert!(names.is_empty());
    assert!(err.unwrap().contains("checksum"), "무결성 실패를 알려야 한다");
    assert!(!files.borrow()[0].2, "반쪽 파일을 남기면 안 된다");
}

#[test]
fn a_dangerous_name_is_refused_before_anything_is_written() {
    let store = MemStorage::new();
    let mut remote = Remote::sender(vec![("../../etc/passwd".into(), b"x".to_vec())]);
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(store)));
    let (names, err) = drive(s, first, &mut remote);

    assert!(names.is_empty());
    assert!(err.unwrap().contains("traversal"));
}

#[test]
fn uploads_one_file() {
    let mut remote = Remote::receiver();
    let t = trigger('R');
    let item = UploadItem {
        name: "note.md".into(),
        size: 11,
        source: Box::new(MemSource::new(b"hello trzsz".to_vec())),
    };
    let (s, first) = Session::new(&t, Plan::Upload(vec![item]));
    let (names, err) = drive(s, first, &mut remote);

    assert_eq!(err, None);
    assert_eq!(names, vec!["note.md".to_string()]);
    assert_eq!(remote.received(), vec![("note.md".to_string(), b"hello trzsz".to_vec())]);
    assert!(remote.saw_exit);
}

#[test]
fn uploads_a_file_that_needs_many_chunks() {
    let data: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
    let mut remote = Remote::receiver();
    let t = trigger('R');
    let item = UploadItem {
        name: "big.bin".into(),
        size: data.len() as u64,
        source: Box::new(MemSource::new(data.clone())),
    };
    let (s, first) = Session::new(&t, Plan::Upload(vec![item]));
    let (_, err) = drive(s, first, &mut remote);

    assert_eq!(err, None);
    assert_eq!(remote.received()[0].1, data, "여러 청크로 나뉘어도 내용이 같아야 한다");
}

#[test]
fn rejecting_tells_the_remote_and_stops() {
    let mut remote = Remote::sender(vec![("x".into(), b"y".to_vec())]);
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Reject("사용자가 거절".into()));
    assert!(s.is_ended(), "거절은 그 자리에서 끝난다");
    let (names, err) = drive(s, first, &mut remote);
    assert!(names.is_empty());
    assert_eq!(err, None, "거절은 실패가 아니다");
    assert!(!remote.saw_confirm, "원격에 confirm:false를 알려야 한다");
}

#[test]
fn a_remote_failure_ends_the_session() {
    let mut remote = Remote::sender(vec![("x".into(), b"y".to_vec())]);
    remote.fail_after_cfg = true;
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(MemStorage::new())));
    let (_, err) = drive(s, first, &mut remote);
    assert!(err.unwrap().contains("disk full"), "원격이 준 사유를 그대로 보여야 한다");
}

#[test]
fn cancelling_mid_transfer_deletes_the_partial_file() {
    let store = MemStorage::new();
    let files = store.shared();
    let mut remote = Remote::sender(vec![("x.bin".into(), vec![1u8; 5000])]);
    let t = trigger('S');
    let (mut s, first) = Session::new(&t, Plan::Download(Box::new(store)));
    // 몇 걸음 진행시킨 뒤 취소한다.
    let mut pending = harness::writes(first);
    for _ in 0..4 {
        let back = remote.reply(&std::mem::take(&mut pending));
        pending = harness::writes(s.on_bytes(&back));
    }
    let steps = s.cancel();
    assert!(s.is_ended());
    assert!(matches!(steps.last(), Some(nabi_trzsz::Step::Failed(_))));
    assert!(!files.borrow()[0].2, "취소하면 받다 만 파일은 지운다");
}

#[test]
fn an_oversized_claim_is_refused() {
    // 원격이 SIZE보다 많은 DATA를 보내면 프로토콜 위반이다(디스크를 채우는 고전 수법).
    let mut remote = Remote::sender(vec![("x.bin".into(), b"0123456789".to_vec())]);
    remote.oversend = true;
    let t = trigger('S');
    let (s, first) = Session::new(&t, Plan::Download(Box::new(MemStorage::new())));
    let (_, err) = drive(s, first, &mut remote);
    assert!(err.unwrap().contains("more than the declared size"));
}
