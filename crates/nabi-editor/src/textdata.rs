//! 용량 제한 없는 **텍스트 문서** — 바이트는 조각 표(mmap), 줄은 인덱스.
//!
//! [`crate::hexdata::HexData`](바이트)와 [`crate::textindex::LineIndex`](줄)를 묶은 것이 전부다.
//! 문서를 통째로 RAM에 올리지 않으므로 `editbuf::EDIT_CAP`(512MB) 같은 상한이 필요 없다.
//!
//! ## 개행을 정규화하지 않는다
//!
//! rope 편집기는 열 때 CRLF를 LF로 바꿔 두고 저장할 때 되돌린다. 그러려면 문서 전체를 한 번
//! 훑어 새 문자열을 만들어야 하는데, 그 순간 "파일을 통째로 올리지 않는다"는 전제가 무너진다.
//!
//! 그래서 여기서는 **원본 바이트를 그대로 둔다.** 줄 인덱스는 `\n` 다음 자리만 기억하므로
//! CRLF에서도 그대로 맞고, 줄을 꺼낼 때 끝의 `\r`만 떼면 된다. 새로 치는 개행은 문서의
//! 원래 EOL로 넣어, 파일 안에서 줄 끝이 섞이지 않게 한다.

use crate::hexdata::HexData;
use crate::textindex::LineIndex;
use std::path::Path;

/// 조각 표 + 줄 인덱스로 이루어진 텍스트 문서.
pub struct TextData {
    data: HexData,
    index: LineIndex,
    enc: &'static encoding_rs::Encoding,
    /// 원본 줄 끝("CRLF" / "LF" / "CR"). 새로 넣는 개행에 쓴다.
    pub eol: &'static str,
}

/// 문서 앞부분을 보고 줄 끝 종류를 정한다 — **규칙은 `eolmix` 한 곳에만 있다**(배치 AE).
///
/// 예전에는 여기서 "첫 개행이 정한다"였고 일반 경로(`editload`)는 "어디든 CRLF 가 하나라도
/// 있으면 CRLF"였다. 같은 파일에 대해 **답이 달랐다** — LF 로 시작해 중간에 CRLF 가 섞인
/// 파일을 여기서는 LF, 저기서는 CRLF 로 읽었다. 그러면 같은 파일을 어느 편집기로 여느냐에
/// 따라 Enter 가 **다른 줄바꿈을 넣는다.** 파일 내용이 달라지는 차이다.
///
/// 여기서는 **받은 앞부분 안에서만** 센다. 전체를 훑는 것은 이 편집기가 하지 않기로 한
/// 일이고, 그 사실은 상태바가 밝힌다.
fn detect_eol(head: &[u8]) -> &'static str {
    // 앞부분이 글자 가운데서 잘렸을 수 있다. 줄 끝만 세므로 깨진 조각은 그냥 건너뛴다.
    crate::eolmix::count_eols(&String::from_utf8_lossy(head)).dominant()
}

impl TextData {
    /// 메모리에 있는 바이트로 문서를 만든다(작은 파일·시험).
    pub fn from_vec(v: Vec<u8>) -> Self {
        let enc = crate::editload::detect_encoding(&v[..v.len().min(64 * 1024)]);
        let eol = detect_eol(&v[..v.len().min(64 * 1024)]);
        let index = LineIndex::build(&v);
        Self { data: HexData::from_vec(v), index, enc, eol }
    }

