//! 대용량 텍스트 편집을 위한 **줄 인덱스** — 줄 시작 바이트 오프셋만 들고, 편집마다 국소 갱신.
//!
//! ## 왜 필요한가
//!
//! rope(ropey)는 문서 전체를 RAM에 올린다. 그래서 지금까지 512MB(`editbuf::EDIT_CAP`)를 넘는
//! 텍스트는 편집을 막고 읽기 전용 뷰어로 떨어뜨렸다. 사용자 요구는 명확하다 — **용량 제한을
//! 두지 않는다.**
//!
//! 바이트는 이미 [`crate::hexdata::HexData`]가 조각 표로 들고 있다(원본 mmap + 덧댐 버퍼).
//! 텍스트 편집기에 없던 나머지 절반이 이것, **줄 → 바이트 오프셋** 표다. 둘을 합치면
//! 문서를 메모리에 올리지 않고도 "n번째 줄을 그려라"에 답할 수 있다.
//!
//! ## 커서를 char 오프셋이 아니라 (줄, 열)로 두는 이유
//!
//! rope의 값어치는 문서 전역 char↔줄 변환이 O(log n)이라는 데 있다. 우리는 그 변환 자체를
//! 안 하면 된다. 커서를 (줄, 줄 안 바이트)로 두면 열 계산은 **그 줄 문자열 안에서만** 일어나고,
//! 그건 길어야 수백 바이트다. 전역 인덱스는 줄 시작만 있으면 충분하다.
//!
//! ## 타자 한 번에 표 전체를 밀지 않는 법
//!
//! 줄 하나에 글자를 넣으면 뒤쪽 줄 시작이 전부 1씩 밀린다. 1GB 문서면 1700만 개다. 매 키마다
//! 그걸 다 더하면 화면이 끊긴다. 그래서 **`pivot` 이후는 `delta`를 얹어 읽는다**(갭 버퍼와
//! 같은 발상). 같은 자리에서 계속 치는 동안은 `delta`만 늘어나 O(1)이고, 커서를 옮겨 다른
//! 자리를 고칠 때 한 번 훑어 정리(`flush`)한다.

use memchr::memchr_iter;

/// 한 번 편집할 때 다시 읽는 최대 구간(바이트).
///
/// 보통은 편집이 걸친 줄 전체를 다시 읽는다. 하지만 개행 없는 초대형 줄(로그 한 덩어리,
/// minified JS)에서는 그 "한 줄"이 파일 전체다. 그런 줄에는 되찾을 줄 시작이 애초에 없으니,
/// 편집 자리 둘레만 봐도 결과가 같다.
const MAX_REGION: u64 = 1 << 20;

/// 줄 시작 바이트 오프셋 표. 줄 수 = `starts.len()`이며, 마지막 줄은 문서 끝까지다.
///
/// 개행이 `\n`으로 정규화된 바이트 열을 전제로 한다(CRLF는 열 때 정규화하고 저장할 때 되돌린다).
pub struct LineIndex {
    /// 원시 오프셋. `pivot` 이상 항목은 `delta`를 더해야 실제 값이 된다.
    starts: Vec<u64>,
    pivot: usize,
    delta: i64,
    total: u64,
}

/// `base`를 시작 오프셋으로 보고 `buf` 안의 줄 시작들을 모은다(`buf` 첫 바이트 자체는 제외).
///
/// 줄 시작은 개행 **다음** 바이트다. 버퍼가 개행으로 끝나면 그 다음 자리는 버퍼 밖이라
/// 보통은 넣지 않는다 — 뒤에 이어지는 줄의 항목이 이미 그 자리를 맡고 있기 때문이다.
/// 다만 이 구간이 **문서 끝**이면(`tail`) 뒤에 맡아 줄 항목이 없으므로, 마지막 빈 줄을
/// 우리가 직접 넣어야 한다. 이걸 빠뜨리면 문서 끝에서 Enter를 쳐도 줄이 늘지 않는다.
fn scan_starts(buf: &[u8], base: u64, tail: bool) -> Vec<u64> {
    let mut out = Vec::new();
    for i in memchr_iter(b'\n', buf) {
        if i + 1 < buf.len() || tail {
            out.push(base + i as u64 + 1);
        }
    }
    out
}

impl LineIndex {
    /// 메모리에 있는 바이트 전체로 표를 만든다.
    pub fn build(buf: &[u8]) -> Self {
        let total = buf.len() as u64;
        let mut starts = Vec::with_capacity(1 + buf.len() / 48);
        starts.push(0);
        for i in memchr_iter(b'\n', buf) {
            starts.push(i as u64 + 1);
        }
        Self { starts, pivot: 0, delta: 0, total }
    }

