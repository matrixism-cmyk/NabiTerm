//! 스니펫 플레이스홀더 치환 — 전송 직전 `{{date}}`/`{{time}}`/`{{datetime}}`/`{{clipboard}}`와
//! `{{env:NAME}}`(환경변수)를 실제 값으로 바꾼다. 시각·클립보드·env 조회를 주입받는
//! [`expand_with`]로 핵심을 테스트 가능하게 분리한다.

use chrono::{DateTime, Local};

/// 실제 현재 시각·클립보드·프로세스 환경변수 + pane 컨텍스트로 확장(메뉴/팔레트에서 스니펫 전송 시).
/// `vars`=포커스 pane의 컨텍스트(예: [("host",..),("user",..),("cwd",..),("port",..)]).
pub(crate) fn expand(tpl: &str, clipboard: &str, vars: &[(&str, String)]) -> String {
    expand_with(tpl, Local::now(), clipboard, vars, |k| std::env::var(k).ok())
}

/// 주입형 확장(테스트 가능). 알 수 없는 `{{...}}`/미정의 env는 안전하게 처리한다.
pub(crate) fn expand_with(
    tpl: &str,
    now: DateTime<Local>,
    clipboard: &str,
    vars: &[(&str, String)],
    env: impl Fn(&str) -> Option<String>,
) -> String {
    // {{datetime}}는 {{date}}/{{time}}의 부분문자열이 아니라 치환 순서는 무관.
    let mut s = tpl
        .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &now.format("%H:%M:%S").to_string())
        .replace("{{clipboard}}", clipboard);
    for (k, v) in vars {
        s = s.replace(&format!("{{{{{k}}}}}"), v); // {{host}} 등 pane 컨텍스트.
    }
    let s = expand_now_fmt(&s, now);
    expand_env(&s, env)
}

/// 대화형 인자 플레이스홀더를 찾는다: `{{?}}` 또는 `{{?:라벨}}`. 반환=내부키("?"/"?:라벨"),
/// 등장 순서·중복 제거. 전송 전 사용자에게 입력받아 expand의 vars로 넘기면 그대로 치환된다.
pub(crate) fn placeholders(tpl: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = tpl;
    while let Some(p) = rest.find("{{?") {
        let after = &rest[p + 2..]; // "?..."부터
        let Some(end) = after.find("}}") else { break };
        let key = after[..end].to_string(); // "?" 또는 "?:라벨"
        if !out.contains(&key) {
            out.push(key);
        }
        rest = &after[end + 2..];
    }
    out
}

/// strftime 형식이 유효한가(잘못된 지정자가 있으면 false — 패닉 방지).
fn valid_fmt(fmt: &str) -> bool {
    use chrono::format::{Item, StrftimeItems};
    !fmt.is_empty() && StrftimeItems::new(fmt).all(|it| !matches!(it, Item::Error))
}

/// `{{now:FORMAT}}`를 strftime 형식으로 치환(예: `{{now:%Y%m%d}}`). 형식이 잘못되면 원문 유지.
fn expand_now_fmt(s: &str, now: DateTime<Local>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find("{{now:") {
        out.push_str(&rest[..p]);
        let after = &rest[p + 6..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[p..]); // 닫힘 없음 → 그대로.
            return out;
        };
        let fmt = &after[..end];
        if valid_fmt(fmt) {
            out.push_str(&now.format(fmt).to_string());
        } else {
            out.push_str(&rest[p..p + 6 + end + 2]); // 잘못된 형식은 토큰 그대로.
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// `{{env:NAME}}`를 환경변수 값으로 치환(미정의는 빈 문자열, 닫힘 없으면 원문 유지).
fn expand_env(s: &str, env: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find("{{env:") {
        out.push_str(&rest[..p]);
        let after = &rest[p + 6..];
        match after.find("}}") {
            Some(end) => {
                out.push_str(&env(&after[..end]).unwrap_or_default());
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[p..]); // 닫는 `}}` 없음 → 그대로 둔다.
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{expand_with, placeholders};

    #[test]
    fn finds_interactive_placeholders() {
        assert_eq!(placeholders("a {{?:host}} b {{?}} c {{?:host}}"), vec!["?:host", "?"]);
        assert!(placeholders("no prompts {{date}} {{env:X}}").is_empty());
        assert_eq!(placeholders("{{?:서버}}"), vec!["?:서버"]);
    }

    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn substitutes_time_and_clipboard() {
        let now = at(2026, 6, 18, 9, 5, 3);
        let none = |_: &str| None;
        assert_eq!(expand_with("d={{date}}", now, "", &[], none), "d=2026-06-18");
        assert_eq!(expand_with("t={{time}}", now, "", &[], none), "t=09:05:03");
        assert_eq!(expand_with("{{datetime}}", now, "", &[], none), "2026-06-18 09:05:03");
        assert_eq!(expand_with("echo {{clipboard}}", now, "hi", &[], none), "echo hi");
        assert_eq!(expand_with("{{nope}} {{date}}", now, "", &[], none), "{{nope}} 2026-06-18");
    }

    #[test]
    fn custom_now_format() {
        let now = at(2026, 6, 18, 9, 5, 3);
        let none = |_: &str| None;
        assert_eq!(expand_with("{{now:%Y%m%d}}", now, "", &[], none), "20260618");
        assert_eq!(expand_with("t={{now:%H-%M}}", now, "", &[], none), "t=09-05");
        assert_eq!(expand_with("{{now:%Q}}", now, "", &[], none), "{{now:%Q}}");
    }

    #[test]
    fn substitutes_env_and_pane_vars() {
        let now = at(2026, 1, 1, 0, 0, 0);
        let env = |k: &str| (k == "USER").then(|| "kim".to_string());
        assert_eq!(expand_with("hi {{env:USER}}", now, "", &[], env), "hi kim");
        assert_eq!(expand_with("x{{env:NOPE}}y", now, "", &[], env), "xy");
        assert_eq!(expand_with("bad {{env:OPEN", now, "", &[], env), "bad {{env:OPEN");
        // pane 컨텍스트 변수.
        let vars = [("host", "h1".into()), ("user", "root".into()), ("cwd", "/srv".into())];
        assert_eq!(expand_with("ssh {{user}}@{{host}}:{{cwd}}", now, "", &vars, env), "ssh root@h1:/srv");
    }
}
