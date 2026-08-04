//! 라인 캡 예외 허용 목록.
//!
//! 정당하게 큰 파일(생성 코드/대형 match 표/임베드 리소스)을 사유와 함께 등록한다.
//! 현재는 비어 있다 — 모든 파일이 캡을 지키는 것이 기본.

/// (워크스페이스 상대경로, 사유).
const ALLOW: &[(&str, &str)] = &[
    // 예: ("crates/nabi-render/src/wgsl.rs", "셰이더 소스 임베드"),
];

/// 주어진 상대경로가 캡 예외인지 확인한다.
pub fn is_allowed(rel_path: &str) -> bool {
    let norm = rel_path.replace('\\', "/");
    ALLOW.iter().any(|(p, _)| norm == *p)
}