    /// 이미 만들어 둔 오프셋 목록으로 표를 세운다(백그라운드 스캔 결과를 받을 때).
    ///
    /// `starts`는 오름차순이어야 하고 첫 항목은 0이어야 한다 — 아니면 앞을 보정한다.
    pub fn from_starts(mut starts: Vec<u64>, total: u64) -> Self {
        if starts.first() != Some(&0) {
            starts.insert(0, 0);
        }
        Self { starts, pivot: 0, delta: 0, total }
    }

    pub fn lines(&self) -> usize {
        self.starts.len()
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    /// `line`번째 줄이 시작하는 바이트 오프셋. 범위를 넘으면 문서 끝.
    pub fn start(&self, line: usize) -> u64 {
        match self.starts.get(line) {
            Some(&s) if line >= self.pivot => (s as i64 + self.delta) as u64,
            Some(&s) => s,
            None => self.total,
        }
    }

    /// `line`번째 줄이 끝나는 바이트 오프셋(**개행을 포함**한 배타적 끝).
    pub fn end(&self, line: usize) -> u64 {
        if line + 1 < self.starts.len() { self.start(line + 1) } else { self.total }
    }

    /// 오프셋이 속한 줄 번호. 유효 오프셋은 정렬돼 있으므로 이분 탐색한다.
    pub fn line_of(&self, off: u64) -> usize {
        let (mut lo, mut hi) = (0usize, self.starts.len());
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.start(mid) <= off { lo = mid } else { hi = mid }
        }
        lo
    }

    /// `pivot` 이후에 미뤄 둔 `delta`를 실제 값에 반영한다.
    fn flush(&mut self) {
        if self.delta != 0 {
            for s in &mut self.starts[self.pivot..] {
                *s = (*s as i64 + self.delta) as u64;
            }
            self.delta = 0;
        }
        self.pivot = 0;
    }

    /// `[at, at+del)`을 `ins`로 바꾼 뒤 표를 맞춘다. `region`은 편집 **후**의 바이트 중
    /// 영향을 받은 줄 구간 전체다([`edit_region`]가 무엇을 읽어야 하는지 알려준다).
    ///
    /// 영향 범위를 줄 단위로 통째로 다시 스캔하므로, 개행이 들어가고 빠지는 온갖 경우를
    /// 따로 따지지 않는다 — 경계 조건을 손으로 세다 틀리느니 그 줄들을 다시 읽는 편이 낫다.
    pub fn patch(&mut self, at: u64, del: u64, ins_len: u64, region_start: u64, region: &[u8]) {
        let at = at.min(self.total);
        let del = del.min(self.total - at);
        let (l0, l1) = (self.line_of(at), self.line_of(at + del));
        let diff = ins_len as i64 - del as i64;

        // 이번 편집이 만들어 낸, 영향 구간 안의 새 줄 시작들.
        // 구간이 잘렸는지(줄 처음에서 시작하지 않는지) 본다. 잘린 구간에는 그 줄의 시작이
        // 들어 있지 않으므로, 거기서 찾은 개행만 새 줄 시작으로 넣으면 된다 — 잘라 낸 쪽에는
        // 개행이 없다는 것이 자르기의 전제다(그래서 그 줄이 그렇게 길었다).
        let new_total = (self.total as i64 + diff) as u64;
        let at_end = l1 + 1 >= self.starts.len() && region_start + region.len() as u64 >= new_total;
        let fresh = scan_starts(region, region_start, at_end);
        let old_count = l1 - l0; // 지워질 항목 수(l0 자신은 남는다).

        // 미뤄 둔 delta를 그대로 이어 쓰려면 두 조건이 다 맞아야 한다. 항목 수가 그대로여야
        // 하고(아니면 어차피 전체를 미는 비용이다), 밀어 둔 경계가 이번 구간 바로 뒤여야 한다
        // (아니면 이번에 새로 넣는 값에까지 delta가 얹혀 버린다).
        if fresh.len() != old_count || (self.delta != 0 && self.pivot != l1 + 1) {
            self.flush();
        }
        let next = l0 + 1 + fresh.len(); // 영향 구간 바로 다음 항목의 자리.
        self.starts.splice(l0 + 1..l1 + 1, fresh);
        self.total = (self.total as i64 + diff) as u64;
        if self.delta == 0 {
            self.pivot = next.min(self.starts.len());
        }
        self.delta += diff;
    }

