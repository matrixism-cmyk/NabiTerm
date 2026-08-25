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
    /// 정규식용 행 문자열(필요할 때만 만든다) + 문자별 바이트 위치.
    text: String,
    byte_at: Vec<usize>,
    text_ok: bool,
}

impl RowScan {
    /// 행을 평탄화한다(버퍼 재사용 — 할당은 처음 몇 행에서만 일어난다).
    pub(crate) fn rebuild(&mut self, row: &[RenderCell]) {
        self.chars.clear();
        self.lower.clear();
        self.owner.clear();
        self.text_ok = false;
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

impl RowScan {
    /// 행을 문자열로 한 번 만들어 둔다(정규식 규칙이 있을 때만 부른다).
    ///
    /// 평소에는 만들지 않는다 — 정규식을 안 쓰는 사람에게 행마다 String 할당을 물릴 이유가
    /// 없다(이 파일의 성능 이력 참고).
    fn ensure_text(&mut self) {
        if self.text_ok {
            return;
        }
        self.text.clear();
        self.byte_at.clear();
        for c in &self.chars {
            self.byte_at.push(self.text.len());
            self.text.push(*c);
        }
        self.byte_at.push(self.text.len()); // 끝 경계.
        self.text_ok = true;
    }

    /// 정규식과 맞는 셀을 표시한다.
    ///
    /// 정규식은 바이트 위치를 주고 우리는 셀 위치가 필요하다. `byte_at`으로 되짚는다 —
    /// 여기서 어긋나면 강조가 옆 칸에 찍힌다(옛 회귀와 같은 부류).
    pub(crate) fn mark_regex(&mut self, re: &regex::Regex, hl: &mut [bool]) {
        self.ensure_text();
        if self.text.is_empty() {
            return;
        }
        for m in re.find_iter(&self.text) {
            if m.start() == m.end() {
                continue; // 빈 일치는 온 행을 칠한다 — 무시한다.
            }
            let from = self.char_of_byte(m.start());
            let to = self.char_of_byte(m.end());
            for &o in self.owner.get(from..to).unwrap_or(&[]) {
                if let Some(slot) = hl.get_mut(o) {
                    *slot = true;
                }
            }
        }
    }

    /// 바이트 위치 → 문자 인덱스.
    fn char_of_byte(&self, b: usize) -> usize {
        match self.byte_at.binary_search(&b) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

/// 정규식을 프레임마다 다시 컴파일하지 않게 모아 둔다.
///
/// 컴파일은 수십 마이크로초다. 규칙 다섯 개면 초당 60프레임에서 무시 못 할 시간이 되고,
/// 규칙은 거의 안 바뀐다. 잘못된 정규식은 `None`으로 기억해 매 프레임 다시 시도하지 않는다.
pub(crate) fn compiled(pat: &str) -> Option<regex::Regex> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static CACHE: RefCell<HashMap<String, Option<regex::Regex>>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear(); // 규칙을 계속 고치는 경우에도 무한히 쌓이지 않게.
        }
        c.entry(pat.to_string())
            .or_insert_with(|| regex::Regex::new(pat).ok())
            .clone()
    })
}

#[cfg(test)]
mod regex_tests {
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

    fn marks(row: &[RenderCell], pat: &str) -> Vec<bool> {
        let mut hl = vec![false; row.len()];
        let mut s = RowScan::default();
        s.rebuild(row);
        s.mark_regex(&compiled(pat).unwrap(), &mut hl);
        hl
    }

    #[test]
    fn a_regex_marks_the_matching_cells() {
        let row = cells("ok ERROR ok");
        let m = marks(&row, "ERROR");
        assert_eq!(&m[3..8], &[true; 5], "{m:?}");
        assert!(!m[0] && !m[9]);
    }

    #[test]
    fn alternation_matches_either_side() {
        assert!(marks(&cells("a FATAL b"), "ERROR|FATAL").iter().any(|x| *x));
    }

    /// **한글 뒤에서 자리가 어긋나면 안 된다** — 바이트와 문자를 헷갈리면 여기서 드러난다.
    #[test]
    fn positions_survive_multibyte_text() {
        let row = cells("가나다 ERROR");
        let m = marks(&row, "ERROR");
        assert_eq!(&m[4..9], &[true; 5], "한글 뒤 자리가 밀렸다: {m:?}");
        assert!(!m[0] && !m[1] && !m[2]);
    }

    /// 빈 일치가 온 행을 칠하면 안 된다.
    #[test]
    fn an_empty_match_paints_nothing() {
        let row = cells("abc");
        assert!(!marks(&row, "x*").iter().any(|x| *x));
    }

    /// 잘못된 정규식은 조용히 없는 것이 된다 — 렌더 중에 터지면 UI가 죽는다.
    #[test]
    fn a_broken_pattern_compiles_to_nothing() {
        assert!(compiled("[unclosed").is_none());
        assert!(compiled(r"\d+").is_some());
    }

    /// 같은 패턴은 다시 컴파일하지 않는다(캐시가 값을 돌려준다).
    #[test]
    fn the_same_pattern_is_reused() {
        let a = compiled("abc").unwrap();
        let b = compiled("abc").unwrap();
        assert_eq!(a.as_str(), b.as_str());
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
