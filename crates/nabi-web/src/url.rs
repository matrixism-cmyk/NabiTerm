//! 주소창에 사람이 친 것을 **진짜 주소로 바꾼다**.
//!
//! 사람은 `google.com` 이라고 치지 `https://google.com` 이라고 치지 않는다. 그렇다고
//! 아무 말이나 주소로 보면 안 된다 — `러스트 문자열 자르기` 는 주소가 아니라 찾을 말이다.
//!
//! ## 어떻게 가르는가
//!
//! 빈칸이 있으면 찾을 말이다. 주소에는 빈칸이 못 들어간다.
//! 점이 없고 `localhost` 도 아니면 찾을 말이다. `깃허브` 같은 한 단어는 주소가 아니다.
//!
//! ## 왜 어떤 것은 http 인가
//!
//! `localhost:8080` 을 https 로 열면 십중팔구 실패한다. 개발 중인 서버는 대개 인증서가
//! 없기 때문이다. 그래서 내 컴퓨터를 가리키는 주소만 http 로 연다.
//!
//! SSH 포트 포워딩으로 원격 화면을 끌어오면 그것도 `localhost:포트` 로 보인다.
//! 우리가 SSH 를 하는 프로그램이라 이 경우가 특히 잦다.

/// 사람이 친 것을 열 수 있는 주소로 바꾼다.
pub fn resolve(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return "about:blank".into();
    }
    if has_scheme(s) {
        return s.into();
    }
    if looks_like_address(s) {
        let scheme = if is_local(s) { "http" } else { "https" };
        return format!("{scheme}://{s}");
    }
    format!("https://duckduckgo.com/?q={}", encode(s))
}

/// `https://` 처럼 앞에 방식이 이미 붙어 있는가.
///
/// 글자·숫자·`+-.` 로 시작해 `:` 로 끝나는 것만 방식으로 본다. `localhost:8080` 의
/// `localhost:` 를 방식으로 착각하면 안 되므로, `:` 뒤가 숫자면 방식이 아니라 포트로 본다.
fn has_scheme(s: &str) -> bool {
    let Some((head, rest)) = s.split_once(':') else {
        return false;
    };
    if head.is_empty() || !head.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return false;
    }
    if !head.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return false;
    }
    // 포트 번호를 방식으로 착각하지 않는다.
    !rest.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// 주소처럼 생겼는가.
fn looks_like_address(s: &str) -> bool {
    if s.chars().any(char::is_whitespace) {
        return false;
    }
    let host = s.split(['/', '?', '#']).next().unwrap_or(s);
    let host = host.split(':').next().unwrap_or(host);
    is_local(s) || (host.contains('.') && !host.starts_with('.') && !host.ends_with('.'))
}

/// 내 컴퓨터를 가리키는가.
fn is_local(s: &str) -> bool {
    let host = s.split(['/', '?', '#']).next().unwrap_or(s);
    let host = host.split(':').next().unwrap_or(host);
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "[::1]"
}

/// 찾을 말을 주소에 실을 수 있게 바꾼다.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn a_bare_host_gets_https() {
        assert_eq!(resolve("google.com"), "https://google.com");
        assert_eq!(resolve("news.ycombinator.com/item?id=1"), "https://news.ycombinator.com/item?id=1");
    }

    #[test]
    fn what_is_already_a_url_is_left_alone() {
        assert_eq!(resolve("http://example.com"), "http://example.com");
        assert_eq!(resolve("file:///C:/a.html"), "file:///C:/a.html");
    }

    #[test]
    fn my_own_machine_gets_http_because_it_has_no_certificate() {
        // SSH 포워딩으로 끌어온 원격 화면이 이 모양이다. 우리에게 가장 잦은 경우다.
        assert_eq!(resolve("localhost:8080"), "http://localhost:8080");
        assert_eq!(resolve("127.0.0.1:3000/admin"), "http://127.0.0.1:3000/admin");
        assert_eq!(resolve("LOCALHOST"), "http://LOCALHOST");
    }

    #[test]
    fn a_port_is_not_mistaken_for_a_scheme() {
        // localhost:8080 의 "localhost:" 를 방식으로 보면 열리지 않는 주소가 된다.
        assert!(resolve("localhost:8080").starts_with("http://"));
    }

    #[test]
    fn words_become_a_search() {
        assert_eq!(resolve("rust 문자열 자르기"), "https://duckduckgo.com/?q=rust+%EB%AC%B8%EC%9E%90%EC%97%B4+%EC%9E%90%EB%A5%B4%EA%B8%B0");
        // 점이 없는 한 단어는 주소가 아니다.
        assert!(resolve("깃허브").starts_with("https://duckduckgo.com/"));
    }

    #[test]
    fn nothing_typed_opens_a_blank_page() {
        assert_eq!(resolve("   "), "about:blank");
    }

    #[test]
    fn a_stray_dot_does_not_make_an_address() {
        assert!(resolve(".hidden").starts_with("https://duckduckgo.com/"));
        assert!(resolve("끝에점.").starts_with("https://duckduckgo.com/"));
    }
}
