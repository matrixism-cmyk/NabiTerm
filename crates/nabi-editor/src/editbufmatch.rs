//! **다중 커서 — 같은 낱말을 하나씩 더 잡기**(VS Code의 Ctrl+D, Sublime의 대표 기능).
//!
//! 선택 모델은 처음부터 "정렬·비겹침 범위 집합"이었다(`editsel`). 그 자료구조를 미리
//! 맞춰 둔 이유가 이것인데, 정작 **범위를 늘리는 길만 없었다.** 여기서 그 길을 낸다.
//!
//! ## 캐럿만 있을 때는 먼저 낱말을 잡는다
//!
//! VS Code와 같다. 아무것도 안 골랐는데 Ctrl+D를 누르면 커서가 놓인 낱말이 잡히고,
//! 한 번 더 누르면 그다음 같은 낱말이 잡힌다. 이 두 단계를 한 함수에 두는 편이
//! 사용자에게 자연스럽다 — 손가락은 같은 키를 반복할 뿐이다.
//!
//! ## 왜 문서를 통째로 문자열로 만들지 않는가
//!
//! 이 편집기는 수백 MB 파일을 연다. `rope.to_string()`은 그 크기만큼 사본을 하나 더 만든다.
//! 그래서 rope의 청크를 훑으며 찾는다 — 사본 없이, 찾은 만큼만.

use crate::editbuf::EditBuf;
use crate::editsel::Range;

impl EditBuf {
    /// 커서가 놓인 낱말의 범위(문자 단위). 낱말 위가 아니면 None.
    pub fn word_range_at(&self, at: usize) -> Option<Range> {
        let len = self.rope.len_chars();
        let at = at.min(len);
        let word = |i: usize| {
            let c = self.rope.char(i);
            c.is_alphanumeric() || c == '_'
        };
        // 커서는 낱말의 오른쪽 끝에 있을 수도 있다 — 양쪽을 본다.
        let inside = at < len && word(at);
        let after = at > 0 && word(at - 1);
        if !inside && !after {
            return None;
        }
        let mut s = at;
        while s > 0 && word(s - 1) {
            s -= 1;
        }
        let mut e = at;
        while e < len && word(e) {
            e += 1;
        }
        (s < e).then_some(Range { anchor: s, head: e })
    }

    /// **Ctrl+D** — 아무것도 안 골랐으면 낱말을 잡고, 이미 골랐으면 다음 같은 것을 더 잡는다.
    ///
    /// 더 잡을 것이 없으면 아무것도 하지 않고 false. (처음으로 되돌아가 감싸지 않는다 —
    /// 끝에서 계속 누르면 이미 잡은 것을 다시 잡아 개수가 줄어든 것처럼 보인다.)
    pub fn add_next_match(&mut self) -> bool {
        let p = self.sel.primary();
        if p.is_caret() {
            return match self.word_range_at(p.head) {
                Some(r) => {
                    self.sel.set_primary(r);
                    true
                }
                None => false,
            };
        }
        let needle = self.rope.slice(p.start()..p.end()).to_string();
        if needle.is_empty() {
            return false;
        }
        let n = needle.chars().count();
        let taken: Vec<usize> = self.sel.ranges().iter().map(|r| r.start()).collect();
        let mut from = p.end();
        // 이미 잡은 자리는 건너뛴다 — 안 그러면 같은 곳을 다시 잡고 병합돼 제자리걸음이 된다.
        // 다음 자리는 **겹치지 않게** 찾는다(편집기의 일치는 겹치지 않는다 — "aaaa"에서
        // "aa"는 둘이지 셋이 아니다).
        while let Some(at) = self.find_from(&needle, from) {
            if !taken.contains(&at) {
                self.sel.push(Range { anchor: at, head: at + n });
                return true;
            }
            from = at + n;
        }
        false
    }

    /// **모든 일치 선택** — 문서 전체에서 같은 것을 한 번에 잡는다.
    ///
    /// 캐럿뿐이면 낱말을 먼저 정한다. 잡은 개수를 돌려준다(0이면 아무 일도 없었다).
    pub fn select_all_matches(&mut self) -> usize {
        let p = self.sel.primary();
        let base = match p.is_caret() {
            true => match self.word_range_at(p.head) {
                Some(r) => r,
                None => return 0,
            },
            false => p,
        };
        let needle = self.rope.slice(base.start()..base.end()).to_string();
        if needle.is_empty() {
            return 0;
        }
        let n = needle.chars().count();
        self.match_capped = false;
        let mut sel = crate::editsel::Selection::single(base.start(), base.end());
        let mut from = 0usize;
        // 상한을 둔다. 한 글자를 전체 선택하면 수십만 개가 되어 화면이 멈춘다.
        const MAX: usize = 10_000;
        // 겹치지 않게 훑는다 — "aaaa"에서 "aa"는 둘이다.
        while let Some(at) = self.find_from(&needle, from) {
            if at != base.start() {
                sel.push(Range { anchor: at, head: at + n });
                if sel.len() >= MAX {
                    // **끊었다는 사실을 남긴다.** 조용히 자르면 사용자는 전부 잡힌 줄 알고
                    // 편집한다 — 그러면 나머지는 안 바뀐 채로 남는다.
                    self.match_capped = true;
                    break;
                }
            }
            from = at + n;
        }
        self.sel = sel;
        // **센 횟수가 아니라 실제 범위 수를 돌려준다.** push는 겹치면 병합하므로 둘이
        // 다를 수 있다(시험이 잡았다 — "aaaa"에서 셋이라고 답했었다).
        self.sel.len()
    }

    /// `from`(문자 인덱스)부터 처음 나오는 자리. rope를 문자열로 펼치지 않는다.
    fn find_from(&self, needle: &str, from: usize) -> Option<usize> {
        let len = self.rope.len_chars();
        if from >= len {
            return None;
        }
        let n: Vec<char> = needle.chars().collect();
        if n.is_empty() || n.len() > len - from {
            return None;
        }
        let mut it = self.rope.chars_at(from);
        let mut window: std::collections::VecDeque<char> = std::collections::VecDeque::new();
        let mut pos = from;
        for _ in 0..n.len() {
            window.push_back(it.next()?);
        }
        loop {
            if window.iter().copied().eq(n.iter().copied()) {
                return Some(pos);
            }
            let c = it.next()?;
            window.pop_front();
            window.push_back(c);
            pos += 1;
        }
    }
}
