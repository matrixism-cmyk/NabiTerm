//! 떠 있는 나비텀에 **일을 넘긴다**(배치 AP).
//!
//! ## 왜 따로 뒀나
//!
//! 탐색기 우클릭은 `nabi.exe --open-here "%V"` 처럼 **새 프로세스**로 들어온다. 그런데
//! 이미 나비텀이 떠 있으면 창을 하나 더 띄우는 것은 사용자가 원한 것이 아니다. 그래서
//! 떠 있는 쪽에 파이프로 넘기고 조용히 끝낸다.
//!
//! 파일을 nabiPad 로 여는 것도 똑같은 모양이라, 그 왕복을 여기 한 번만 적는다.
//! 두 곳에 따로 적어 두면 한쪽만 고치는 날이 온다.

use std::io::{BufRead, BufReader, Write};

/// 떠 있는 인스턴스에 `op` 요청을 보낸다. 성공하면 true.
///
/// `field` 는 보낼 값의 이름(`path` 등), `value` 는 그 값이다.
pub(crate) fn send(pipe: &str, token: &str, op: &str, field: &str, value: &str) -> bool {
    let Ok(mut f) = std::fs::OpenOptions::new().read(true).write(true).open(pipe) else {
        return false;
    };
    let Ok(clone) = f.try_clone() else { return false };
    let mut rd = BufReader::new(clone);
    let mut line = String::new();
    let hello = format!(r#"{{"op":"hello","token":"{token}","from":null}}"#);
    if writeln!(f, "{hello}").is_err() || rd.read_line(&mut line).is_err() || !line.contains(r#""res":"ok""#) {
        return false;
    }
    let req = format!(r#"{{"op":"{op}","{field}":"{}"}}"#, escape(value));
    line.clear();
    writeln!(f, "{req}").is_ok() && rd.read_line(&mut line).is_ok() && line.contains(r#""res":"ok""#)
}

/// JSON 문자열에 넣을 수 있게 다듬는다.
///
/// 윈도우 경로에는 역슬래시가 들어 있고 따옴표도 들어올 수 있다. 그대로 넣으면 JSON 이
/// 깨져서 **아무 일도 일어나지 않는다** — 그러면 왜 안 되는지 알 길이 없다.
pub(crate) fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 떠 있는 인스턴스를 찾아 넘긴다. 넘겼으면 true.
pub(crate) fn delegate(op: &str, field: &str, value: &str) -> bool {
    let dir = nabi_config::StorageLayout::resolve().base;
    match nabi_control::discovery::read(&dir) {
        Some((pipe, token)) => send(&pipe, &token, op, field, value),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn windows_paths_survive_the_json() {
        // 역슬래시를 그대로 넣으면 JSON 이 깨져서 조용히 아무 일도 안 일어난다.
        assert_eq!(escape(r"C:\일감\a.txt"), r"C:\\일감\\a.txt");
    }

    #[test]
    fn quotes_are_escaped_too() {
        assert_eq!(escape(r#"C:\a"b.txt"#), r#"C:\\a\"b.txt"#);
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(escape("hello"), "hello");
        assert_eq!(escape(""), "");
    }
}
