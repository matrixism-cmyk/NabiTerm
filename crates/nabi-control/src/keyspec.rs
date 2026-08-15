//! 사람이 읽는 키 이름 → 터미널 바이트(B5). `send --keys "ctrl+c enter esc f1"`.
//!
//! 지금까지 에이전트는 이스케이프 시퀀스를 알아야 raw 전송을 할 수 있었다(`\x1b[5~` 등).
//! herdr send-keys 표기를 벤치마킹: 공백 구분 키 이름 목록을 시퀀스로 컴파일한다.

/// 키 이름 목록(공백 구분)을 바이트로. 모르는 이름은 Err(전체 거부 — 부분 전송이 더 위험).
pub fn compile(spec: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for name in spec.split_whitespace() {
        out.extend(one(name).ok_or_else(|| format!("모르는 키: {name}"))?);
    }
    Ok(out)
}

fn one(name: &str) -> Option<Vec<u8>> {
    let n = name.to_ascii_lowercase();
    // 수식키 조합.
    if let Some(rest) = n.strip_prefix("ctrl+") {
        let c = single_char(rest)?;
        return c.is_ascii_alphabetic().then(|| vec![(c as u8) & 0x1f]);
    }
    if let Some(rest) = n.strip_prefix("alt+") {
        let mut v = vec![0x1b];
        v.extend(one(rest)?);
        return Some(v);
    }
    if n == "shift+tab" {
        return Some(b"\x1b[Z".to_vec());
    }
    // 기능·이동 키.
    let seq: &[u8] = match n.as_str() {
        "enter" | "return" | "cr" => b"\r",
        "tab" => b"\t",
        "esc" | "escape" => b"\x1b",
        "space" => b" ",
        "backspace" | "bs" => b"\x7f",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pgup" | "pageup" => b"\x1b[5~",
        "pgdn" | "pagedown" => b"\x1b[6~",
        "insert" => b"\x1b[2~",
        "delete" | "del" => b"\x1b[3~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => {
            // 단일 문자는 그대로(리터럴 타이핑).
            let c = single_char(&n)?;
            return Some(c.to_string().into_bytes());
        }
    };
    Some(seq.to_vec())
}

fn single_char(s: &str) -> Option<char> {
    let mut it = s.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_common_sequences() {
        assert_eq!(compile("ctrl+c").unwrap(), vec![0x03]);
        assert_eq!(compile("ctrl+t").unwrap(), vec![0x14]);
        assert_eq!(compile("enter").unwrap(), b"\r");
        assert_eq!(compile("esc pgup").unwrap(), b"\x1b\x1b[5~");
        assert_eq!(compile("shift+tab").unwrap(), b"\x1b[Z");
        assert_eq!(compile("alt+x").unwrap(), b"\x1bx");
        assert_eq!(compile("f5").unwrap(), b"\x1b[15~");
        assert_eq!(compile("q").unwrap(), b"q");
    }

    /// 모르는 키가 하나라도 있으면 전체 거부 — 부분 전송은 앱 상태를 어중간하게 만든다.
    #[test]
    fn unknown_key_rejects_whole_spec() {
        assert!(compile("enter frobnicate").is_err());
        assert!(compile("ctrl+1").is_err(), "ctrl+숫자는 미정의");
    }
}
