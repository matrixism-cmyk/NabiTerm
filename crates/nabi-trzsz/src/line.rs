//! 줄 프로토콜 — `#<TYPE>:<PAYLOAD><NEWLINE>`.
//!
//! 줄바꿈은 협상값이다. 원격이 Windows면 `!\n`을 쓴다(cmd가 단독 `\n`을 흘리지 못해서다).
//! 그리고 줄 앞에는 **쓰레기가 섞일 수 있다** — tmux 상태줄, 셸이 뱉은 잔여 출력, 프롬프트.
//! 그래서 줄에서 마지막 `#`부터를 프레임으로 본다(원본 trzsz도 같은 방식이다).

/// 한 프레임.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub typ: String,
    pub payload: String,
}

impl Line {
    /// 이 프레임이 원격의 실패 통보인가(`FAIL` = 추적 포함, `fail` = 조용한 실패).
    pub fn is_failure(&self) -> bool {
        self.typ.eq_ignore_ascii_case("fail")
    }
}

/// 한 줄이 이보다 길면 프로토콜 위반으로 본다(메모리 폭주 방어).
const LINE_MAX: usize = 64 << 20;

/// 바이트 스트림을 프레임으로 자르는 조립기.
#[derive(Default)]
pub struct LineFramer {
    buf: Vec<u8>,
    overflowed: bool,
}

impl LineFramer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 청크를 넣고 완성된 프레임을 받는다. 프레임이 아닌 줄은 조용히 버린다.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Line> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(nl) = self.buf.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=nl).collect();
            if self.overflowed {
                self.overflowed = false; // 넘친 줄의 나머지였다 — 버리고 정상으로 돌아온다.
                continue;
            }
            if let Some(l) = parse_line(&raw[..nl]) {
                out.push(l);
            }
        }
        if self.buf.len() > LINE_MAX {
            self.buf.clear();
            self.overflowed = true;
        }
        out
    }

    /// 줄이 너무 길어 버린 적이 있는가 — 호출자는 이걸 프로토콜 오류로 다뤄야 한다.
    pub fn had_overflow(&self) -> bool {
        self.overflowed
    }
}

/// 줄 하나를 프레임으로. 앞의 쓰레기는 버리고 **마지막 `#`부터** 읽는다.
fn parse_line(line: &[u8]) -> Option<Line> {
    let hash = line.iter().rposition(|&b| b == b'#')?;
    let body = &line[hash + 1..];
    let colon = body.iter().position(|&b| b == b':')?;
    let typ = std::str::from_utf8(&body[..colon]).ok()?;
    if typ.is_empty() || !typ.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    // 줄 끝의 `\r`과 Windows 줄바꿈의 `!`를 걷어낸다. 페이로드(base64·10진수)에는 없는 글자다.
    let tail = trim_end(&body[colon + 1..]);
    let payload = std::str::from_utf8(tail).ok()?;
    Some(Line { typ: typ.to_owned(), payload: payload.to_owned() })
}

fn trim_end(mut s: &[u8]) -> &[u8] {
    while let [rest @ .., last] = s {
        if matches!(last, b'\r' | b'!' | b' ' | b'\t') {
            s = rest;
        } else {
            break;
        }
    }
    s
}

/// 보낼 프레임을 바이트로 만든다. `newline`은 협상값(`"\n"` 또는 `"!\n"`).
pub fn render(typ: &str, payload: &str, newline: &str) -> Vec<u8> {
    format!("#{typ}:{payload}{newline}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(s: &str) -> Option<Line> {
        LineFramer::new().feed(s.as_bytes()).into_iter().next()
    }

    #[test]
    fn parses_a_plain_frame() {
        let l = one("#NUM:3\n").unwrap();
        assert_eq!(l.typ, "NUM");
        assert_eq!(l.payload, "3");
    }

    #[test]
    fn handles_windows_newline() {
        assert_eq!(one("#SIZE:1024!\n").unwrap().payload, "1024");
    }

    #[test]
    fn strips_junk_before_the_frame() {
        // tmux 상태줄·프롬프트 잔여물이 앞에 붙어도 프레임을 찾아야 한다.
        assert_eq!(one("garbage $ #SUCC:7\r\n").unwrap().payload, "7");
    }

    #[test]
    fn takes_the_last_hash_when_several_appear() {
        assert_eq!(one("#OLD:x #SUCC:9\n").unwrap().typ, "SUCC");
    }

    #[test]
    fn drops_lines_that_are_not_frames() {
        assert!(one("just some output\n").is_none());
        assert!(one("#:missing type\n").is_none());
        assert!(one("#BAD TYPE:x\n").is_none());
    }

    #[test]
    fn reassembles_across_chunks() {
        let mut f = LineFramer::new();
        assert!(f.feed(b"#NA").is_empty());
        assert!(f.feed(b"ME:eJx").is_empty());
        let got = f.feed(b"y\n#SIZE:5\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].typ, "NAME");
        assert_eq!(got[1].payload, "5");
    }

    #[test]
    fn recognises_both_failure_spellings() {
        assert!(one("#FAIL:boom\n").unwrap().is_failure());
        assert!(one("#fail:boom\n").unwrap().is_failure());
        assert!(!one("#SUCC:1\n").unwrap().is_failure());
    }

    #[test]
    fn renders_with_the_negotiated_newline() {
        assert_eq!(render("ACT", "eJx", "\n"), b"#ACT:eJx\n");
        assert_eq!(render("ACT", "eJx", "!\n"), b"#ACT:eJx!\n");
    }
}
