//! 증분 강조 검증 — 핵심은 "증분 결과 == 처음부터 계산한 결과"다.
//! 이게 깨지면 색이 조용히 틀어지므로 편집 형태(수정·삽입·삭제·블록주석)별로 대조한다.

use super::{diff_range, IncHl};
use crate::editorsyntax::Assets;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

const THEME: &str = "base16-mocha.dark";

fn assets() -> Assets {
    Assets { ps: SyntaxSet::load_defaults_newlines(), themes: ThemeSet::load_defaults() }
}

fn hl(ext: &str) -> IncHl {
    IncHl::new(ext.into(), THEME.into())
}

/// (구간, 색) 목록 — 증분/전체 결과 비교용 지문.
fn fingerprint(j: &egui::text::LayoutJob) -> Vec<(std::ops::Range<usize>, egui::Color32)> {
    j.sections.iter().map(|s| (s.byte_range.clone(), s.format.color)).collect()
}

/// 같은 텍스트를 증분으로 도달했을 때와 새로 계산했을 때가 같은지 확인한다.
fn same_as_fresh(steps: &[&str], ext: &str) {
    let a = assets();
    let mut inc = hl(ext);
    for t in steps {
        let got = inc.job(t, 12.0, &a);
        assert_eq!(&got.text, t, "갤리 텍스트는 원문과 정확히 같아야 한다");
        let want = hl(ext).job(t, 12.0, &a);
        assert_eq!(fingerprint(&got), fingerprint(&want), "증분 결과가 전체 계산과 다름: {t:?}");
    }
}

#[test]
fn prefix_suffix_diff() {
    assert_eq!(diff_range(&[1, 2, 3], &[1, 2, 3]), (3, 0), "동일하면 접두가 전부");
    assert_eq!(diff_range(&[1, 2, 3], &[1, 9, 3]), (1, 1));
    assert_eq!(diff_range(&[1, 2, 3], &[1, 2, 9, 3]), (2, 1), "삽입");
    assert_eq!(diff_range(&[1, 2, 3], &[1, 3]), (1, 1), "삭제");
    assert_eq!(diff_range(&[], &[1, 2]), (0, 0), "빈 문서에서 시작");
}

#[test]
fn edit_insert_delete_match_full_recompute() {
    same_as_fresh(
        &[
            "fn a() {\n    let x = 1;\n    // c\n}\n",
            "fn a() {\n    let x = 22;\n    // c\n}\n", // 한 줄 수정
            "fn a() {\n    let x = 22;\n    let y = 3;\n    // c\n}\n", // 줄 삽입
            "fn a() {\n    // c\n}\n",                 // 줄 삭제
            "",                                        // 전체 삭제
            "fn a() {}\n",
        ],
        "rs",
    );
}

#[test]
fn block_comment_change_propagates() {
    // 블록 주석을 열면 뒤쪽 줄들의 색이 통째로 바뀐다 — 상태가 달라졌으므로 조기 중단하면 안 된다.
    same_as_fresh(
        &[
            "let a = 1;\nlet b = 2;\nlet c = 3;\n",
            "/* open\nlet b = 2;\nlet c = 3;\n",
            "let a = 1;\nlet b = 2;\nlet c = 3;\n", // 다시 닫아 원상 복귀
        ],
        "rs",
    );
}

#[test]
fn long_document_edit_matches() {
    // 체크포인트(100줄)를 여러 개 넘기는 길이 — 조기 중단 경로가 실제로 동작하는 구간.
    let base: String = (0..400).map(|i| format!("let v{i} = {i};\n")).collect();
    let mut edited: Vec<&str> = base.lines().collect();
    edited[250] = "// changed line";
    let after = edited.join("\n") + "\n";
    same_as_fresh(&[&base, &after, &base], "rs");
}

#[test]
fn font_size_change_rebuilds_job() {
    let a = assets();
    let mut inc = hl("rs");
    let t = "fn a() {}\n";
    let j12 = inc.job(t, 12.0, &a);
    let j20 = inc.job(t, 20.0, &a);
    assert_eq!(j12.text, j20.text);
    assert_eq!(j20.sections[0].format.font_id.size, 20.0, "글꼴 크기 변경이 반영돼야 함");
}
