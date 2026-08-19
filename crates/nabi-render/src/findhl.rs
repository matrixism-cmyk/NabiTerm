//! 검색어/키워드 셀 일치 판정(스마트 케이스). painter에서 분리(라인 한도).
#![allow(clippy::needless_range_loop)]

use nabi_vt::RenderCell;

/// 한 행을 **한 번만** 평탄화해 여러 패턴(검색어 + 키워드 규칙 K개)을 검사하는 스캐너.
///
/// 예전엔 패턴마다 행을 다시 평탄화하고 셀마다 `to_lowercase()`로 String을 할당했다 —
/// 찾기 창이 열려 있거나 강조 규칙이 있으면 프레임당 (행×열×(1+K)) 할당이 됐다
/// (성능 리뷰 2026-08-19). 지금은 행당 한 번 평탄화하고 버퍼를 프레임 내내 재사용한다.
#[derive(Default)]
pub(crate) struct RowScan {
    /// 원문 문자열(대소문자 구분 검색용).
    chars: Vec<char>,
    /// 소문자 접기 결과(대소문자 무시 검색용) — 인덱스는 `chars`와 1:1.
    lower: Vec<char>,
    /// 문자 인덱스 → 소유 셀 인덱스(와이드/조합 문자 보정).
    owner: Vec<usize>,
}

impl RowScan {
    /// 행을 평탄화한다(버퍼 재사용 — 할당은 처음 몇 행에서만 일어난다).
    pub(crate) fn rebuild(&mut self, row: &[RenderCell]) {
        self.chars.clear();
        self.lower.clear();
        self.owner.clear();
        for (ci, cell) in row.iter().enumerate() {
            if cell.text.is_empty() {
                self.push(' ', ci);
            } else {
                for c in cell.text.chars() {
                    self.push(c, ci);
                }
            }
        }
    }

    fn push(&mut self, c: char, ci: usize) {
        self.chars.push(c);
        // to_lowercase는 1:n일 수 있으나(가령 İ) 첫 문자만 쓰면 인덱스 1:1이 유지된다 —
        // 터미널 셀 검색에서 실사용 차이는 없고, 정렬이 어긋나면 강조가 옆 칸에 찍힌다.
        self.lower.push(c.to_lowercase().next().unwrap_or(c));
        self.owner.push(ci);
    }

    /// `query`와 일치하는 셀을 `hl`에 표시한다(스마트 케이스: 대문자가 있으면 구분).
    pub(crate) fn mark(&self, query: &str, hl: &mut [bool]) {
        if query.is_empty() {
            return;
        }
        let cs = query.chars().any(char::is_uppercase);
        let hay: &[char] = if cs { &self.chars } else { &self.lower };
        let q: Vec<char> = if cs {
            query.chars().collect()
        } else {
            query.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
        };
        if q.is_empty() || hay.len() < q.len() {
            return;
        }
        for i in 0..=hay.len() - q.len() {
            if hay[i..i + q.len()] == q[..] {
                for &o in &self.owner[i..i + q.len()] {
                    if let Some(slot) = hl.get_mut(o) {
                        *slot = true;
                    }
                }
            }
        }
    }
}

/// 행 안에서 query와 일치하는 셀을 표시한다(단발 호출용 — 내부는 [`RowScan`]).
/// 렌더 경로는 버퍼를 재사용하는 [`RowScan`]을 직접 쓴다(할당 없음) — 여기는 테스트용.
#[cfg(test)]
pub(crate) fn match_cells(row: &[RenderCell], query: &str) -> Vec<bool> {
    let mut hl = vec![false; row.len()];
    let mut scan = RowScan::default();
    scan.rebuild(row);
    scan.mark(query, &mut hl);
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
