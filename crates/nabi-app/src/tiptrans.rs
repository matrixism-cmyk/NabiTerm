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

/// 사전 항목: 핵심 구절(모두 포함돼야 매칭) + 번역 + 접두사 없는 줄에도 붙일지 여부.
///
/// `standalone`이 false면 `Tip:`/`Note:` 줄에서만 번역한다 — 상태 줄처럼 다른 정보가 함께
/// 있는 줄을 통째로 덮어 버리지 않기 위해서다(예: "esc to interrupt"는 상태 줄의 일부).
struct Entry {
    needles: &'static [&'static str],
    ko: &'static str,
    standalone: bool,
}

const fn e(needles: &'static [&'static str], ko: &'static str) -> Entry {
    Entry { needles, ko, standalone: false }
}

/// 접두사 없이 그 줄만으로도 뜻이 완결되는 항목(안내 박스 문장 등).
const fn s(needles: &'static [&'static str], ko: &'static str) -> Entry {
    Entry { needles, ko, standalone: true }
}

/// 실제 화면에서 확인한 문구를 기준으로 하고, 확신이 없으면 넣지 않는다.
const DICT: &[Entry] = &[
    s(&["double-tap esc", "rewind"], "Esc를 두 번 누르면 코드와 대화를 이전 시점으로 되돌립니다"),
    s(&["/color", "/rename"], "여러 세션을 함께 쓴다면 /color·/rename으로 한눈에 구분하세요"),
    s(&["/init", "claude.md"], "/init 을 실행하면 프로젝트 지침 파일(CLAUDE.md)을 만들어 줍니다"),
    s(&["tips for getting started"], "시작하기 팁"),
    s(&["what's new"], "새로운 기능"),
    s(&["launched claude in your home directory"], "홈 디렉터리에서 claude를 실행했습니다 — 작업할 프로젝트 폴더에서 실행하는 편이 좋습니다"),
    s(&["transcript saving is off"], "대화 기록 저장이 꺼져 있습니다"),
    e(&["shift+tab", "cycle"], "Shift+Tab을 누르면 권한 모드가 순환합니다"),
    e(&["esc to interrupt"], "Esc를 누르면 진행 중인 작업을 중단합니다"),
    e(&["ctrl+c", "exit"], "Ctrl+C를 두 번 누르면 종료합니다"),
    e(&["drag", "drop", "image"], "이미지를 창에 끌어다 놓으면 대화에 첨부됩니다"),
    e(&["@", "mention", "file"], "@ 뒤에 파일명을 적으면 그 파일을 대화에 붙일 수 있습니다"),
    s(&["/help", "command"], "/help 를 입력하면 사용할 수 있는 명령 목록이 나옵니다"),
    e(&["--continue", "resume"], "--continue(또는 /resume)로 지난 대화를 이어서 시작할 수 있습니다"),
    e(&["plan mode"], "계획 모드에서는 파일을 바꾸지 않고 계획만 먼저 세웁니다"),
    s(&["/compact", "context"], "/compact 로 대화를 요약해 컨텍스트 여유를 확보하세요"),
    s(&["/memory", "claude.md"], "/memory 로 프로젝트 지침(CLAUDE.md)을 편집할 수 있습니다"),
    e(&["/mcp", "server"], "/mcp 에서 MCP 서버 연결을 관리합니다"),
    e(&["/vim", "vim mode"], "/vim 으로 입력창에서 vim 키 조작을 쓸 수 있습니다"),
    e(&["run /", "in the background"], "명령을 백그라운드로 돌려 두고 다른 작업을 계속할 수 있습니다"),
    e(&["type", "/", "slash command"], "슬래시(/)를 입력하면 명령 목록이 열립니다"),
];

/// 사전 조회(핵심 구절이 모두 포함될 때만). `standalone_only`면 접두사 없는 줄에도
/// 붙일 수 있는 항목만 본다.
fn find(text: &str, standalone_only: bool) -> Option<&'static str> {
    let n = norm(text);
    DICT.iter()
        .filter(|e| !standalone_only || e.standalone)
        .find(|e| e.needles.iter().all(|k| n.contains(*k)))
        .map(|e| e.ko)
}

/// `Tip:`/`Note:` 본문의 번역(모든 항목 대상).
pub(crate) fn lookup(body: &str) -> Option<&'static str> {
    find(body, false)
}

/// 접두사 없는 일반 줄의 번역 — 그 줄만으로 뜻이 완결되는 항목만 매칭한다
/// (사용자 요청 2026-08-19: 'Tips for getting started' 같은 줄도 번역).
/// AI 번역은 요청하지 않는다 — 아무 줄이나 보내면 비용이 폭발한다.
pub(crate) fn lookup_line(line: &str) -> Option<&'static str> {
    let t = line.trim();
    // 너무 짧거나 비ASCII(이미 번역된 줄·한글 출력)는 건너뛴다.
    if t.chars().count() < 12 || !t.is_ascii() {
        return None;
    }
    find(t, true)
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

    /// 접두사 없는 줄도 번역된다(단, standalone 항목만).
    #[test]
    fn standalone_lines_translate_without_prefix() {
        assert!(lookup_line("Tips for getting started").is_some());
        assert!(lookup_line("Run /init to create a CLAUDE.md file with instructions").is_some());
        // 상태 줄 일부는 접두사 없이는 번역하지 않는다(줄 전체를 덮어 정보가 사라진다).
        assert_eq!(lookup_line("esc to interrupt - left arrow for agents"), None);
        // 짧은 줄·한글 줄은 대상 아님.
        assert_eq!(lookup_line("what's new"), None, "12자 미만");
        assert_eq!(lookup_line("한글 출력 줄입니다 여기에는 무엇이"), None);
    }

    #[test]
    fn unknown_tips_return_none() {
        assert_eq!(lookup("Something entirely new that we have never seen"), None);
    }
}
