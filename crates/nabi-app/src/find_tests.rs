//! `find` 의 시험 — 파일이 소프트 한도를 넘어 갈라 냈다(배치 AI).
//!
//! 한도를 맞추려고 설명을 지우지 않는다는 규칙이 있어서, 줄여야 할 것은 코드 쪽이다.
//! 시험은 그 자체로 한 덩어리라 통째로 옮기기 좋다(`textbuf_tests` 와 같은 이유).

use super::{build_matcher, FindKey};


    #[test]
    fn matcher_literal_and_regex() {
        let m = build_matcher("err", false, false).unwrap();
        assert!(m.is_match("an error here"));
        assert_eq!(m.count("err err"), 2);
        assert!(build_matcher("err", false, false).unwrap().is_match("ERR")); // 소문자→무시
        assert!(!build_matcher("ERR", false, false).unwrap().is_match("err")); // 대문자→구분
        let re = build_matcher("e.*r", true, false).unwrap();
        assert!(re.is_match("eXXr"));
        assert_eq!(re.count("eXr eYr"), 1); // 탐욕적 → 한 번
        assert!(build_matcher("(", true, false).is_none()); // 잘못된 정규식
        assert!(build_matcher("", false, false).is_none());
    }

    #[test]
    fn whole_word_matches_only_full_words() {
        let m = build_matcher("err", false, true).unwrap();
        assert!(m.is_match("an err here"));
        assert!(!m.is_match("error here")); // 단어 일부는 제외.
        // 리터럴은 escape되므로 정규식 메타문자가 그대로 글자로 취급된다.
        let dot = build_matcher("a.b", false, true).unwrap();
        assert!(dot.is_match("x a.b y"));
        assert!(!dot.is_match("x axb y"));
    }

    #[test]
    fn the_cache_key_reacts_to_every_input_that_changes_the_count() {
        // 이 시험이 이 타입이 생긴 이유다. 예전 열쇠는 (질의, 정규식) 뿐이라 단어 단위를
        // 켜도 개수가 그대로였다 — 화면이 틀린 말을 했다.
        let base = FindKey::new("port", false, false);
        assert_ne!(base, FindKey::new("PORT", false, false), "질의");
        assert_ne!(base, FindKey::new("port", true, false), "정규식");
        assert_ne!(base, FindKey::new("port", false, true), "단어 단위 — 예전에 빠뜨린 것");
        assert_eq!(base, FindKey::new("port", false, false), "같은 셋이면 같다");
    }

    #[test]
    fn the_key_takes_the_same_three_inputs_as_the_matcher() {
        // build_matcher 와 같은 셋을 받는다. 넷째가 생기면 컴파일러가 두 곳을 함께 짚는다.
        let (q, re, whole) = ("a.b", true, false);
        let _ = build_matcher(q, re, whole);
        let _ = FindKey::new(q, re, whole);
    }

    #[test]
    fn whole_word_really_changes_the_count() {
        // 열쇠만 고치고 개수가 실제로 안 달라지면 이 수정은 뜻이 없다.
        let line = "port portal report";
        let loose = build_matcher("port", false, false).unwrap();
        let whole = build_matcher("port", false, true).unwrap();
        assert_eq!(loose.count(line), 3, "portal·report 안의 port 까지 센다");
        assert_eq!(whole.count(line), 1, "단어 단위면 하나");
    }


