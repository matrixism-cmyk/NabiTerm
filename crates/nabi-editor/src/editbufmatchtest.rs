//! `editbufmatch`의 시험 — 본체가 소프트 라인 한도에 닿아 분리했다.
//!
//! 시험을 줄이는 대신 파일을 나눈다. 이 시험들은 하나하나가 겪은 실패의 기록이라
//! (겹치는 일치를 세 번 세던 버릇, 주 범위에만 쓰던 insert) 지울 것이 없다.

mod tests {
    use crate::editbuf::EditBuf;

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "\n")
    }

    /// 아무것도 안 골랐을 때 첫 Ctrl+D는 **낱말을 잡는다**(VS Code와 같다).
    #[test]
    fn the_first_press_selects_the_word_under_the_caret() {
        let mut b = buf("alpha beta alpha");
        b.set_cursor(2); // alpha 안
        assert!(b.add_next_match());
        assert_eq!(b.sel.len(), 1);
        assert_eq!(b.selected_text(), "alpha");
    }

    /// 낱말 오른쪽 끝에 커서가 붙어 있어도 그 낱말로 본다.
    #[test]
    fn a_caret_just_after_a_word_still_finds_it() {
        let mut b = buf("alpha beta");
        b.set_cursor(5); // "alpha" 바로 뒤
        assert!(b.add_next_match());
        assert_eq!(b.selected_text(), "alpha");
    }

    #[test]
    fn a_caret_in_whitespace_finds_nothing() {
        let mut b = buf("  \n  ");
        b.set_cursor(1);
        assert!(!b.add_next_match(), "공백에서는 잡을 낱말이 없다");
    }

    /// 두 번째 누르면 **다음 같은 낱말이 더 잡힌다** — 이게 이 기능의 본체다.
    #[test]
    fn the_second_press_adds_the_next_occurrence() {
        let mut b = buf("alpha beta alpha gamma alpha");
        b.set_cursor(0);
        assert!(b.add_next_match()); // alpha 하나
        assert!(b.add_next_match()); // 둘
        assert_eq!(b.sel.len(), 2);
        assert!(b.add_next_match()); // 셋
        assert_eq!(b.sel.len(), 3);
    }

    /// **더 없으면 조용히 멈춘다.** 처음으로 감싸 돌면 이미 잡은 것을 다시 잡아
    /// 개수가 줄어든 것처럼 보인다(병합되므로).
    #[test]
    fn running_out_of_matches_stops_instead_of_wrapping() {
        let mut b = buf("alpha beta alpha");
        b.set_cursor(0);
        b.add_next_match();
        b.add_next_match();
        assert_eq!(b.sel.len(), 2);
        assert!(!b.add_next_match(), "더 없는데 true를 냈다");
        assert_eq!(b.sel.len(), 2, "감싸 돌며 개수가 흔들렸다");
    }

    #[test]
    fn all_matches_can_be_selected_at_once() {
        let mut b = buf("x = x + x");
        b.set_cursor(0);
        assert_eq!(b.select_all_matches(), 3);
        assert_eq!(b.sel.len(), 3);
    }

    /// 겹치는 자리는 선택 모델이 병합한다 — 개수가 뻥튀기되면 안 된다.
    #[test]
    fn overlapping_matches_do_not_multiply() {
        let mut b = buf("aaaa");
        b.sel = crate::editsel::Selection::single(0, 2); // "aa"
        let n = b.select_all_matches();
        assert!(n <= 2, "겹치는 aa를 세 번 세었다: {n}");
    }

    /// 한글도 낱말이다 — is_word가 alphanumeric이므로 잡혀야 한다.
    #[test]
    fn hangul_words_are_words_too() {
        let mut b = buf("나비 터미널 나비");
        b.set_cursor(0);
        assert!(b.add_next_match());
        assert_eq!(b.selected_text(), "나비");
        assert!(b.add_next_match());
        assert_eq!(b.sel.len(), 2);
    }

    /// **문자 인덱스여야 한다.** 바이트로 세면 한글 뒤의 자리가 전부 어긋난다.
    #[test]
    fn positions_are_char_indices_not_bytes() {
        let mut b = buf("가나다 target 라마바 target");
        let at = b.rope.to_string().chars().position(|_| false).unwrap_or(4);
        b.set_cursor(at);
        b.add_next_match();
        assert_eq!(b.selected_text(), "target", "바이트로 셌다면 여기가 깨진다");
        assert!(b.add_next_match());
        let starts: Vec<usize> = b.sel.ranges().iter().map(|r| r.start()).collect();
        for s in starts {
            let got: String = b.rope.slice(s..s + 6).to_string();
            assert_eq!(got, "target", "자리가 어긋났다");
        }
    }

    #[test]
    fn an_empty_document_is_harmless() {
        let mut b = buf("");
        assert!(!b.add_next_match());
        assert_eq!(b.select_all_matches(), 0);
    }
}

