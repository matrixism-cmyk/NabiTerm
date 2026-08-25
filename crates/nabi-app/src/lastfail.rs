//! **세션별 마지막 실패 이유** — 왜 안 붙었는지를 그 세션 옆에 남긴다.
//!
//! 지금까지 연결 실패는 토스트로 한 번 스치고 사라졌다. 목록으로 돌아오면 어느 세션이
//! 왜 실패했는지 남는 것이 없어서, 알아보려면 다시 눌러 다시 실패시켜야 했다.
//!
//! ## 무엇을 열쇠로 삼는가
//!
//! 세션 **이름**이 아니라 접속 정보(`SessionKind`)를 열쇠로 쓴다. `pane_tag`가 같은
//! 이유로 그렇게 한다 — 이름은 사용자가 언제든 바꾸고, 바뀌면 기록이 끊긴다.
//!
//! ## 원문을 지우지 않는다
//!
//! 우리가 고른 갈래는 실마리일 뿐이다. 남에게 물어볼 때 필요한 것은 원문이므로 함께 남긴다.

use nabi_session::SessionKind;
use std::collections::HashMap;
use std::time::Instant;

/// 한 세션의 마지막 실패.
#[derive(Clone, Debug)]
pub(crate) struct LastFail {
    pub when: Instant,
    /// 서버·라이브러리가 준 원문(가공 없음).
    pub raw: String,
    /// 우리가 고른 갈래.
    pub cause: nabi_ssh::diagnose::Cause,
}

/// 접속 정보 → 마지막 실패. 세션 수만큼만 늘어나므로 상한이 따로 필요 없다.
pub(crate) type FailMap = HashMap<SessionKind, LastFail>;

/// 실패를 적어 둔다. 같은 세션의 옛 기록은 덮어쓴다 — 알고 싶은 것은 **마지막** 이유다.
pub(crate) fn note(map: &mut FailMap, kind: SessionKind, raw: &str) {
    let raw = raw.trim();
    if raw.is_empty() {
        return;
    }
    map.insert(
        kind,
        LastFail { when: Instant::now(), raw: clip(raw, 400), cause: nabi_ssh::diagnose::classify(raw) },
    );
}

/// 성공했으면 지운다. 붙고 나서도 옛 실패가 남아 있으면 **지금 고장 난 것으로 읽힌다.**
pub(crate) fn clear(map: &mut FailMap, kind: &SessionKind) {
    map.remove(kind);
}

/// 목록 옆에 붙일 한 줄: "인증 실패 · 3분 전".
pub(crate) fn summary(lang: nabi_i18n::Lang, f: &LastFail) -> String {
    let head = nabi_i18n::tr(lang, headline(f.cause));
    format!("{head} \u{00b7} {}", ago(lang, f.when.elapsed().as_secs()))
}

/// 자세히 보기(툴팁): 갈래 + 해 볼 것 + 원문.
pub(crate) fn detail(lang: nabi_i18n::Lang, f: &LastFail) -> String {
    let d = nabi_ssh::diagnose::diagnose(&f.raw, nabi_ssh::diagnose::AuthKind::Agent);
    // 첫 줄은 목록에 쓰는 것과 **같은 한 줄**이다. 여기서만 다른 말을 쓰면 같은 실패가
    // 두 가지 이름으로 불린다.
    let mut out = summary(lang, f);
    for h in &d.hints {
        out.push('\n');
        out.push_str("  - ");
        out.push_str(nabi_i18n::tr(lang, h));
    }
    out.push('\n');
    out.push('\n');
    out.push_str(&f.raw);
    out
}

/// 갈래 → 짧은 제목 키. `diagnose`의 긴 설명과 달리 목록 한 줄에 들어갈 길이다.
fn headline(c: nabi_ssh::diagnose::Cause) -> &'static str {
    use nabi_ssh::diagnose::Cause as C;
    match c {
        C::DnsFailure => "lastfail.dns",
        C::Refused => "lastfail.refused",
        C::Timeout => "lastfail.timeout",
        C::AuthFailed => "lastfail.auth",
        C::KeyFile => "lastfail.key",
        C::HostKey => "lastfail.hostkey",
        C::Algorithm => "lastfail.algo",
        C::Disconnected => "lastfail.dropped",
        C::Unknown => "lastfail.unknown",
    }
}

