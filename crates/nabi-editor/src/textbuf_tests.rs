//! `textbuf` 의 시험 — 파일이 소프트 한도를 넘어 갈라 냈다(배치 AG).
//!
//! 시험을 옮긴 이유: 한도를 맞추려고 **설명을 지우지 않는다**는 규칙이 있어서, 줄여야 할
//! 것은 코드 쪽이다. 시험은 그 자체로 한 덩어리라 통째로 옮기기 좋다.

use super::*;
use crate::textdata::TextData;


    fn buf(s: &str) -> TextBuf {
        TextBuf::new(TextData::from_vec(s.as_bytes().to_vec()))
    }





    #[test]
    fn arrow_keys_step_over_whole_characters_not_bytes() {
        let mut b = buf("가나");
        b.step(true, false);
        assert_eq!(b.caret, 3);
        b.step(true, false);
        assert_eq!(b.caret, 6);
        b.step(false, false);
        assert_eq!(b.caret, 3);
    }




    #[test]
    fn moving_down_keeps_the_column_across_a_short_line() {
        let mut b = buf("long line\nx\nlong line");
        b.go(7, false); // 첫 줄 7열.
        assert_eq!(b.caret_col(), 7);
        b.step_line(true, false);
        assert_eq!(b.caret_col(), 1); // 짧은 줄에서는 끝까지만.
        b.step_line(true, false);
        assert_eq!(b.caret_col(), 7); // 원래 열로 돌아온다.
    }

    #[test]
    fn home_and_end_go_to_the_edges_of_the_line() {
        let mut b = buf("aa\nbbbb\ncc");
        b.go(5, false);
        b.go_line_edge(false, false);
        assert_eq!(b.caret, 3);
        b.go_line_edge(true, false);
        assert_eq!(b.caret, 7);
    }
    // --- 아래는 codex 교차 검토(2026-08-25)가 찾아낸 결함들의 회귀 시험이다. ---

    /// 커서가 CRLF **사이**에 서면 안 된다 — 백스페이스가 CR만 지워 문서가 망가진다.
    #[test]
    fn the_caret_never_stops_between_cr_and_lf() {
        let mut b = buf("a\r\nb");
        b.go(1, false);
        b.step(true, false);
        assert_eq!(b.caret, 3, "CRLF를 한 걸음으로 넘어야 한다");
        b.step(false, false);
        assert_eq!(b.caret, 1);
    }

    /// CP949 한 글자는 두 바이트다. 오른쪽 화살표 한 번에 한 글자만 지나야 한다.
    ///
    /// 옛 코드는 UTF-8 이어짐 바이트(0b10xxxxxx)를 세어 경계를 찾았는데, `가`(B0 A1)의 A1이
    /// 그 패턴에 걸린다. 그래서 한글 문서에서 화살표 한 번이 파일 끝까지 훑었다.
    #[test]
    fn one_arrow_key_crosses_exactly_one_cp949_character() {
        // "가나다"(CP949) — 6바이트.
        let d = crate::textdata::TextData::from_vec(vec![0xB0, 0xA1, 0xB3, 0xAA, 0xB4, 0xD9]);
        let mut b = TextBuf::new(d);
        for want in [2u64, 4, 6] {
            b.step(true, false);
            assert_eq!(b.caret, want, "CP949 한 글자씩 지나야 한다");
        }
        b.step(true, false);
        assert_eq!(b.caret, 6, "끝을 넘지 않는다");
    }

    #[test]
    fn selected_text_comes_back_decoded() {
        let mut b = buf("가나다");
        b.go(3, false);
        b.go(9, true);
        assert_eq!(b.selected_text(), "나다");
    }

    #[test]
    fn going_to_a_line_moves_the_caret_and_scrolls() {
        let mut b = buf("첫째
둘째
셋째
넷째");
        b.go_to_line(2, None);
        assert_eq!(b.caret_line(), 2, "셋째 줄");
        assert_eq!(b.caret_col(), 0);
        assert!(b.scroll_to.is_some(), "화면도 따라가야 한다 — 커서만 옮기면 안 보인다");
    }

    #[test]
    fn a_line_past_the_end_lands_on_the_last_line() {
        // 아무 일도 안 하면 사용자는 자기가 잘못 눌렀는지 무시당했는지 알 수 없다.
        let mut b = buf("하나
둘
셋");
        b.go_to_line(999, None);
        assert_eq!(b.caret_line(), 2, "마지막 줄");
        // 커서만 보면 이 시험은 방어를 빼도 통과한다 — `go()` 가 이미 총 길이로 자르기
        // 때문이다(일부러 깨서 확인했다). 정작 잘라야 하는 것은 **스크롤**이다. 안 자르면
        // 화면이 문서 끝을 한참 지나쳐 빈 곳을 보여 준다.
        assert_eq!(b.scroll_to, Some(0), "화면도 문서 안에 머물러야 한다");
    }

    #[test]
    fn a_column_is_counted_in_characters() {
        // 바이트로 세면 한글 줄에서 커서가 글자 가운데에 떨어진다.
        let mut b = buf("가나다라
다음");
        b.go_to_line(0, Some(2));
        assert_eq!(b.caret_col(), 2);
        assert_eq!(b.caret, 6, "두 글자 = 6바이트");
    }

    #[test]
    fn the_view_starts_two_lines_above_so_context_is_visible() {
        let mut b = buf("1
2
3
4
5
6
7
8");
        b.go_to_line(5, None);
        assert_eq!(b.scroll_to, Some(3), "찾던 줄이 맨 위에 딱 붙으면 앞 맥락이 안 보인다");
    }

    #[test]
    fn near_the_top_the_view_does_not_go_negative() {
        let mut b = buf("1
2
3");
        b.go_to_line(1, None);
        assert_eq!(b.scroll_to, Some(0));
    }

    #[test]
    fn a_found_range_is_selected_not_just_pointed_at() {
        // 커서만 옮기면 사용자가 무엇이 걸렸는지 눈으로 확인해야 한다. 선택돼 있으면
        // 바로 복사하거나 덮어쓸 수 있다.
        let mut b = buf("port 22
port 80");
        b.select_range(8, 12);
        assert!(b.has_selection());
        assert_eq!(b.selected_text(), "port");
        assert!(b.scroll_to.is_some(), "화면도 그리로 가야 한다");
    }

    #[test]
    fn selecting_a_range_in_a_hangul_document_keeps_the_bytes() {
        let mut b = buf("앞줄
포트 설정");
        // "포트" 는 앞줄(6바이트) + 개행(1) 다음 6바이트.
        b.select_range(7, 13);
        assert_eq!(b.selected_text(), "포트");
    }


