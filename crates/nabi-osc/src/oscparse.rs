//! OSC 본문 파서(코드별 OscEvent 생성). scanner의 상태 기계에서 호출.

use crate::base64::base64_decode;
use crate::scanner::OscEvent;

/// OSC 본문("code;rest")을 해석해 이벤트를 만든다(미지원 코드는 None).
pub(crate) fn parse_osc(body: &str) -> Option<OscEvent> {
    let mut parts = body.splitn(2, ';');
    let code = parts.next()?;
    let rest = parts.next().unwrap_or("");
    match code {
        "7" => Some(OscEvent::Cwd(decode_file_uri(rest))),
        "133" => parse_133(rest),
        "633" => parse_633(rest),
        "52" => parse_52(rest),
        "9" => parse_9(rest),
        "777" => parse_777(rest),
        "1337" => parse_1337(rest),
        "7771" => parse_7771(rest),
        _ => None,
    }
}

/// OSC 7771;<verb>;<json> — 제어 평면 in-band 동작. verb와 json 본문을 분리.
fn parse_7771(rest: &str) -> Option<OscEvent> {
    let mut it = rest.splitn(2, ';');
    let verb = it.next().filter(|v| !v.is_empty())?;
    let json = it.next().unwrap_or("").to_string();
    Some(OscEvent::Control(verb.to_string(), json))
}

/// OSC 9: "<text>"는 데스크톱 알림. "4;<state>;<pct>"는 진행률(WT/ConEmu).
/// state 0=제거, 1/2/4=설정(pct%), 3=불확정.
fn parse_9(rest: &str) -> Option<OscEvent> {
    if rest.is_empty() {
        return None;
    }
    if rest == "4" || rest.starts_with("4;") {
        let mut it = rest.split(';').skip(1);
        let state = it.next().unwrap_or("0");
        let pct = it.next().and_then(|p| p.parse::<u8>().ok());
        let val = if state == "0" { None } else { pct.map(|p| p.min(100)) };
        return Some(OscEvent::Progress(val));
    }
    Some(OscEvent::Notify(rest.to_string()))
}

/// OSC 1337 (iTerm2): "CurrentDir=/path" → cwd. 그 외 키는 무시.
fn parse_1337(rest: &str) -> Option<OscEvent> {
    let dir = rest.strip_prefix("CurrentDir=")?;
    (!dir.is_empty()).then(|| OscEvent::Cwd(dir.to_string()))
}

/// OSC 52: "<selection>;<base64>" → 클립보드 텍스트. "?"(쿼리)는 무시.
fn parse_52(rest: &str) -> Option<OscEvent> {
    let (_selection, data) = rest.split_once(';')?;
    
    
    if data == "?" {
        return None;
    }
    let bytes = base64_decode(data)?;
    let text = String::from_utf8(bytes).ok()?;
    Some(OscEvent::ClipboardCopy(text))
}

/// OSC 777;notify;<title>;<body> → 알림(urxvt/foot 계열).
fn parse_777(rest: &str) -> Option<OscEvent> {
    let mut it = rest.splitn(3, ';');
    if it.next()? != "notify" {
        return None;
    }
    let title = it.next().unwrap_or("");
    let body = it.next().unwrap_or("");
    let msg = if body.is_empty() {
        title.to_string()
    } else {
        format!("{title}: {body}")
    };
    (!msg.is_empty()).then_some(OscEvent::Notify(msg))
}

/// OSC 633;E;<base64> — 셸 통합 v2가 보낸 "방금 실행한 명령줄"(base64 UTF-8).
/// (VS Code 호환 코드. 명령에 ;·개행이 섞여도 안전하도록 base64로 싣는다.)
fn parse_633(rest: &str) -> Option<OscEvent> {
    let data = rest.strip_prefix("E;")?;
    let bytes = base64_decode(data)?;
    let cmd = String::from_utf8(bytes).ok()?;
    (!cmd.trim().is_empty()).then_some(OscEvent::CommandLine(cmd))
}

fn parse_133(rest: &str) -> Option<OscEvent> {
    let mut it = rest.split(';');
    match it.next()? {
        "A" => Some(OscEvent::PromptStart),
        "B" => Some(OscEvent::CommandStart),
        "C" => Some(OscEvent::CommandExecuted),
        "D" => Some(OscEvent::CommandFinished(
            it.next().and_then(|s| s.parse::<i32>().ok()),
        )),
        _ => None,
    }
}

fn decode_file_uri(s: &str) -> String {
    let path = s
        .strip_prefix("file://")
        .map(|rest| match rest.find('/') {
            Some(i) => &rest[i..],
            None => rest,
        })
        .unwrap_or(s);
    percent_decode(path)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let (Some(hi), Some(lo)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_osc;
    use crate::scanner::OscEvent;

    #[test]
    fn cwd_via_osc7_and_1337() {
        assert_eq!(parse_osc("7;file://host/home/u%20a"), Some(OscEvent::Cwd("/home/u a".into()))); // URI+퍼센트 디코드.
        assert_eq!(parse_osc("1337;CurrentDir=/tmp/x"), Some(OscEvent::Cwd("/tmp/x".into())));
        assert_eq!(parse_osc("1337;Other=1"), None); // 미지원 키.
    }

    #[test]
    fn prompt_markers_133() {
        assert_eq!(parse_osc("133;A"), Some(OscEvent::PromptStart));
        assert_eq!(parse_osc("133;D;0"), Some(OscEvent::CommandFinished(Some(0))));
        assert_eq!(parse_osc("133;D"), Some(OscEvent::CommandFinished(None)));
        assert_eq!(parse_osc("133;Z"), None); // 미지원 서브.
    }

    #[test]
    fn progress_and_notify_9() {
        assert_eq!(parse_osc("9;4;3;55"), Some(OscEvent::Progress(Some(55))));
        assert_eq!(parse_osc("9;4;0;55"), Some(OscEvent::Progress(None))); // state 0=제거.
        assert_eq!(parse_osc("9;4;3;250"), Some(OscEvent::Progress(Some(100)))); // 100 상한.
        assert_eq!(parse_osc("9;hello"), Some(OscEvent::Notify("hello".into())));
    }

    #[test]
    fn clipboard_52_base64() {
        assert_eq!(parse_osc("52;c;aGk="), Some(OscEvent::ClipboardCopy("hi".into()))); // "hi".
        assert_eq!(parse_osc("52;c;?"), None); // 쿼리는 무시.
    }

    #[test]
    fn notify_777_and_control_7771() {
        assert_eq!(parse_osc("777;notify;Title;Body"), Some(OscEvent::Notify("Title: Body".into())));
        assert_eq!(parse_osc("777;other;x"), None); // notify 아님.
        assert_eq!(parse_osc("7771;list;{}"), Some(OscEvent::Control("list".into(), "{}".into())));
        assert_eq!(parse_osc("7771;"), None); // 빈 verb.
        assert_eq!(parse_osc("999;x"), None); // 미지원 코드.
    }
}
