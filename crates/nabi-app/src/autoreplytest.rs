//! `autoreply` 시험 — 이 기능은 원격에 대신 글자를 보내므로 **막는 쪽**을 더 두껍게 본다.

use crate::autoreply::*;

fn rule(when: &str, send: &str) -> Rule {
    Rule { when: when.into(), send: send.into(), ..Default::default() }
}

#[test]
fn a_matching_prompt_gets_its_answer() {
    let rs = vec![rule("Continue? [y/N]", "y")];
    let got = decide("... Continue? [y/N] ", &rs, None).unwrap();
    assert_eq!(got, Some((0, "y\r".to_string())));
}

#[test]
fn newline_can_be_left_off() {
    let rs = vec![Rule { send_newline: false, ..rule("press any key", "x") }];
    assert_eq!(decide("press any key", &rs, None).unwrap(), Some((0, "x".to_string())));
}

#[test]
fn nothing_matches_nothing() {
    let rs = vec![rule("Continue?", "y")];
    assert_eq!(decide("all quiet here", &rs, None).unwrap(), None);
    assert_eq!(decide("", &rs, None).unwrap(), None);
}

/// 꺼 둔 규칙은 없는 것과 같아야 한다.
#[test]
fn a_disabled_rule_never_fires() {
    let rs = vec![Rule { enabled: false, ..rule("Continue?", "y") }];
    assert_eq!(decide("Continue?", &rs, None).unwrap(), None);
}

/// **비밀번호 물음에는 절대 답하지 않는다** — 규칙이 그렇게 돼 있어도 막는다.
#[test]
fn a_password_prompt_is_never_answered() {
    let rs = vec![rule("assword", "hunter2")];
    assert_eq!(decide("root@host's password: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

/// 한국어 프롬프트도 막는다.
#[test]
fn a_korean_secret_prompt_is_blocked_too() {
    let rs = vec![rule("입력", "x")];
    assert_eq!(decide("비밀번호를 입력하세요: ", &rs, None), Err(Blocked::LooksLikeSecret));
    assert_eq!(decide("암호: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

/// 2단계 인증 코드도 비밀이다.
#[test]
fn one_time_codes_are_secrets_as_well() {
    let rs = vec![rule("code", "123456")];
    assert_eq!(decide("Enter verification code: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

/// **되먹임을 끊는다** — 답이 또 프롬프트를 부르면 무한히 쏟아붓게 된다.
#[test]
fn a_rule_that_keeps_firing_is_stopped() {
    let rs = vec![rule("Continue?", "y")];
    assert!(decide("Continue?", &rs, Some((0, MAX_STREAK - 1))).unwrap().is_some());
    assert_eq!(decide("Continue?", &rs, Some((0, MAX_STREAK))), Err(Blocked::TooManyTimes));
}

/// 다른 규칙의 연속 횟수는 이 규칙을 막지 않는다.
#[test]
fn another_rules_streak_does_not_block_this_one() {
    let rs = vec![rule("Continue?", "y")];
    assert!(decide("Continue?", &rs, Some((7, 99))).unwrap().is_some());
}

/// 주입 텍스트는 ASCII 전용 — 한글을 넣으면 AI TUI가 깨진다(기존 규율).
#[test]
fn non_ascii_answers_are_refused() {
    let rs = vec![rule("계속?", "예")];
    assert_eq!(decide("계속?", &rs, None), Err(Blocked::NonAscii));
}

#[test]
fn an_empty_answer_is_refused() {
    let rs = vec![rule("Continue?", "")];
    assert_eq!(decide("Continue?", &rs, None), Err(Blocked::NonAscii));
}

/// 정규식 규칙.
#[test]
fn a_regex_rule_matches() {
    let rs = vec![Rule { regex: true, ..rule(r"\[y/N\]\s*$", "y") }];
    assert!(decide("Proceed [y/N] ", &rs, None).unwrap().is_some());
}

/// **잘못된 정규식이 pane을 죽이면 안 된다** — 안 맞는 것으로 본다.
#[test]
fn a_broken_regex_is_inert_not_fatal() {
    let rs = vec![Rule { regex: true, ..rule("[unclosed", "y") }];
    assert_eq!(decide("[unclosed", &rs, None).unwrap(), None);
}

/// **화면 위쪽의 옛 글에 걸리면 안 된다.** 프롬프트는 항상 끝에 있다.
#[test]
fn only_the_tail_of_the_screen_is_considered() {
    let rs = vec![rule("Continue? [y/N]", "y")];
    let old = format!("Continue? [y/N]{}", "x".repeat(2000));
    assert_eq!(decide(&old, &rs, None).unwrap(), None, "한참 위의 옛 프롬프트에 답했다");
}

/// 규칙 순서대로 먼저 맞는 것이 이긴다(사용자가 순서로 우선순위를 정한다).
#[test]
fn the_first_matching_rule_wins() {
    let rs = vec![rule("Continue", "a"), rule("Continue?", "b")];
    assert_eq!(decide("Continue?", &rs, None).unwrap().map(|x| x.1), Some("a\r".to_string()));
}

#[test]
fn secret_detection_is_case_insensitive() {
    assert!(looks_like_secret("PASSWORD:"));
    assert!(looks_like_secret("Passphrase for key:"));
    assert!(!looks_like_secret("Continue? [y/N]"));
}

#[test]
fn sendable_rejects_empty_and_unicode() {
    assert!(sendable("y"));
    assert!(!sendable(""));
    assert!(!sendable("예"));
}