/// 다중 커서로 **고친 뒤에** 문서가 맞는지 — 잡는 것과 고치는 것은 별개의 위험이다.
///
/// 범위를 여럿 잡아 놓고 타자를 치면 앞 범위의 편집이 뒤 범위의 자리를 밀어낸다.
/// `editbufboxsel`이 "아래에서 위로 적용"으로 그 문제를 이미 풀어 놓았는데, 그 성질이
/// 일치 선택에도 그대로 통하는지는 **확인해 두지 않으면 조용히 깨진다.**
mod edit_tests {
    use crate::editbuf::EditBuf;

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "\n")
    }

    /// 모든 일치를 잡고 한 글자 치면 **모두** 바뀌어야 한다.
    #[test]
    fn typing_replaces_every_selected_occurrence() {
        let mut b = buf("cat dog cat dog cat");
        b.set_cursor(0);
        assert_eq!(b.select_all_matches(), 3);
        b.insert_multi("X");
        assert_eq!(b.rope.to_string(), "X dog X dog X");
    }

    /// **되돌리기 한 번으로 원래대로**여야 한다 — 커서 수만큼 눌러야 한다면 못 쓴다.
    #[test]
    fn one_undo_restores_everything() {
        let before = "cat dog cat";
        let mut b = buf(before);
        b.set_cursor(0);
        b.select_all_matches();
        b.insert_multi("X");
        assert_ne!(b.rope.to_string(), before);
        b.undo();
        assert_eq!(b.rope.to_string(), before, "되돌리기 한 번으로 안 돌아왔다");
    }

    /// 지우기도 모든 자리에서 일어나야 한다.
    #[test]
    fn deleting_removes_every_selected_occurrence() {
        let mut b = buf("a1 a2 a3");
        b.sel = crate::editsel::Selection::single(0, 1); // "a"
        b.select_all_matches();
        b.delete_multi(true);
        assert_eq!(b.rope.to_string(), "1 2 3");
    }

    /// **길이가 다른 글자로 바꿔도 자리가 안 밀려야 한다.** 여기가 가장 깨지기 쉽다 —
    /// 위에서 아래로 적용하면 두 번째부터 어긋난다.
    #[test]
    fn replacing_with_a_longer_word_keeps_later_positions_right() {
        let mut b = buf("x y x y x");
        b.set_cursor(0);
        b.select_all_matches();
        b.insert_multi("LONG");
        assert_eq!(b.rope.to_string(), "LONG y LONG y LONG");
    }

    /// 한글(멀티바이트)에서도 같아야 한다 — 바이트로 셌다면 여기서 깨진다.
    #[test]
    fn multibyte_text_survives_multi_cursor_editing() {
        let mut b = buf("가 나 가 나 가");
        b.set_cursor(0);
        assert_eq!(b.select_all_matches(), 3);
        b.insert_multi("다");
        assert_eq!(b.rope.to_string(), "다 나 다 나 다");
    }

    /// Esc로 멀티커서를 풀면 그다음 타자는 한 자리에서만 일어난다.
    #[test]
    fn collapsing_leaves_a_single_caret() {
        let mut b = buf("z z z");
        b.set_cursor(0);
        b.select_all_matches();
        b.sel.collapse_to_primary();
        assert_eq!(b.sel.len(), 1);
        b.insert_multi("Q");
        assert_eq!(b.rope.to_string().matches('Q').count(), 1);
    }

    /// **상한에 걸리면 그렇다고 표시한다** — 조용히 자르면 전부 잡힌 줄 알고 편집하게 되고,
    /// 나머지는 안 바뀐 채 남는다.
    #[test]
    fn hitting_the_cursor_cap_is_recorded() {
        // 한 글자를 잔뜩 깔아 상한을 넘긴다.
        let text = "a ".repeat(11_000);
        let mut b = EditBuf::new_buf(&text, "UTF-8".into(), "LF");
        b.set_cursor(0);
        let n = b.select_all_matches();
        assert!(n >= 10_000, "상한까지 못 갔다: {n}");
        assert!(b.match_capped, "끊겼는데 표시가 없다");
    }

    /// 상한에 안 걸리면 표시도 없다(멀쩡한 결과에 경고를 붙이지 않는다).
    #[test]
    fn a_small_document_is_not_marked_as_capped() {
        let mut b = EditBuf::new_buf("id x id", "UTF-8".into(), "LF");
        b.set_cursor(0);
        assert_eq!(b.select_all_matches(), 2);
        assert!(!b.match_capped);
    }
}
