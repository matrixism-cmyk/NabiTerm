//! 검색어/키워드 셀 일치 판정(스마트 케이스). painter에서 분리(라인 한도).
#![allow(clippy::needless_range_loop)]

use nabi_vt::RenderCell;

/// 행 안에서 query(스마트 케이스: 대문자 있으면 구분, 없으면 무시)와 일치하는 셀을 표시한다.
pub(crate) fn match_cells(row: &[RenderCell], query: &str) -> Vec<bool> {
    let mut hl = vec![false; row.len()];
    if query.is_empty() {
        return hl;
    }
    let cs = query.chars().any(|c| c.is_uppercase());
    let fold = |s: &str| -> String {
        if cs {
            s.to_string()
        } else {
            s.to_lowercase()
        }
    };
    let q: Vec<char> = fold(query).chars().collect();
    let mut chars: Vec<char> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    for (ci, cell) in row.iter().enumerate() {
        if cell.text.is_empty() {
            chars.push(' ');
            owner.push(ci);
        } else {
            for c in fold(&cell.text).chars() {
                chars.push(c);
                owner.push(ci);
            }
        }
    }
    if chars.len() >= q.len() {
        for i in 0..=chars.len() - q.len() {
            if chars[i..i + q.len()] == q[..] {
                for &o in &owner[i..i + q.len()] {
                    hl[o] = true;
                }
            }
        }
    }
    hl
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_types::{CellAttrs, Rgba};

    fn cells(s: &str) -> Vec<RenderCell> {
        s.chars()
            .map(|c| RenderCell {
                text: c.to_string(),
                fg: Rgba::WHITE,
                bg: Rgba::BLACK,
                attrs: CellAttrs::default(),
                ul_color: None,
            })
            .collect()
    }

    #[test]
    fn find_highlights_case_insensitive_match() {
        let row = cells("Hello World");
        let hl = match_cells(&row, "world");
        assert!(hl[6] && hl[7] && hl[8] && hl[9] && hl[10], "World 하이라이트");
        assert!(!hl[0], "Hello는 비일치");
        assert_eq!(match_cells(&row, "xyz").iter().filter(|b| **b).count(), 0);
    }

    #[test]
    fn find_smart_case_sensitive_when_uppercase() {
        let row = cells("abc ABC");
        let hl = match_cells(&row, "ABC");
        assert!(hl[4] && hl[5] && hl[6], "대문자 ABC 일치");
        assert!(!hl[0] && !hl[1] && !hl[2], "소문자 abc 비일치");
    }
}
