//! SFTP 파일명 인코딩(벤더 패치 charset) 검증 — 워크스페이스 게이트에서 돌도록 여기 둔다.
//!
//! 벤더 크레이트(vendor/russh-sftp)의 자체 테스트는 workspace 멤버가 아니라 게이트에서
//! 실행되지 않는다. 전역 모드를 공유하므로 한 테스트 함수에 순차로 몰아넣는다.

use russh_sftp::charset;

fn cp949(s: &str) -> Vec<u8> {
    encoding_rs::EUC_KR.encode(s).0.into_owned()
}

#[test]
fn name_charset_roundtrip_and_modes() {
    // auto(기본): CP949 파일명이 무손실 왕복돼야 한다(사용자 보고 서버 사례).
    crate::set_name_charset("auto");
    let wire = cp949("한글파일.txt");
    assert!(std::str::from_utf8(&wire).is_err());
    let name = charset::decode(&wire);
    assert_eq!(name, "한글파일.txt", "CP949 파일명 디코드");
    charset::promote_candidate(); // list()가 readdir 응답에서 하는 일(파일명 확정).
    assert_eq!(charset::encode_path(&name), wire, "송신 시 서버 원본 바이트 복원");
    assert_eq!(crate::detected_name_charset(), Some("EUC-KR"), "auto 감지 배지");
    // v0.1.448 회귀 방지: 같은 서버에 섞여 있는 UTF-8 이름은(목록에서 받은 뒤) UTF-8로
    // 나가야 열린다 — 초판은 감지된 CP949로 재인코딩해 파일을 못 찾았다.
    assert_eq!(charset::decode("계약서.pdf".as_bytes()), "계약서.pdf");
    assert_eq!(charset::encode_path("계약서.pdf"), "계약서.pdf".as_bytes().to_vec());
    // 유효 UTF-8은 폴백을 타지 않는다(현대 서버 무영향).
    assert_eq!(charset::decode("한글.txt".as_bytes()), "한글.txt");
    // ASCII 경로는 어떤 모드에서도 그대로.
    assert_eq!(charset::encode_path("/var/log/a.txt"), b"/var/log/a.txt".to_vec());
    // 강제 utf8 모드 = 기존(lossy) 동작 유지.
    crate::set_name_charset("utf8");
    assert!(charset::decode(&wire).contains('\u{FFFD}'));
    // 강제 euc-kr 모드.
    crate::set_name_charset("euc-kr");
    assert_eq!(charset::decode(&wire), "한글파일.txt");
    assert_eq!(charset::encode_path("한글파일.txt"), wire);
    // 미지 라벨은 auto로 폴백 + 감지 리셋.
    crate::set_name_charset("nonsense");
    assert_eq!(crate::detected_name_charset(), None);
}

/// **큰 폴더를 훑어도 방금 본 이름을 잊으면 안 된다**(2026-09-01).
///
/// 원본 바이트 기억은 상한이 있고, 예전에는 차면 **통째로 비웠다.** 주석에는 "비어도
/// 규약 인코딩으로 폴백할 뿐이라 안전"하다고 적혀 있었지만, EUC-KR 로 감지된 서버에
/// UTF-8 이름이 섞여 있으면(바로 위 시험이 지키는 그 경우) 기억이 사라지는 순간
/// 그 이름이 CP949 로 재인코딩되어 **서버에서 못 찾는다** — v0.1.448 결함이 폴더가
/// 클 때만 되살아나는 셈이다. 8192 는 메일 큐·로그 폴더에서 예사로 넘는 수다.
///
/// 그래서 세대를 둘로 나눴다. 이 시험은 상한을 넘겨 훑은 뒤에도 **처음 본 이름**이
/// 원본 바이트로 나가는지 본다.
#[test]
fn a_big_listing_does_not_forget_what_it_just_saw() {
    crate::set_name_charset("auto");
    charset::forget_all();

    // 서버가 CP949 로 보낸 이름 하나(이것이 잊히면 안 된다).
    let first_wire = cp949("첫파일.txt");
    let first = charset::decode(&first_wire);
    assert_eq!(first, "첫파일.txt");
    charset::promote_candidate();
    // 같은 서버에 섞여 있는 UTF-8 이름 — 기억이 있어야 UTF-8 로 나간다.
    let utf8_name = charset::decode("계약서.pdf".as_bytes());

    // 상한(8192)을 넉넉히 넘겨 훑는다 — 큰 폴더 하나면 이만큼 나온다.
    for i in 0..9000 {
        let _ = charset::decode(&cp949(&format!("파일{i}.txt")));
    }

    // 처음 본 둘이 **여전히 원본 바이트로** 나가야 한다.
    assert_eq!(charset::encode_path(&first), first_wire, "CP949 이름을 잊었다");
    assert_eq!(
        charset::encode_path(&utf8_name),
        "계약서.pdf".as_bytes().to_vec(),
        "UTF-8 이름이 CP949 로 재인코딩됐다 — 서버에서 못 찾는다"
    );
    crate::set_name_charset("auto"); // 다른 시험에 영향 주지 않게 되돌린다.
}
