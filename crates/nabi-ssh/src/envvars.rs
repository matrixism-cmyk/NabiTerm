//! **세션마다 보낼 환경변수** — `KEY=VALUE` 줄을 판다.
//!
//! 서버는 접속한 사람이 누구인지, 무엇을 하러 왔는지 환경변수로 가른다. 배포 스크립트가
//! `DEPLOY_USER`를 보고, 로그가 `LANG`을 보고, 사내 도구가 `TEAM`을 본다. 지금까지는
//! 붙고 나서 매번 손으로 `export`를 치거나 접속 후 실행 명령에 욱여넣어야 했다.
//!
//! ## 서버가 거절할 수 있다
//!
//! ssh 서버는 `AcceptEnv`에 적힌 것만 받는다(대개 `LANG`·`LC_*`뿐이다). 거절은 **오류가
//! 아니다** — 그래서 답을 기다리지 않고 보내고, 실패해도 세션을 열어 준다. 기다렸다가
//! 실패로 처리하면 대부분의 서버에서 접속 자체가 안 되는 것처럼 보인다.
//!
//! ## 무엇을 거르나
//!
//! 이름은 셸이 환경변수로 인정하는 모양(글자·숫자·밑줄, 숫자로 시작 금지)만 받는다.
//! 값에 개행이 들어가면 프로토콜 프레임이 깨지므로 **그 줄은 통째로 버린다** — 잘라서
//! 보내면 사용자가 적은 것과 서버가 받은 것이 조용히 달라진다.

/// `KEY=VALUE` 여러 줄을 판다. 잘못된 줄은 조용히 버린다(모양은 UI가 미리 보여 준다).
pub fn parse(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(parse_line).collect()
}

/// 한 줄을 판다. 주석(`#`)과 빈 줄은 None.
pub fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (k, v) = line.split_once('=')?;
    let (k, v) = (k.trim(), v.trim());
    if !valid_name(k) || v.contains(['\r', '\n']) {
        return None;
    }
    Some((k.to_string(), v.to_string()))
}

/// 셸이 환경변수 이름으로 인정하는 모양인가.
pub fn valid_name(k: &str) -> bool {
    !k.is_empty()
        && !k.starts_with(|c: char| c.is_ascii_digit())
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_pair_is_read() {
        assert_eq!(parse_line("LANG=ko_KR.UTF-8"), Some(("LANG".into(), "ko_KR.UTF-8".into())));
    }

    #[test]
    fn spaces_around_the_equals_are_trimmed() {
        assert_eq!(parse_line("  TEAM = infra  "), Some(("TEAM".into(), "infra".into())));
    }

    /// 값 안의 `=`는 값의 일부다 — 첫 `=`에서만 가른다.
    #[test]
    fn only_the_first_equals_splits() {
        assert_eq!(parse_line("OPTS=a=b=c"), Some(("OPTS".into(), "a=b=c".into())));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        assert_eq!(parse_line("# 주석"), None);
        assert_eq!(parse_line("   "), None);
    }

    /// 셸이 못 쓰는 이름은 버린다 — 보내 봐야 서버가 무시하거나 거절한다.
    #[test]
    fn a_name_the_shell_cannot_use_is_refused() {
        assert!(!valid_name("2FAST"));
        assert!(!valid_name("MY-VAR"));
        assert!(!valid_name(""));
        assert!(valid_name("_x9"));
        assert_eq!(parse_line("2FAST=1"), None);
    }

    /// **개행이 든 값은 통째로 버린다.** 잘라 보내면 적은 것과 보낸 것이 조용히 달라진다.
    #[test]
    fn a_value_with_a_newline_is_dropped_whole() {
        assert_eq!(parse_line("K=a\nb"), None);
        assert_eq!(parse_line("K=a\rb"), None);
    }

    #[test]
    fn several_lines_are_read_in_order() {
        let v = parse("A=1\n# 주석\n\nB=2\n잘못된줄\nC=3");
        assert_eq!(v, vec![
            ("A".into(), "1".into()),
            ("B".into(), "2".into()),
            ("C".into(), "3".into()),
        ]);
    }

    #[test]
    fn an_empty_value_is_allowed() {
        assert_eq!(parse_line("EMPTY="), Some(("EMPTY".into(), String::new())));
    }
}