    /// 파일을 매핑해 연다. **파일 크기만큼 메모리를 쓰지 않는다** — 줄 인덱스만 세운다.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let data = HexData::map_file(path)?;
        let head = data.read(0, 64 * 1024);
        let (enc, eol) = (crate::editload::detect_encoding(&head), detect_eol(&head));
        if eol == "CR" {
            // 줄 인덱스는 개행(LF)만 센다. CR만으로 줄을 나눈 문서(OS X 이전 Mac)는 통째로
            // 한 줄이 되어 버린다. 못 다루면서 다루는 척하느니 거절하고 다른 경로로 보낸다.
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "CR-only line endings are not supported by the streaming editor",
            ));
        }
        let index = Self::build_index(&data);
        Ok(Self { data, index, enc, eol })
    }

    /// 조각을 훑어 줄 시작을 모은다. 조각 경계가 개행을 가르지 않으므로 이어 붙일 필요가 없다
    /// (개행은 한 바이트라 어느 한 조각 안에 온전히 들어간다).
    fn build_index(data: &HexData) -> LineIndex {
        let mut starts = vec![0u64];
        data.scan_chunks(|base, buf| {
            for i in memchr::memchr_iter(b'\n', buf) {
                starts.push((base + i + 1) as u64);
            }
        });
        // 파일이 개행으로 끝나면 `starts` 마지막 항목이 문서 끝과 같아지고, 그것이 곧
        // 마지막 빈 줄이다(편집기 관행 — ropey도 같다).
        LineIndex::from_starts(starts, data.len() as u64)
    }

    pub fn lines(&self) -> usize {
        self.index.lines()
    }

    pub fn total(&self) -> u64 {
        self.index.total()
    }

    pub fn encoding(&self) -> &'static str {
        self.enc.name()
    }

    pub fn is_empty(&self) -> bool {
        self.index.total() == 0
    }

    /// 편집이 얼마나 쌓였는지(진단·시험용).
    pub fn piece_count(&self) -> usize {
        self.data.piece_count()
    }

    /// `line`번째 줄의 바이트 범위(개행 제외 — 화면에 그릴 부분).
    pub fn line_range(&self, line: usize) -> (u64, u64) {
        let (a, mut b) = (self.index.start(line), self.index.end(line));
        if b > a && self.data.get(b as usize - 1) == Some(b'\n') {
            b -= 1;
        }
        if b > a && self.data.get(b as usize - 1) == Some(b'\r') {
            b -= 1;
        }
        (a, b)
    }

    /// `line`번째 줄의 끝 — **개행을 포함**한다(줄 전체가 선택됐는지 판정할 때).
    pub fn line_end_with_break(&self, line: usize) -> u64 {
        self.index.end(line)
    }

    /// `line`번째 줄을 문자열로 꺼낸다(개행 제외). 화면에 보이는 줄만 부르는 것이 전제다.
    pub fn line(&self, line: usize) -> String {
        let (a, b) = self.line_range(line);
        if b <= a {
            return String::new();
        }
        let raw = self.data.read(a as usize, (b - a) as usize);
        self.enc.decode(&raw).0.into_owned()
    }

    /// 바이트를 이 문서의 인코딩으로 디코드한다(선택 복사 등).
    ///
    /// **BOM을 다시 해석하지 않는다.** `decode`는 앞머리의 U+FEFF를 스트림 BOM으로 보고
    /// 삼켜 버리는데, 우리는 줄 단위로 부르므로 둘째 줄 첫 글자가 U+FEFF이면 그 글자가
    /// 조용히 사라진다(교차 검토 2026-08-25).
    pub fn decode(&self, raw: &[u8]) -> String {
        self.enc.decode_without_bom_handling(raw).0.into_owned()
    }

    /// 바이트를 이 문서의 인코딩으로 디코드했을 때의 **표시 문자 수**.
    ///
    /// 커서 열은 바이트가 아니라 글자로 세야 화면과 맞는다("가"는 UTF-8에서 3바이트다).
    pub fn decode_len(&self, raw: &[u8]) -> usize {
        self.enc.decode(raw).0.chars().count()
    }

    /// `line`번째 줄에서 표시 열 `col`에 해당하는 바이트 오프셋.
    ///
    /// 줄이 짧으면 줄 끝을 준다 — 위아래로 움직일 때 짧은 줄을 지나도 넘치지 않게.
    pub fn offset_of_col(&self, line: usize, col: usize) -> u64 {
        let starts = self.char_starts(line);
        *starts.get(col).unwrap_or_else(|| starts.last().unwrap_or(&0))
    }

    /// 오프셋이 속한 줄 번호.
    pub fn line_of(&self, off: u64) -> usize {
        self.index.line_of(off)
    }

    pub fn line_start(&self, line: usize) -> u64 {
        self.index.start(line)
    }

    /// 문서의 EOL에 해당하는 바이트("\r\n" 등) — 새 줄을 넣을 때 쓴다.
    pub fn eol_bytes(&self) -> &'static [u8] {
        match self.eol {
            "CRLF" => b"\r\n",
            "CR" => b"\r",
            _ => b"\n",
        }
    }

    /// 문자열을 이 문서의 인코딩으로 바꾼다. 이 인코딩으로 못 적는 글자가 있으면 None.
    ///
    /// `encoding_rs`는 표현할 수 없는 글자를 HTML 참조(`&#128512;`)로 바꿔 버린다. 그대로
    /// 넣으면 CP949 문서에 이모지를 치는 순간 문서에 `&#128512;`라는 **글자들이** 박힌다
    /// (교차 검토 2026-08-25). 조용히 다른 내용을 쓰느니 넣지 않는 편이 낫다.
    pub fn encode(&self, s: &str) -> Option<Vec<u8>> {
        let (out, _, bad) = self.enc.encode(s);
        (!bad).then(|| out.into_owned())
    }

    /// 줄 안에서 각 글자가 시작하는 바이트 오프셋(줄 끝 포함, 개행 제외).
    ///
    /// 커서 이동·삭제는 **전부 이걸로만** 한다. 예전에는 UTF-8 이어짐 바이트(`0b10xxxxxx`)를
    /// 세어 글자 경계를 찾았는데, CP949는 그 패턴에 걸리는 바이트가 널려 있다 — `가`(B0 A1)의
    /// `A1`이 그렇다. 그래서 한글 문서에서 오른쪽 화살표 한 번이 파일 끝까지 훑었다
    /// (교차 검토 2026-08-25). 줄 안에서만 재면 인코딩이 무엇이든 맞고, 줄 길이로 묶인다.
    pub fn char_starts(&self, line: usize) -> Vec<u64> {
        let (a, b) = self.line_range(line);
        let raw = self.read(a, (b - a) as usize);
        let text = self.decode(&raw);
        let mut out = Vec::with_capacity(text.len() + 1);
        let mut at = a;
        for ch in text.chars() {
            out.push(at);
            // 한 글자씩 되돌려 그 바이트 수를 쓴다. CP949·Shift_JIS 같은 바이트 인코딩은
            // 글자마다 길이가 정해져 있어 이 방법이 맞다.
            at += self.enc.encode(ch.encode_utf8(&mut [0u8; 4])).0.len() as u64;
            if at > b {
                // 되돌리기가 원본과 어긋났다(깨진 바이트 등) — 한 바이트씩 세는 쪽이 안전하다.
                return (a..=b).collect();
            }
        }
        out.push(b);
        if out.last() != Some(&b) || at != b {
            return (a..=b).collect();
        }
        out
    }

    /// `[at, at+del)`을 `ins`로 바꾼다. 바이트와 줄 인덱스를 함께 맞춘다.
    ///
    /// 인덱스는 편집 **전** 좌표로 영향 구간을 정한 뒤, 바이트를 고치고 나서 그 구간을 다시
    /// 읽어 맞춘다. 순서가 뒤바뀌면 좌표가 어긋난다.
    pub fn splice(&mut self, at: u64, del: u64, ins: &[u8]) {
        let at = at.min(self.index.total());
        let del = del.min(self.index.total() - at);
        let (rs, re) = self.index.edit_region(at, del);
        self.data.splice(at as usize, del as usize, ins);
        let new_re = (re as i64 + ins.len() as i64 - del as i64).max(rs as i64) as u64;
        let region = self.data.read(rs as usize, (new_re - rs) as usize);
        self.index.patch(at, del, ins.len() as u64, rs, &region);
    }

    /// `[at, at+n)` 바이트를 그대로 꺼낸다(찾기·복사 등 **작은 구간**).
    pub fn read(&self, at: u64, n: usize) -> Vec<u8> {
        self.data.read(at as usize, n)
    }

    /// `from`부터 바이트 열이 처음 나오는 위치.
    pub fn find(&self, needle: &[u8], from: u64) -> Option<u64> {
        self.data.find(needle, from as usize).map(|i| i as u64)
    }

    /// 전체를 순서대로 흘려 쓴다(저장) — 문서를 메모리에 모으지 않는다.
    pub fn write_to(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        self.data.write_to(w)
    }

    /// 전부를 한 문자열로 — **작은 문서에서만** 쓴다(시험·클립보드).
    pub fn to_string_lossy(&self) -> String {
        self.enc.decode(&self.data.to_vec()).0.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> TextData {
        TextData::from_vec(s.as_bytes().to_vec())
    }

    /// 모든 줄을 꺼내 본다 — 인덱스와 바이트가 같은 문서를 말하는지 통째로 비교하려고.
    fn all(d: &TextData) -> Vec<String> {
        (0..d.lines()).map(|i| d.line(i)).collect()
    }

    #[test]
    fn lines_come_back_without_their_newline() {
        assert_eq!(all(&doc("one\ntwo\nthree")), vec!["one", "two", "three"]);
    }

    #[test]
    fn a_crlf_file_keeps_its_bytes_but_hides_the_carriage_return() {
        let d = doc("a\r\nb\r\n");
        assert_eq!(d.eol, "CRLF");
        assert_eq!(all(&d), vec!["a", "b", ""]);
        assert_eq!(d.total(), 6); // 원본 바이트는 그대로다 — 정규화하지 않는다.
    }

    #[test]
    fn typing_updates_both_the_bytes_and_the_lines() {
        let mut d = doc("aa\nbb\ncc");
        d.splice(1, 0, b"XY");
        assert_eq!(all(&d), vec!["aXYa", "bb", "cc"]);
        assert_eq!(d.to_string_lossy(), "aXYa\nbb\ncc");
    }

    #[test]
    fn a_new_line_uses_the_documents_own_eol() {
        let mut d = doc("a\r\nb");
        let (at, nl) = (3, d.eol_bytes().to_vec());
        d.splice(at, 0, &nl); // "a\r\n" 뒤 = 둘째 줄 앞.
        assert_eq!(all(&d), vec!["a", "", "b"]);
        assert_eq!(d.to_string_lossy(), "a\r\n\r\nb");
    }

    #[test]
    fn deleting_across_lines_joins_them() {
        let mut d = doc("one\ntwo\nthree");
        d.splice(2, 6, b""); // 둘째 줄 전체와 앞뒤를 걸쳐 지운다.
        assert_eq!(all(&d), vec!["onthree"]);
    }

    #[test]
    fn line_of_finds_the_line_holding_an_offset() {
        let d = doc("ab\ncd\nef");
        assert_eq!((d.line_of(0), d.line_of(3), d.line_of(7)), (0, 1, 2));
    }

    #[test]
    fn find_locates_bytes_across_piece_boundaries() {
        let mut d = doc("hello world");
        d.splice(5, 0, b"XX"); // 조각을 셋으로 쪼갠다.
        assert_eq!(d.find(b"XX wor", 0), Some(5));
        assert_eq!(d.find(b"nope", 0), None);
    }

    #[test]
    fn a_korean_cp949_file_decodes_per_line() {
        // "가\n나" (CP949). 줄 인덱스는 바이트로 세고, 표시는 줄 단위로 디코드한다.
        let d = TextData::from_vec(vec![0xB0, 0xA1, b'\n', 0xB3, 0xAA]);
        assert_eq!(d.lines(), 2);
        assert_eq!(all(&d), vec!["가", "나"]);
    }

    #[test]
    fn saving_streams_the_document_back_out_unchanged() {
        let mut d = doc("keep\nthis\n");
        d.splice(5, 0, b"or ");
        let mut out = Vec::new();
        d.write_to(&mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "keep\nor this\n");
    }

    #[test]
    fn an_empty_document_has_one_line() {
        let d = doc("");
        assert_eq!((d.lines(), d.total()), (1, 0));
        assert_eq!(all(&d), vec![""]);
    }

    /// 편집을 많이 해도 조각이 무한정 늘지 않는지 — 이어 치기는 합쳐져야 한다.
    #[test]
    fn typing_in_one_place_does_not_pile_up_pieces() {
        let mut d = doc("start\nend");
        for i in 0..200u64 {
            d.splice(5 + i, 0, b"x");
        }
        assert!(d.piece_count() < 8, "조각이 {}개나 됨", d.piece_count());
        assert_eq!(d.lines(), 2);
    }

    /// **큰 파일을 열고·고치고·저장하는 전 과정**이 메모리를 문서 크기만큼 쓰지 않는지.
    ///
    /// 이 편집기의 존재 이유가 그것 하나다. 200MB 파일에서 조각 수가 몇 개 늘 뿐이어야 하고,
    /// 저장도 흘려 쓰기라 결과 파일이 정확히 같아야 한다.
    #[test]
    fn a_big_file_opens_edits_and_saves_without_loading_it() {
        let p = std::env::temp_dir().join("nabi-textdata-big.txt");
        {
            use std::io::Write;
            let f = std::fs::File::create(&p).unwrap();
            let mut w = std::io::BufWriter::new(f);
            for i in 0..2_000_000u32 {
                writeln!(w, "line {i} of a rather long log file").unwrap();
            }
            w.flush().unwrap();
        }
        let size = std::fs::metadata(&p).unwrap().len();
        assert!(size > 60_000_000, "시험 파일이 충분히 커야 의미가 있다: {size}");

        let mut d = TextData::open(&p).unwrap();
        assert_eq!(d.lines(), 2_000_001); // 끝 개행이 만든 마지막 빈 줄 포함.
        assert_eq!(d.line(0), "line 0 of a rather long log file");
        assert_eq!(d.line(1_999_999), "line 1999999 of a rather long log file");

        // 한가운데를 고쳐도 조각이 몇 개 늘 뿐이다.
        let at = d.line_start(1_000_000);
        d.splice(at, 0, b"EDITED ");
        assert_eq!(d.line(1_000_000), "EDITED line 1000000 of a rather long log file");
        assert!(d.piece_count() < 8, "조각이 {}개나 됨", d.piece_count());
        assert_eq!(d.total(), size + 7);

        let out = std::env::temp_dir().join("nabi-textdata-big-out.txt");
        {
            use std::io::Write;
            let f = std::fs::File::create(&out).unwrap();
            let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
            d.write_to(&mut w).unwrap();
            w.flush().unwrap();
        }
        assert_eq!(std::fs::metadata(&out).unwrap().len(), size + 7);
        // 고친 줄이 저장 결과에도 그대로 있는지 다시 열어 확인한다.
        let back = TextData::open(&out).unwrap();
        assert_eq!(back.lines(), d.lines());
        assert_eq!(back.line(1_000_000), "EDITED line 1000000 of a rather long log file");
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&out);
    }

    /// 파일에서 열어도 메모리 문서와 똑같이 동작하는지 — mmap 경로.
    #[test]
    fn a_file_opens_by_mapping_and_edits_the_same_way() {
        let p = std::env::temp_dir().join("nabi-textdata-open.txt");
        std::fs::write(&p, b"alpha\nbravo\ncharlie\n").unwrap();
        let mut d = TextData::open(&p).unwrap();
        assert_eq!(all(&d), vec!["alpha", "bravo", "charlie", ""]);
        d.splice(6, 5, b"BRAVO!");
        assert_eq!(all(&d), vec!["alpha", "BRAVO!", "charlie", ""]);
        let _ = std::fs::remove_file(&p);
    }
}
