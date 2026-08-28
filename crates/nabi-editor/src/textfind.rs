//! 초대용량 파일에서 **다음 것을 찾는다**(배치 AG).
//!
//! `TextData::find` 는 이미 있었다 — HEX 편집기가 쓰고 있었고, 텍스트 쪽에서는 **아무도
//! 부르지 않았다.** 수 GB 로그를 열어 두고 문자열 하나를 못 찾는 편집기였던 셈이다.
//!
//! ## 전부 찾지 않고 다음 것만 찾는다
//!
//! 일치를 전부 모으려면 문서 전체를 훑어야 하고, 그것은 이 편집기가 하지 않기로 한 일이다
//! (`textview` 헤더). 대신 **커서 다음 하나**를 찾는다 — 사람이 찾기를 쓰는 방식이 원래
//! 그렇고, 그 답은 문서가 아무리 커도 첫 일치까지만 읽으면 나온다.
//!
//! 그래서 "3/512 번째" 같은 숫자는 보여 주지 않는다. 그 숫자를 보여 주려면 전부 세어야 하고,
//! **셀 수 없는 것을 센 척하지 않는다.**
//!
//! ## 인코딩
//!
//! 찾을 말은 사용자가 UTF-8 로 친다. 문서는 CP949 일 수 있다. 그래서 **문서 인코딩으로
//! 바꿔서** 찾는다 — 안 그러면 한국 서버의 CP949 로그에서 한글이 영원히 안 찾힌다
//! (배치 AD 에서 원격 내용 찾기가 정확히 그 결함이었다).

use crate::textdata::TextData;

/// 어느 방향으로 찾는가.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Next,
    Prev,
}

/// 찾은 자리 — `(시작 바이트, 길이)`.
pub type Hit = (u64, usize);

/// 질의를 문서 인코딩의 바이트로 바꾼다. 그 인코딩으로 적을 수 없으면 `None`.
///
/// 예를 들어 CP949 문서에서 이모지를 찾는 것은 **없는 것을 찾는 일**이다. 그때는 "못 찾음"이
/// 아니라 "이 문서에는 있을 수 없음"이라고 말할 수 있어야 한다.
pub fn needle_for(data: &TextData, query: &str) -> Option<Vec<u8>> {
    (!query.is_empty()).then(|| data.encode(query)).flatten()
}

/// `from` 다음(또는 이전)의 일치 하나. 끝에 닿으면 반대쪽 끝에서 이어 찾는다(감싸기).
///
/// 감싸기를 하는 이유: 찾기는 대개 "어딘가에 있나"를 묻는 일이라, 커서 뒤에만 있다고
/// 없다고 답하면 사용자는 파일 맨 앞으로 스스로 가야 한다.
pub fn find_from(data: &TextData, needle: &[u8], from: u64, dir: Dir) -> Option<Hit> {
    if needle.is_empty() {
        return None;
    }
    let n = needle.len();
    match dir {
        Dir::Next => data
            .find(needle, from)
            .or_else(|| data.find(needle, 0))
            .map(|p| (p, n)),
        // 뒤로 찾기는 처음부터 훑어 `from` 앞의 마지막 것을 고른다. 앞쪽에 없으면 감싸서
        // 문서 전체의 마지막 것을 준다.
        Dir::Prev => {
            let before = last_before(data, needle, from);
            before.or_else(|| last_before(data, needle, data.total())).map(|p| (p, n))
        }
    }
}

/// `limit` 앞에 있는 마지막 일치.
///
/// 앞에서부터 훑는다. 뒤에서부터 읽는 방법이 더 빨라 보이지만, 조각(piece) 경계를 거슬러
/// 읽는 코드를 따로 두면 **앞으로 찾기와 다른 답을 낼 자리**가 하나 더 생긴다.
fn last_before(data: &TextData, needle: &[u8], limit: u64) -> Option<u64> {
    let (mut at, mut best) = (0u64, None);
    while let Some(p) = data.find(needle, at) {
        if p >= limit {
            break;
        }
        best = Some(p);
        at = p + 1;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textdata::TextData;

    fn doc(s: &str) -> TextData {
        TextData::from_vec(s.as_bytes().to_vec())
    }

    #[test]
    fn finds_the_next_one_after_the_caret() {
        let d = doc("port 22\nport 80\nport 443");
        let n = needle_for(&d, "port").unwrap();
        assert_eq!(find_from(&d, &n, 0, Dir::Next), Some((0, 4)));
        assert_eq!(find_from(&d, &n, 1, Dir::Next), Some((8, 4)));
    }

    #[test]
    fn it_wraps_around_the_end() {
        // 커서 뒤에만 없다고 "없다"고 답하면 사용자가 스스로 맨 앞으로 가야 한다.
        let d = doc("port 22\nnothing here");
        let n = needle_for(&d, "port").unwrap();
        assert_eq!(find_from(&d, &n, 10, Dir::Next), Some((0, 4)), "끝에 닿으면 앞에서 이어 찾는다");
    }

    #[test]
    fn backwards_finds_the_last_one_before_the_caret() {
        let d = doc("port 22\nport 80\nport 443");
        let n = needle_for(&d, "port").unwrap();
        assert_eq!(find_from(&d, &n, 20, Dir::Prev), Some((16, 4)));
        assert_eq!(find_from(&d, &n, 16, Dir::Prev), Some((8, 4)));
    }

    #[test]
    fn backwards_wraps_to_the_last_one_in_the_document() {
        let d = doc("port 22\nport 80");
        let n = needle_for(&d, "port").unwrap();
        assert_eq!(find_from(&d, &n, 0, Dir::Prev), Some((8, 4)), "앞에 없으면 감싸서 마지막");
    }

    #[test]
    fn a_missing_word_is_not_found() {
        let d = doc("port 22");
        let n = needle_for(&d, "socket").unwrap();
        assert_eq!(find_from(&d, &n, 0, Dir::Next), None);
        assert_eq!(find_from(&d, &n, 0, Dir::Prev), None);
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let d = doc("port 22");
        assert!(needle_for(&d, "").is_none(), "빈 질의로 문서를 훑게 하지 않는다");
    }

    #[test]
    fn a_korean_query_finds_korean_in_a_cp949_document() {
        // 배치 AD 에서 원격 내용 찾기가 정확히 이 결함이었다 — 질의를 문서 인코딩으로
        // 바꾸지 않으면 한국 서버의 CP949 로그에서 한글이 영원히 안 찾힌다.
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("앞줄\n포트 설정\n뒷줄");
        let d = TextData::from_vec(bytes.into_owned());
        let n = needle_for(&d, "포트").expect("CP949 로 적을 수 있는 말이다");
        let hit = find_from(&d, &n, 0, Dir::Next).expect("찾아야 한다");
        assert_eq!(d.read(hit.0, hit.1), n, "찾은 자리의 바이트가 질의와 같아야 한다");
    }

    #[test]
    fn a_word_the_document_encoding_cannot_hold_is_refused() {
        // CP949 문서에서 이모지를 찾는 것은 없는 것을 찾는 일이다. "못 찾음"과 구분한다.
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("한글 문서");
        let d = TextData::from_vec(bytes.into_owned());
        assert!(needle_for(&d, "\u{1f600}").is_none());
    }
}