    /// 편집 `[at, at+del)`이 영향을 주는 **줄 구간**(편집 전 좌표, 배타적 끝).
    ///
    /// 편집이 걸친 첫 줄의 처음부터 마지막 줄의 끝까지다. 호출자는 이 구간의 편집 후 바이트를
    /// 읽어 [`patch`](Self::patch)에 넘긴다.
    ///
    /// **다만 무한정 넓히지 않는다.** 개행이 하나도 없는 1GB 파일이면 "그 줄"이 곧 파일 전체라,
    /// 글자 하나 칠 때마다 1GB를 복사하게 된다(교차 검토 2026-08-25). 구간이 [`MAX_REGION`]을
    /// 넘으면 편집 자리 둘레만 잘라 쓴다 — 그렇게 잘라도 맞는 이유는 [`patch`]에 적어 두었다.
    pub fn edit_region(&self, at: u64, del: u64) -> (u64, u64) {
        let at = at.min(self.total);
        let del = del.min(self.total - at);
        let (a, b) = (self.start(self.line_of(at)), self.end(self.line_of(at + del)));
        if b - a <= MAX_REGION {
            return (a, b);
        }
        let pad = MAX_REGION / 4;
        (a.max(at.saturating_sub(pad)), b.min(at + del + pad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 표가 말하는 줄들을 실제로 잘라 낸다 — 기대값과 통째로 비교하기 위해.
    fn lines_of(ix: &LineIndex, buf: &[u8]) -> Vec<String> {
        (0..ix.lines())
            .map(|i| {
                let (a, b) = (ix.start(i) as usize, ix.end(i) as usize);
                String::from_utf8_lossy(&buf[a..b]).trim_end_matches('\n').to_string()
            })
            .collect()
    }

    /// 편집을 바이트 열과 표에 **동시에** 적용해, 표가 처음부터 다시 만든 것과 같은지 본다.
    /// 국소 갱신이 전체 재계산과 어긋나지 않는다는 것이 이 자료구조의 유일한 계약이다.
    fn apply(buf: &mut Vec<u8>, ix: &mut LineIndex, at: usize, del: usize, ins: &[u8]) {
        let (rs, re) = ix.edit_region(at as u64, del as u64);
        let new_re = (re as i64 + ins.len() as i64 - del as i64) as usize;
        buf.splice(at..at + del, ins.iter().copied());
        let region = buf[rs as usize..new_re.min(buf.len())].to_vec();
        ix.patch(at as u64, del as u64, ins.len() as u64, rs, &region);
        let want = LineIndex::build(buf);
        assert_eq!(
            (0..ix.lines()).map(|i| ix.start(i)).collect::<Vec<_>>(),
            (0..want.lines()).map(|i| want.start(i)).collect::<Vec<_>>(),
            "표가 재계산과 어긋남: at={at} del={del} ins={:?}",
            String::from_utf8_lossy(ins)
        );
        assert_eq!(ix.total(), buf.len() as u64);
    }

    #[test]
    fn an_empty_document_has_one_empty_line() {
        let ix = LineIndex::build(b"");
        assert_eq!(ix.lines(), 1);
        assert_eq!((ix.start(0), ix.end(0)), (0, 0));
    }

    #[test]
    fn a_trailing_newline_makes_a_final_empty_line() {
        let ix = LineIndex::build(b"abc\n");
        assert_eq!(ix.lines(), 2);
        assert_eq!(lines_of(&ix, b"abc\n"), vec!["abc", ""]);
    }

    #[test]
    fn lines_without_a_trailing_newline_are_all_counted() {
        let buf = b"one\ntwo\nthree";
        let ix = LineIndex::build(buf);
        assert_eq!(lines_of(&ix, buf), vec!["one", "two", "three"]);
    }

    #[test]
    fn line_of_maps_every_offset_back_to_its_line() {
        let buf = b"aa\nbb\ncc";
        let ix = LineIndex::build(buf);
        let got: Vec<usize> = (0..buf.len()).map(|o| ix.line_of(o as u64)).collect();
        assert_eq!(got, vec![0, 0, 0, 1, 1, 1, 2, 2]);
    }

    #[test]
    fn typing_inside_one_line_shifts_every_later_line() {
        let (mut buf, mut ix) = (b"aa\nbb\ncc".to_vec(), LineIndex::build(b"aa\nbb\ncc"));
        apply(&mut buf, &mut ix, 1, 0, b"XY");
        assert_eq!(lines_of(&ix, &buf), vec!["aXYa", "bb", "cc"]);
    }

    #[test]
    fn pressing_enter_splits_a_line_in_two() {
        let (mut buf, mut ix) = (b"aa\nbb".to_vec(), LineIndex::build(b"aa\nbb"));
        apply(&mut buf, &mut ix, 4, 0, b"\n");
        assert_eq!(lines_of(&ix, &buf), vec!["aa", "b", "b"]);
    }

    #[test]
    fn backspacing_a_newline_joins_two_lines() {
        let (mut buf, mut ix) = (b"aa\nbb".to_vec(), LineIndex::build(b"aa\nbb"));
        apply(&mut buf, &mut ix, 2, 1, b"");
        assert_eq!(lines_of(&ix, &buf), vec!["aabb"]);
    }

    #[test]
    fn deleting_across_many_lines_collapses_them() {
        let src = b"one\ntwo\nthree\nfour\nfive";
        let (mut buf, mut ix) = (src.to_vec(), LineIndex::build(src));
        apply(&mut buf, &mut ix, 2, 12, b""); // "e\ntwo\nthree\n" 를 지운다.
        assert_eq!(lines_of(&ix, &buf), vec!["onfour", "five"]);
    }

    #[test]
    fn pasting_a_block_of_lines_inserts_every_one() {
        let (mut buf, mut ix) = (b"top\nend".to_vec(), LineIndex::build(b"top\nend"));
        apply(&mut buf, &mut ix, 4, 0, b"a\nb\nc\n");
        assert_eq!(lines_of(&ix, &buf), vec!["top", "a", "b", "c", "end"]);
    }

    #[test]
    fn replacing_a_whole_line_keeps_the_line_count() {
        let (mut buf, mut ix) = (b"aa\nbb\ncc".to_vec(), LineIndex::build(b"aa\nbb\ncc"));
        apply(&mut buf, &mut ix, 3, 2, b"ZZZZ");
        assert_eq!(lines_of(&ix, &buf), vec!["aa", "ZZZZ", "cc"]);
    }

    #[test]
    fn typing_at_the_very_end_appends() {
        let (mut buf, mut ix) = (b"aa\nbb".to_vec(), LineIndex::build(b"aa\nbb"));
        apply(&mut buf, &mut ix, 5, 0, b"!");
        apply(&mut buf, &mut ix, 6, 0, b"\n");
        assert_eq!(lines_of(&ix, &buf), vec!["aa", "bb!", ""]);
    }

    #[test]
    fn typing_into_an_empty_document_works() {
        let (mut buf, mut ix) = (Vec::new(), LineIndex::build(b""));
        for (i, c) in b"hi\nyo".iter().enumerate() {
            apply(&mut buf, &mut ix, i, 0, &[*c]);
        }
        assert_eq!(lines_of(&ix, &buf), vec!["hi", "yo"]);
    }

    /// 미뤄 둔 delta가 **자리를 옮겨 가며** 편집해도 어긋나지 않는지 — pivot 정리 경로.
    #[test]
    fn edits_that_jump_around_stay_consistent() {
        let src = b"alpha\nbravo\ncharlie\ndelta\necho\nfoxtrot";
        let (mut buf, mut ix) = (src.to_vec(), LineIndex::build(src));
        apply(&mut buf, &mut ix, 30, 0, b"..");   // 뒤쪽
        apply(&mut buf, &mut ix, 2, 0, b"!!");    // 앞쪽으로 점프
        apply(&mut buf, &mut ix, 3, 0, b"?");     // 바로 옆에서 이어 치기
        apply(&mut buf, &mut ix, 20, 3, b"\n\n"); // 가운데를 여러 줄로
        apply(&mut buf, &mut ix, 0, 1, b"");      // 맨 앞 지우기
        assert_eq!(ix.total(), buf.len() as u64);
    }

    /// 무작위 편집을 길게 이어 붙여 재계산과 계속 같은지 본다 — 손으로 못 세는 경계들.
    #[test]
    fn a_long_run_of_random_edits_matches_a_full_rebuild() {
        let mut buf = b"the quick\nbrown fox\njumps over\nthe lazy dog\n".to_vec();
        let mut ix = LineIndex::build(&buf);
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..400 {
            let at = (next() as usize) % (buf.len() + 1);
            // 문서가 너무 짧아지면 여러 줄 경계를 더 못 밟으므로, 짧을 때는 넣기만 한다.
            let room = if buf.len() < 24 { 0 } else { (buf.len() - at).min(9) };
            let del = if room == 0 { 0 } else { (next() as usize) % (room + 1) };
            let ins: Vec<u8> = match next() % 4 {
                0 => b"\n".to_vec(),
                1 => b"xy\nz".to_vec(),
                2 => Vec::new(),
                _ => b"abc".to_vec(),
            };
            apply(&mut buf, &mut ix, at, del, &ins);
        }
        assert!(ix.lines() > 3, "여러 줄 문서를 유지한 채 끝나야 경계를 실제로 밟은 것이다");
        assert_eq!(ix.lines(), LineIndex::build(&buf).lines());
    }
}
