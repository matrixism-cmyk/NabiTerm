//! 코드 하이라이트 진입점 — editorsyntax의 전역 자산 + editorhlinc의 증분 계산.
//! onig(C) 회피로 syntect는 fancy-regex(순수 Rust) 빌드.

use egui::text::LayoutJob;

/// 매 프레임 문서 전체를 훑는 보조 기능(개요·괄호 짝·단어 강조·찾기 강조)의 상한.
/// 이 값을 넘는 문서에서는 해당 기능만 꺼진다(구문 강조 자체는 아래 상한을 따른다).
pub(crate) const MAX_HL_BYTES: usize = 200_000;

/// 구문 강조 상한 — 증분 계산이라 비용이 편집 구간에 묶이므로 문자열 편집 경로 전체를 덮는다
/// (이보다 큰 파일은 rope 편집기로 열리며 이 경로를 쓰지 않는다 — editbig::BIG_THRESHOLD).
pub(crate) const MAX_SYNTAX_BYTES: usize = 2_000_000;

/// 문서 `id`의 텍스트를 하이라이트한 LayoutJob. 바뀐 줄만 다시 계산한다(editorhlinc).
pub(crate) fn highlight(id: u64, text: &str, ext: &str, font_size: f32) -> LayoutJob {
    crate::editorhlinc::job(id, text, ext, font_size)
}
