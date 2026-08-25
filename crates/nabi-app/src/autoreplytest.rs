//! `autoreply` 시험 — 이 기능은 원격에 대신 글자를 보내므로 **막는 쪽**을 더 두껍게 본다.

use crate::autoreply::*;
use crate::triggers::{parse_rule, Action};

/// 사용자가 설정에 적을 그대로에서 규칙을 만든다 — 시험이 실제 입력 경로를 밟게.
fn rules(entries: &[&str]) -> Vec<(String, Action)> {
    entries.iter().filter_map(|e| parse_rule(e)).collect()
}

#[test]
fn the_rule_syntax_understands_reply() {
    assert_eq!(parse_rule("continue? [y/n] -> reply:y").map(|r| r.1), Some(Action::Reply("y".into())));
}

#[test]
fn a_matching_prompt_gets_its_answer() {
    let rs = rules(&["continue? [y/n] -> reply:y"]);
    assert_eq!(decide("... Continue? [y/N] ", &rs, None).unwrap(), Some((0, "y\r".to_string())));
}

/// 끝의 역슬래시 하나는 "개행 붙이지 말라"는 뜻(단일 키 응답).
#[test]
fn a_trailing_backslash_means_no_newline() {
    let rs = rules(&["press any key -> reply:x\\"]);
    assert_eq!(decide("press any key", &rs, None).unwrap(), Some((0, "x".to_string())));
}

#[test]
fn nothing_matches_nothing() {
    let rs = rules(&["continue? -> reply:y"]);
    assert_eq!(decide("all quiet here", &rs, None).unwrap(), None);
    assert_eq!(decide("", &rs, None).unwrap(), None);
    assert_eq!(decide("   \n  ", &rs, None).unwrap(), None);
}

/// 알림 트리거는 자동 응답이 아니다 — 같은 목록에 있어도 답하지 않는다.
#[test]
fn a_plain_alert_rule_never_sends_anything() {
    let rs = rules(&["build failed", "deploy done -> telegram", "x -> command: git push"]);
    assert_eq!(decide("BUILD FAILED", &rs, None).unwrap(), None);
    assert_eq!(decide("deploy done", &rs, None).unwrap(), None);
}

/// **비밀번호 물음에는 절대 답하지 않는다** — 규칙이 그렇게 돼 있어도 막는다.
#[test]
fn a_password_prompt_is_never_answered() {
    let rs = rules(&["assword -> reply:hunter2"]);
    assert_eq!(decide("root@host's password: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

#[test]
fn a_korean_secret_prompt_is_blocked_too() {
    let rs = rules(&["입력 -> reply:x"]);
    assert_eq!(decide("비밀번호를 입력하세요: ", &rs, None), Err(Blocked::LooksLikeSecret));
    assert_eq!(decide("암호: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

#[test]
fn one_time_codes_are_secrets_as_well() {
    let rs = rules(&["code -> reply:123456"]);
    assert_eq!(decide("Enter verification code: ", &rs, None), Err(Blocked::LooksLikeSecret));
    assert_eq!(decide("Passphrase for key: ", &rs, None), Err(Blocked::LooksLikeSecret));
}

/// **되먹임을 끊는다** — 답이 또 프롬프트를 부르면 무한히 쏟아붓게 된다.
#[test]
fn a_rule_that_keeps_firing_is_stopped() {
    let rs = rules(&["continue? -> reply:y"]);
    assert!(decide("Continue?", &rs, Some((0, MAX_STREAK - 1))).unwrap().is_some());
    assert_eq!(decide("Continue?", &rs, Some((0, MAX_STREAK))), Err(Blocked::TooManyTimes));
}

#[test]
fn another_rules_streak_does_not_block_this_one() {
    let rs = rules(&["continue? -> reply:y"]);
    assert!(decide("Continue?", &rs, Some((7, 99))).unwrap().is_some());
}

/// 주입 텍스트는 ASCII 전용 — 한글을 넣으면 AI TUI가 깨진다(기존 규율).
#[test]
fn non_ascii_answers_are_refused() {
    let rs = rules(&["계속 -> reply:예"]);
    assert_eq!(decide("계속?", &rs, None), Err(Blocked::NonAscii));
}

#[test]
fn an_empty_answer_is_refused() {
    let rs = rules(&["continue? -> reply:"]);
    assert_eq!(decide("Continue?", &rs, None), Err(Blocked::NonAscii));
}

/// 대소문자는 가리지 않는다(트리거 규칙은 소문자로 저장된다).
#[test]
fn matching_ignores_case() {
    let rs = rules(&["CONTINUE? -> reply:y"]);
    assert!(decide("continue?", &rs, None).unwrap().is_some());
}

/// 규칙 순서대로 먼저 맞는 것이 이긴다(사용자가 순서로 우선순위를 정한다).
#[test]
fn the_first_matching_rule_wins() {
    let rs = rules(&["continue -> reply:a", "continue? -> reply:b"]);
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
