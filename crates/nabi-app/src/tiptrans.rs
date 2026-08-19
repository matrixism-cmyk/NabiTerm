//! 터미널의 영문 팁 감지 + 한글 사전 번역(순수 로직 — UI는 tipoverlay.rs).
//!
//! AI CLI들은 화면에 `Tip: Double-tap esc to rewind…` 같은 영문 안내를 띄운다. 한국어
//! 사용자에게는 읽는 부담이 커서(사용자 요청 2026-08-19) 그 줄 위에 한글 번역을 겹쳐
//! 그린다. **터미널 그리드는 절대 고치지 않는다** — 프로그램이 다시 출력하면 충돌하고,
//! 복사·검색 결과도 원문이어야 하기 때문이다(오버레이만 덧그림, 원문은 호버로 표시).
//!
//! 사전은 "구별되는 핵심 구절(needle)"로 맞춘다. 문구가 조금 바뀌어도 살아남고, 맞지
//! 않으면 아무 것도 하지 않는다(오역보다 미번역이 낫다). 사전에 없으면 선택적으로
//! AI 번역(tipai.rs, 기본 꺼짐)을 쓴다.

/// 한 줄에서 팁 본문을 뽑는다 — `Tip:`/`Note:` 접두(앞의 기호·공백 무시)를 인식한다.
/// 팁이 아니면 None.
pub(crate) fn tip_body(line: &str) -> Option<&str> {
    let t = line.trim();
    // 앞의 장식 기호(※·💡··, > 등)를 건너뛰고 첫 알파벳부터 본다.
    let start = t.find(|c: char| c.is_ascii_alphabetic())?;
    let rest = &t[start..];
    for prefix in ["Tip:", "TIP:", "Note:", "NOTE:", "Hint:"] {
        if let Some(body) = rest.strip_prefix(prefix) {
            let body = body.trim();
            // 너무 짧으면(잘린 줄) 번역하지 않는다.
            return (body.chars().count() >= 12).then_some(body);
        }
    }
    None
}

/// 비교용 정규화: 소문자 + 공백 접기.
fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// (핵심 구절, 한국어 번역). 구절이 모두 들어 있는 팁에만 매칭된다(오탐 방지).
/// 실제 화면에서 확인한 문구를 기준으로 하고, 확신이 없으면 넣지 않는다.
const DICT: &[(&[&str], &str)] = &[
    (&["double-tap esc", "rewind"], "Esc를 두 번 누르면 코드와 대화를 이전 시점으로 되돌립니다"),
    (&["/color", "/rename"], "여러 세션을 함께 쓴다면 /color·/rename으로 한눈에 구분하세요"),
    (&["/init", "claude.md"], "/init 을 실행하면 프로젝트 지침 파일(CLAUDE.md)을 만들어 줍니다"),
    (&["shift+tab", "cycle"], "Shift+Tab을 누르면 권한 모드가 순환합니다"),
    (&["esc to interrupt"], "Esc를 누르면 진행 중인 작업을 중단합니다"),
    (&["ctrl+c", "exit"], "Ctrl+C를 두 번 누르면 종료합니다"),
    (&["drag", "drop", "image"], "이미지를 창에 끌어다 놓으면 대화에 첨부됩니다"),
    (&["@", "mention", "file"], "@ 뒤에 파일명을 적으면 그 파일을 대화에 붙일 수 있습니다"),
    (&["/help", "command"], "/help 를 입력하면 사용할 수 있는 명령 목록이 나옵니다"),
    (&["--continue", "resume"], "--continue(또는 /resume)로 지난 대화를 이어서 시작할 수 있습니다"),
    (&["plan mode"], "계획 모드에서는 파일을 바꾸지 않고 계획만 먼저 세웁니다"),
    (&["/compact", "context"], "/compact 로 대화를 요약해 컨텍스트 여유를 확보하세요"),
    (&["/memory", "claude.md"], "/memory 로 프로젝트 지침(CLAUDE.md)을 편집할 수 있습니다"),
    (&["/mcp", "server"], "/mcp 에서 MCP 서버 연결을 관리합니다"),
    (&["/vim", "vim mode"], "/vim 으로 입력창에서 vim 키 조작을 쓸 수 있습니다"),
    (&["run /", "in the background"], "명령을 백그라운드로 돌려 두고 다른 작업을 계속할 수 있습니다"),
    (&["type", "/", "slash command"], "슬래시(/)를 입력하면 명령 목록이 열립니다"),
];

/// 사전에서 번역을 찾는다(핵심 구절이 모두 포함될 때만).
pub(crate) fn lookup(body: &str) -> Option<&'static str> {
    let n = norm(body);
    DICT.iter()
        .find(|(needles, _)| needles.iter().all(|k| n.contains(*k)))
        .map(|(_, ko)| *ko)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tip_lines_only() {
        assert_eq!(
            tip_body("  \u{203b} Tip: Double-tap esc to rewind the code"),
            Some("Double-tap esc to rewind the code")
        );
        assert_eq!(tip_body("Note: You have launched claude in your home directory"),
                   Some("You have launched claude in your home directory"));
        assert_eq!(tip_body("cargo build --release"), None, "일반 명령은 팁이 아니다");
        assert_eq!(tip_body("Tip: short"), None, "잘린 짧은 줄은 번역하지 않는다");
        assert_eq!(tip_body(""), None);
    }

    #[test]
    fn dictionary_matches_by_key_phrases() {
        let body = tip_body("Tip: Double-tap esc to rewind the code and/or conversation").unwrap();
        assert!(lookup(body).unwrap().contains("되돌"));
        // 구절이 하나라도 빠지면 매칭하지 않는다(오역 방지).
        assert_eq!(lookup("esc is a key on your keyboard"), None);
        // 공백·대소문자 차이는 무시.
        assert!(lookup("Running multiple sessions?  Use /COLOR and /rename").is_some());
    }

    #[test]
    fn unknown_tips_return_none() {
        assert_eq!(lookup("Something entirely new that we have never seen"), None);
    }
}
