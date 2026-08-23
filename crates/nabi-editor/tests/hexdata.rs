//! 조각 표(HexData) 시험 — 대용량 HEX 편집의 토대라 여기가 틀리면 파일이 망가진다.
//!
//! 핵심 성질 두 가지를 계속 확인한다.
//! 1. **겉보기는 평범한 바이트 배열** — 어떤 편집 순서로도 `to_vec()`이 기대값과 같다.
//! 2. **원본을 복사하지 않는다** — 편집해도 조각 수만 늘고 메모리는 편집량에 비례한다.

use nabi_editor::hexdata::HexData;

/// 같은 편집을 평범한 Vec에 적용한 결과와 대조한다(기준 모델 비교).
fn model(v: &mut Vec<u8>, at: usize, del: usize, ins: &[u8]) {
    let at = at.min(v.len());
    let del = del.min(v.len() - at);
    v.splice(at..at + del, ins.iter().copied());
}

#[test]
fn behaves_like_a_plain_byte_array() {
    let base: Vec<u8> = (0..64u8).collect();
    let mut d = HexData::from_vec(base.clone());
    let mut m = base.clone();

    for (at, del, ins) in [
        (10usize, 0usize, &b"AB"[..]),   // 삽입
        (0, 3, &b""[..]),                // 앞을 삭제
        (20, 4, &b"XYZW"[..]),           // 같은 길이 덮어쓰기
        (5, 0, &b"hello"[..]),           // 중간 삽입
        (60, 2, &b"Q"[..]),              // 끝 근처 줄이기
    ] {
        d.splice(at, del, ins);
        model(&mut m, at, del, ins);
        assert_eq!(d.to_vec(), m, "at={at} del={del} ins={ins:?}");
        assert_eq!(d.len(), m.len());
    }
}

#[test]
fn reads_across_piece_boundaries() {
    let mut d = HexData::from_vec(b"0123456789".to_vec());
    d.splice(5, 0, b"---"); // 0123 4 --- 56789
    assert_eq!(d.to_vec(), b"01234---56789");
    // 조각 경계를 걸친 구간을 읽어도 이어져야 한다.
    assert_eq!(d.read(3, 6), b"34---5");
    assert_eq!(d.read(0, 100), b"01234---56789", "범위를 넘으면 있는 만큼만");
    assert_eq!(d.read(99, 5), b"", "범위 밖은 빈 결과");
}

#[test]
fn single_bytes_come_back_in_order() {
    let mut d = HexData::from_vec(vec![1, 2, 3]);
    d.splice(1, 1, &[9, 9]); // 1 9 9 3
    let got: Vec<u8> = (0..d.len()).map(|i| d.get(i).unwrap()).collect();
    assert_eq!(got, vec![1, 9, 9, 3]);
    assert_eq!(d.get(d.len()), None, "범위 밖은 None");
}

/// 이어 치는 편집이 조각을 무한정 늘리면 안 된다(합치기가 동작하는가).
#[test]
fn sequential_overwrites_do_not_explode_the_piece_list() {
    let mut d = HexData::from_vec(vec![0u8; 1000]);
    for i in 0..200 {
        d.splice(i, 1, &[0xFF]); // 앞에서부터 한 바이트씩 덮어쓴다
    }
    assert_eq!(d.len(), 1000);
    assert_eq!(&d.to_vec()[..200], &[0xFFu8; 200][..]);
    assert!(d.piece_count() <= 3, "조각이 {}개나 됐다", d.piece_count());
}

#[test]
fn deleting_everything_leaves_an_empty_document() {
    let mut d = HexData::from_vec(b"abcdef".to_vec());
    d.splice(0, 6, b"");
    assert!(d.is_empty());
    assert_eq!(d.to_vec(), b"");
    d.splice(0, 0, b"new");
    assert_eq!(d.to_vec(), b"new");
}

#[test]
fn finds_patterns_across_pieces() {
    let mut d = HexData::from_vec(b"the quick brown".to_vec());
    d.splice(10, 0, b"very "); // 'b' 자리(10) 앞에 끼운다 → "the quick very brown"
    assert_eq!(d.to_vec(), b"the quick very brown");
    assert_eq!(d.find(b"very", 0), Some(10));
    assert_eq!(d.find(b"k very b", 0), Some(8), "조각 경계를 걸친 패턴");
    assert_eq!(d.find(b"the", 1), None, "시작 위치 뒤에서만 찾는다");
    assert_eq!(d.find(b"", 0), None);
}

