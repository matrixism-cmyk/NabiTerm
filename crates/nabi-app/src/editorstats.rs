//! nabiPad 문서 통계(줄/단어/문자/바이트) — 보기 메뉴 "문서 정보"에 표시. 순수 계산.

pub(crate) struct DocStats {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    pub chars_no_ws: usize,
    pub bytes: usize,
    pub paragraphs: usize,
    pub max_line: usize,
    pub avg_line: usize,
}

/// 문서 텍스트의 줄·단어·문자(공백 포함/제외)·바이트 + 단락 수·최장/평균 줄 길이를 센다.
/// 줄 수는 `split('\n')` 기준(빈 문서는 0). 단어는 공백 분리 토큰 수.
pub(crate) fn document_stats(text: &str) -> DocStats {
    let line_lens: Vec<usize> = text.split('\n').map(|l| l.chars().count()).collect();
    let lines = if text.is_empty() { 0 } else { line_lens.len() };
    // 단락 = 빈 줄로 구분된 비어있지 않은 블록.
    let mut paragraphs = 0;
    let mut in_para = false;
    for l in text.split('\n') {
        if l.trim().is_empty() {
            in_para = false;
        } else if !in_para {
            paragraphs += 1;
            in_para = true;
        }
    }
    let sum: usize = line_lens.iter().sum();
    DocStats {
        lines,
        words: text.split_whitespace().count(),
        chars: text.chars().count(),
        chars_no_ws: text.chars().filter(|c| !c.is_whitespace()).count(),
        bytes: text.len(),
        paragraphs,
        max_line: line_lens.iter().copied().max().unwrap_or(0),
        avg_line: if lines == 0 { 0 } else { sum.div_ceil(lines) },
    }
}

#[cfg(test)]
mod tests {
    use super::document_stats;

    #[test]
    fn counts_basic() {
        let s = document_stats("hello world\nbye");
        assert_eq!(s.lines, 2);
        assert_eq!(s.words, 3);
        assert_eq!(s.chars, 15); // "hello world\nbye" 길이(개행 포함).
        assert_eq!(s.chars_no_ws, 13); // 공백·개행 제외.
        assert_eq!(s.bytes, 15);
    }

    #[test]
    fn paragraphs_and_lines() {
        let s = document_stats("aa\nbbbb\n\ncc\n\n\nd");
        assert_eq!(s.paragraphs, 3); // 빈 줄로 나뉜 비어있지 않은 블록 3개.
        assert_eq!(s.max_line, 4); // "bbbb".
        assert_eq!(s.lines, 7);
        assert_eq!(document_stats("").paragraphs, 0);
    }

    #[test]
    fn empty_and_multibyte() {
        let e = document_stats("");
        assert_eq!((e.lines, e.words, e.chars, e.bytes), (0, 0, 0, 0));
        let k = document_stats("한글"); // 2 chars, UTF-8 6 bytes.
        assert_eq!(k.chars, 2);
        assert_eq!(k.bytes, 6);
        assert_eq!(k.lines, 1);
    }
}