/// "3분 전" — 초 단위까지 보여 줄 이유가 없다.
fn ago(lang: nabi_i18n::Lang, secs: u64) -> String {
    let (n, unit) = match secs {
        0..=59 => (secs.max(1), "lastfail.sec"),
        60..=3599 => (secs / 60, "lastfail.min"),
        3600..=86_399 => (secs / 3600, "lastfail.hour"),
        _ => (secs / 86_400, "lastfail.day"),
    };
    format!("{n}{}", nabi_i18n::tr(lang, unit))
}

fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}\u{2026}", &s[..i]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_ssh::diagnose::Cause;

    fn ssh(host: &str) -> SessionKind {
        SessionKind::Ssh {
            host: host.into(),
            port: 22,
            user: "u".into(),
            credential_ref: None,
            key_path: None,
            jump: None,
            agent_forward: false,
        }
    }

    #[test]
    fn a_failure_is_remembered_against_its_session() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), "IO error: Connection refused (os error 10061)");
        assert_eq!(m[&ssh("a")].cause, Cause::Refused);
        assert!(!m.contains_key(&ssh("b")), "다른 세션까지 물들면 안 된다");
    }

    /// **원문을 지우지 않는다** — 남에게 물을 때 쓰이는 것은 우리 번역이 아니다.
    #[test]
    fn the_original_message_is_kept() {
        let mut m = FailMap::new();
        let raw = "Auth failed: no more auth methods available";
        note(&mut m, ssh("a"), raw);
        assert_eq!(m[&ssh("a")].raw, raw);
    }

    /// 마지막 것만 남는다 — 알고 싶은 것은 지금 왜 안 되는가다.
    #[test]
    fn only_the_latest_failure_survives() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), "Connection refused");
        note(&mut m, ssh("a"), "Auth failed");
        assert_eq!(m[&ssh("a")].cause, Cause::AuthFailed);
        assert_eq!(m.len(), 1);
    }

    /// **붙고 나면 지운다.** 남겨 두면 멀쩡한 세션이 고장 난 것으로 보인다.
    #[test]
    fn connecting_successfully_clears_the_mark() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), "Connection refused");
        clear(&mut m, &ssh("a"));
        assert!(m.is_empty());
    }

    /// 목록의 한 줄과 자세히 보기의 첫 줄은 **같은 말**이어야 한다.
    #[test]
    fn the_detail_opens_with_the_same_line_the_list_shows() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), "Connection refused (os error 10061)");
        let f = &m[&ssh("a")];
        let l = nabi_i18n::Lang::Ko;
        assert!(detail(l, f).starts_with(&summary(l, f)));
    }

    #[test]
    fn an_empty_message_is_not_recorded() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), "   ");
        assert!(m.is_empty());
    }

    /// 아주 긴 원문이 목록을 밀어내지 않게 자른다(다만 잘렸음을 표시한다).
    #[test]
    fn a_huge_message_is_clipped() {
        let mut m = FailMap::new();
        note(&mut m, ssh("a"), &"x".repeat(2000));
        let kept = &m[&ssh("a")].raw;
        assert!(kept.chars().count() <= 401);
        assert!(kept.ends_with('\u{2026}'));
    }

    #[test]
    fn elapsed_time_reads_in_the_largest_useful_unit() {
        let l = nabi_i18n::Lang::En;
        assert!(ago(l, 5).starts_with('5'));
        assert!(ago(l, 300).starts_with('5'));
        assert!(ago(l, 7200).starts_with('2'));
        assert!(ago(l, 172_800).starts_with('2'));
    }

    /// 0초여도 "0"이 아니라 최소 1로 읽는다 — "0분 전"은 아무 말도 하지 않는다.
    #[test]
    fn a_fresh_failure_does_not_read_as_zero() {
        assert!(ago(nabi_i18n::Lang::En, 0).starts_with('1'));
    }
}