/// 64KB 청크 경계를 걸친 패턴도 찾아야 한다 — 겹침 처리가 틀리면 여기서 걸린다.
#[test]
fn finds_a_pattern_straddling_the_scan_chunk() {
    let mut v = vec![b'.'; 200_000];
    let at = 65_530; // 첫 청크(65536)의 끝을 걸친다
    v[at..at + 6].copy_from_slice(b"NEEDLE");
    let d = HexData::from_vec(v);
    assert_eq!(d.find(b"NEEDLE", 0), Some(at));
}

/// 파일을 매핑해 열고, 편집한 뒤, 흘려 써서 저장한다 — 전체 왕복.
#[test]
fn maps_a_file_edits_it_and_writes_it_back() {
    let dir = std::env::temp_dir().join(format!("nabi-hexdata-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("sample.bin");
    let original: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
    std::fs::write(&src, &original).unwrap();

    let mut d = HexData::map_file(&src).expect("매핑");
    assert_eq!(d.len(), original.len());
    assert_eq!(d.piece_count(), 1, "열자마자는 조각 하나 — 아무것도 복사하지 않았다");

    d.splice(1000, 4, b"nabi"); // 같은 길이 덮어쓰기
    d.splice(0, 0, b"HEAD"); // 앞에 삽입

    let mut out = Vec::new();
    d.write_to(&mut out).unwrap();

    let mut want = original.clone();
    want[1000..1004].copy_from_slice(b"nabi");
    want.splice(0..0, b"HEAD".iter().copied());
    assert_eq!(out.len(), want.len());
    assert_eq!(out, want);

    let _ = std::fs::remove_dir_all(&dir);
}

/// 빈 파일도 열려야 한다(0바이트는 매핑할 수 없어 따로 다룬다).
#[test]
fn an_empty_file_opens_as_an_empty_document() {
    let dir = std::env::temp_dir().join(format!("nabi-hexdata-e{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("empty.bin");
    std::fs::write(&p, b"").unwrap();
    let mut d = HexData::map_file(&p).expect("빈 파일도 열린다");
    assert!(d.is_empty());
    d.splice(0, 0, b"x");
    assert_eq!(d.to_vec(), b"x");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 이 배치의 목적 그 자체 — **크기 제한 없이 열리고, 여는 데 파일을 읽지 않는다.**
///
/// 예전 HEX 버퍼는 16MB를 넘으면 편집을 막고 읽기 전용 뷰어로 떨어뜨렸다.
#[test]
fn a_file_far_over_the_old_16mb_limit_opens_for_editing() {
    use nabi_editor::edithex::HexBuf;
    let dir = std::env::temp_dir().join(format!("nabi-hexbig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("big.bin");

    // 예전 상한(16MB)의 네 배.
    let size = 64 * 1024 * 1024usize;
    {
        use std::io::Write;
        let f = std::fs::File::create(&p).unwrap();
        let mut w = std::io::BufWriter::new(f);
        let chunk = vec![0xA5u8; 1 << 20];
        for _ in 0..(size >> 20) {
            w.write_all(&chunk).unwrap();
        }
        w.flush().unwrap();
    }

    let mut h = HexBuf::open(&p).expect("크기와 무관하게 열려야 한다");
    assert_eq!(h.len(), size);
    assert_eq!(h.at(0), Some(0xA5));
    assert_eq!(h.at(size - 1), Some(0xA5));

    // 한가운데를 고친다 — 조각 몇 개만 늘 뿐 파일을 복제하지 않는다.
    h.splice(size / 2, 4, b"nabi", false);
    assert_eq!(h.range(size / 2, size / 2 + 4), b"nabi");
    assert_eq!(h.len(), size, "덮어쓰기는 길이를 바꾸지 않는다");

    // 취소도 동작한다.
    h.undo();
    assert_eq!(h.range(size / 2, size / 2 + 4), vec![0xA5; 4]);

    let _ = std::fs::remove_dir_all(&dir);
}
