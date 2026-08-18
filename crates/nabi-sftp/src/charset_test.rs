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
    assert_eq!(charset::encode_path(&name), wire, "송신 시 서버 원본 바이트 복원");
    assert_eq!(crate::detected_name_charset(), Some("EUC-KR"), "auto 감지 배지");
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
