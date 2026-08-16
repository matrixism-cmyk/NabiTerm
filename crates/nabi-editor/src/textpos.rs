//! 텍스트 좌표 헬퍼 — nabi-app termlink에서 이관(T5-1).

/// (0-base 줄, 1-base 열)을 텍스트 내 char 오프셋으로 환산. 줄/열이 범위를 넘으면 그 줄/문서 끝으로 클램프.
pub fn line_col_to_offset(text: &str, line0: usize, col1: u32) -> usize {
    let mut off = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        let len = l.chars().count();
        if i == line0 {
            return off + (col1.saturating_sub(1) as usize).min(len);
        }
        off += len + 1; // +1 = '\n'.
    }
    text.chars().count()
}

