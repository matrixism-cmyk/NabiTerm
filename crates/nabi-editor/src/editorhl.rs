//! 코드 하이라이트 진입점 — editorsyntax의 전역 자산 + editorhlinc의 증분 계산.
//! onig(C) 회피로 syntect는 fancy-regex(순수 Rust) 빌드.

use egui::text::LayoutJob;

/// 매 프레임 문서 전체를 훑는 보조 기능(개요·괄호 짝·단어 강조·찾기 강조)의 상한.
/// 이 값을 넘는 문서에서는 해당 기능만 꺼진다(구문 강조 자체는 아래 상한을 따른다).
pub const MAX_HL_BYTES: usize = 200_000;

/// 구문 강조 상한 — 증분 계산이라 비용이 편집 구간에 묶이므로 문자열 편집 경로 전체를 덮는다
/// (이보다 큰 파일은 rope 편집기로 열리며 이 경로를 쓰지 않는다 — editbig::BIG_THRESHOLD).
pub const MAX_SYNTAX_BYTES: usize = 2_000_000;

/// 문서 `id`의 텍스트를 하이라이트한 LayoutJob. 바뀐 줄만 다시 계산한다(editorhlinc).
///
/// **표 파일은 구문이 아니라 칸으로 칠한다**(`csvcolor`). syntect 에 CSV 문법을 물어봐야
/// 얻을 것이 없다 — 표에서 알고 싶은 것은 "이 값이 몇 번째 칸인가"지 "이것이 문자열인가"가
/// 아니다. 칸이 어긋난 줄은 색이 어긋나 보여서 눈에 먼저 들어온다.
pub fn highlight(id: u64, text: &str, ext: &str, font_size: f32) -> LayoutJob {
    // 큰 표는 그냥 둔다 — 이 계산은 증분이 아니라 매번 전체를 훑으므로, 다른 전체 훑기
    // 기능들과 같은 상한을 쓴다(개요·괄호 짝과 같은 규칙이라 따로 외울 것이 없다).
    if let Some(delim) = table_delim(ext) {
        if text.len() <= MAX_HL_BYTES {
            return table_job(text, delim, font_size);
        }
    }
    crate::editorhlinc::job(id, text, ext, font_size)
}

/// 표 파일이면 그 구분자. 아니면 `None`.
fn table_delim(ext: &str) -> Option<char> {
    match ext.to_ascii_lowercase().as_str() {
        "csv" => Some(','),
        "tsv" | "tab" => Some('\t'),
        _ => None,
    }
}

/// 칸마다 색이 다른 LayoutJob.
fn table_job(text: &str, delim: char, font_size: f32) -> LayoutJob {
    let font = egui::FontId::monospace(font_size);
    let mut job = LayoutJob::default();
    for (a, b, col) in crate::csvcolor::spans(text, delim) {
        let fmt = egui::TextFormat { font_id: font.clone(), color: crate::csvcolor::color_of(col), ..Default::default() };
        job.append(&text[a..b], 0.0, fmt);
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

#[cfg(test)]
mod tabletests {
    use super::*;

    #[test]
    fn only_table_extensions_get_column_colors() {
        assert_eq!(table_delim("csv"), Some(','));
        assert_eq!(table_delim("TSV"), Some('\t'), "확장자 대소문자를 가리지 않는다");
        assert_eq!(table_delim("rs"), None);
        assert_eq!(table_delim("txt"), None);
    }

    /// **글자가 하나도 새지 않아야 한다.** 조각을 이어 붙인 것이 원본과 달라지면
    /// 화면에서 글이 사라지거나 겹친다.
    #[test]
    fn the_job_contains_the_whole_text() {
        let text = "이름,값\n가나다,3\n";
        let job = table_job(text, ',', 12.0);
        assert_eq!(job.text, text);
    }

    /// 칸 색이 몇 가지나 쓰였는가 — 이것으로 두 경로를 가린다.
    ///
    /// 길이로는 못 가린다. 되돌아가는 경로도 글 전체를 담기 때문이다(그렇게 재려다
    /// 시험이 빨개져서 알았다).
    fn column_colors(job: &LayoutJob) -> usize {
        let mut seen: Vec<egui::Color32> = Vec::new();
        for s in &job.sections {
            let c = s.format.color;
            if crate::csvcolor::COLORS.contains(&c) && !seen.contains(&c) {
                seen.push(c);
            }
        }
        seen.len()
    }

    /// 작은 표는 칸마다 다른 색이 붙는다.
    #[test]
    fn a_small_table_gets_more_than_one_column_color() {
        let job = highlight(1, "a,b,c\n1,2,3\n", "csv", 12.0);
        assert!(column_colors(&job) >= 3, "칸 색이 안 붙었다");
    }

    /// 큰 표는 이 경로를 타지 않는다 — 매 프레임 전체를 훑으면 안 된다.
    #[test]
    fn a_huge_table_falls_back_to_the_normal_path() {
        let big = "a,b\n".repeat(MAX_HL_BYTES / 4 + 10);
        assert!(big.len() > MAX_HL_BYTES);
        let job = highlight(2, &big, "csv", 12.0);
        assert_eq!(column_colors(&job), 0, "상한을 넘겼는데 칸 색칠을 했다");
    }
}
