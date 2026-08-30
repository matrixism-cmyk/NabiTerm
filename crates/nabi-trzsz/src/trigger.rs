//! 트리거 감지 — 원격이 `trz`/`tsz`를 실행하면 표준출력에 나오는 매직 문자열을 잡아낸다.
//!
//! ```text
//! ::TRZSZ:TRANSFER:<MODE>:<VERSION>[:<UNIQUE_ID>][:<TUNNEL_PORT>]
//! ```
//!
//! 어려운 점은 형식이 아니라 **스트림**이다. 출력은 아무 데서나 잘려 들어오므로 매직이
//! 두 청크에 걸칠 수 있다. 그렇다고 매 청크를 통째로 붙들면 터미널이 끊겨 보인다.
//! 그래서 "매직의 접두사가 될 수 있는 꼬리"만 붙들고 나머지는 즉시 흘려보낸다.

use std::collections::VecDeque;

const MAGIC: &[u8] = b"::TRZSZ:TRANSFER:";
/// 매직 뒤 필드(모드·판·ID·포트)가 아무리 길어도 이 안에 끝난다. 넘으면 트리거가 아니다.
const FIELD_MAX: usize = 96;
/// 가짜 트리거 판정에 살펴볼 범위(원본도 매직 뒤 40바이트부터 본다).
const FAKE_SCAN: usize = 220;
/// 같은 트리거를 두 번 처리하지 않기 위해 기억하는 ID 개수.
const SEEN_MAX: usize = 128;

/// 전송 방향. 이름은 **원격 기준**이 아니라 **우리 기준**으로 읽는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `tsz <파일>` — 원격이 보낸다 = 우리가 받는다.
    Download,
    /// `trz` — 원격이 받는다 = 우리가 보낸다.
    Upload,
    /// `trz -d` — 디렉터리까지 허용하는 업로드.
    UploadDir,
    /// 원격이 CFG의 `client_files`로 **올릴 로컬 파일을 지정**한다. 위험해서 기본 차단이다.
    UploadSpecified,
}

impl Mode {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'S' => Some(Self::Download),
            b'R' => Some(Self::Upload),
            b'D' => Some(Self::UploadDir),
            b'F' => Some(Self::UploadSpecified),
            _ => None,
        }
    }

    /// 우리가 파일을 보내는 쪽인가.
    pub fn is_upload(self) -> bool {
        !matches!(self, Self::Download)
    }
}

/// 감지된 트리거.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub mode: Mode,
    pub version: (u32, u32, u32),
    pub unique_id: String,
    /// 원격이 Windows다 — 줄바꿈이 `!\n`이고 바이너리 모드를 쓸 수 없다.
    pub win_server: bool,
    /// 0이 아니면 원격이 TCP 터널을 제안한 것. 우리는 쓰지 않는다(폐쇄망·방화벽).
    pub tunnel_port: u16,
}

/// 한 번 훑은 결과.
#[derive(Debug, Default)]
pub struct Scanned {
    /// 터미널 화면(VT 모델)에 그대로 보낼 바이트.
    pub display: Vec<u8>,
    /// 잡아낸 트리거.
    pub trigger: Option<Trigger>,
    /// 트리거 **뒤에** 붙어 온 바이트 — 전송 세션의 것이다(화면에 보이면 안 된다).
    pub rest: Vec<u8>,
}

/// 출력 스트림에서 트리거를 찾는 스캐너(pane 하나에 하나).
#[derive(Default)]
pub struct TriggerScanner {
    /// 매직의 접두사가 될 수 있어 아직 못 내보낸 꼬리.
    held: Vec<u8>,
    /// 이미 처리한 트리거 ID — 중계 구성에서 같은 트리거가 두 번 보인다.
    seen: VecDeque<String>,
    /// 트리거를 잡은 뒤에는 멈춘다 — 그 다음 바이트는 전송 세션의 것이지 화면의 것이 아니다.
    /// 이렇게 해 두면 호출자가 `resume()`을 잊어도 프로토콜 바이트가 화면에 새지 않는다.
    suspended: bool,
}

impl TriggerScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// 전송이 끝났다 — 다시 화면으로 흘려보낸다.
    pub fn resume(&mut self) {
        self.suspended = false;
    }

    /// 출력 청크를 넣고, 화면에 보낼 바이트와(있다면) 트리거를 받는다.
    pub fn feed(&mut self, chunk: &[u8]) -> Scanned {
        if self.suspended {
            return Scanned { rest: chunk.to_vec(), ..Scanned::default() };
        }
        let mut buf = std::mem::take(&mut self.held);
        buf.extend_from_slice(chunk);
        let mut out = Scanned::default();
        let mut from = 0usize;

        while let Some(rel) = find(&buf[from..], MAGIC) {
            let idx = from + rel;
            match self.parse_at(&buf, idx) {
                Parsed::Incomplete => {
                    // 필드가 아직 다 안 왔다 — 매직부터 붙들고 앞부분만 내보낸다.
                    out.display.extend_from_slice(&buf[..idx]);
                    self.held = buf[idx..].to_vec();
                    return out;
                }
                Parsed::NotTrigger(end) => {
                    // 가짜거나 이미 본 트리거 — 화면에 그대로 흘린다(로그일 수도 있다).
                    from = end;
                }
                Parsed::Found(trigger, end) => {
                    // 매직 문자열 자체는 화면에서 지운다. 사용자에게는 진행률 UI가 보인다.
                    out.display.extend_from_slice(&buf[..idx]);
                    out.rest = buf[end..].to_vec();
                    out.trigger = Some(trigger);
                    self.suspended = true;
                    return out;
                }
            }
        }

        // 트리거 없음. 매직의 접두사가 될 수 있는 꼬리만 남기고 전부 내보낸다.
        let keep = prefix_tail(&buf, MAGIC);
        out.display.extend_from_slice(&buf[..buf.len() - keep]);
        self.held = buf[buf.len() - keep..].to_vec();
        out
    }

    /// 붙들고 있던 바이트를 돌려주고 스캐너를 비운다(전송이 끝나 정리할 때).
    pub fn flush(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.held)
    }

    fn parse_at(&mut self, buf: &[u8], idx: usize) -> Parsed {
        let start = idx + MAGIC.len();
        // 필드 영역: 모드 글자 + ':' + 판·ID·포트(숫자·점·콜론).
        let mut end = start;
        while end < buf.len() && is_field(buf[end]) && end - start < FIELD_MAX {
            end += 1;
        }
        if end == buf.len() && end - start < FIELD_MAX {
            return Parsed::Incomplete; // 더 올 수도 있다.
        }
        let Some(t) = parse_fields(&buf[start..end]) else {
            return Parsed::NotTrigger(end.max(start + 1));
        };
        // 가짜 판정: 트리거 뒤에 이런 낱말이 붙어 있으면 누가 로그를 화면에 뿌린 것이다.
        let tail_to = (idx + FAKE_SCAN).min(buf.len());
        if idx + 40 < tail_to && looks_like_log(&buf[idx + 40..tail_to]) {
            return Parsed::NotTrigger(end);
        }
        if self.is_repeat(&t.unique_id) {
            return Parsed::NotTrigger(end);
        }
        Parsed::Found(t, end)
    }

    /// 같은 ID를 이미 봤는가. ID가 비었거나 너무 짧으면 판단하지 않는다(오탐이 더 나쁘다).
    fn is_repeat(&mut self, id: &str) -> bool {
        if id.len() <= 6 {
            return false;
        }
        if self.seen.iter().any(|s| s == id) {
            return true;
        }
        if self.seen.len() >= SEEN_MAX {
            self.seen.pop_front();
        }
        self.seen.push_back(id.to_owned());
        false
    }
}

enum Parsed {
    /// 필드가 아직 덜 왔다.
    Incomplete,
    /// 트리거가 아니다(끝 위치 — 여기서부터 다시 찾는다).
    NotTrigger(usize),
    Found(Trigger, usize),
}

fn is_field(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'.' || b == b':'
}

/// `S:1.1.8:1755780000000:0` 꼴을 뜯는다.
fn parse_fields(s: &[u8]) -> Option<Trigger> {
    let mut it = s.split(|&b| b == b':');
    let mode = Mode::from_byte(*it.next()?.first()?)?;
    let version = parse_version(it.next()?)?;
    let unique_id = it.next().map_or(String::new(), |b| String::from_utf8_lossy(b).into_owned());
    if !unique_id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let tunnel_port = it
        .next()
        .and_then(|b| std::str::from_utf8(b).ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // 13자리 ID가 "10"으로 끝나면 원격이 Windows다(trzsz가 ID 끝 두 자리에 실어 보낸다).
    let win_server = unique_id == "1" || (unique_id.len() == 13 && unique_id.ends_with("10"));
    Some(Trigger { mode, version, unique_id, win_server, tunnel_port })
}

fn parse_version(s: &[u8]) -> Option<(u32, u32, u32)> {
    let text = std::str::from_utf8(s).ok()?;
    let mut it = text.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

/// 트리거처럼 생겼지만 실은 로그·안내문인 경우.
fn looks_like_log(s: &[u8]) -> bool {
    const MARKS: [&[u8]; 5] = [b"#CFG:", b"Saved", b"Cancelled", b"Stopped", b"Interrupted"];
    MARKS.iter().any(|m| find(s, m).is_some())
}

/// `hay` 안의 `needle` 첫 위치.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `buf`의 꼬리 중 `needle`의 **진접두사**인 가장 긴 길이 — 이만큼만 붙들면 된다.
fn prefix_tail(buf: &[u8], needle: &[u8]) -> usize {
    let max = needle.len().saturating_sub(1).min(buf.len());
    (1..=max).rev().find(|&n| buf[buf.len() - n..] == needle[..n]).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trig(s: &str) -> Option<Trigger> {
        TriggerScanner::new().feed(s.as_bytes()).trigger
    }

    #[test]
    fn parses_each_mode() {
        assert_eq!(trig("::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n").unwrap().mode, Mode::Download);
        assert_eq!(trig("::TRZSZ:TRANSFER:R:1.1.8:1755780000000\n").unwrap().mode, Mode::Upload);
        assert_eq!(trig("::TRZSZ:TRANSFER:D:1.1.8:1755780000000\n").unwrap().mode, Mode::UploadDir);
        assert_eq!(
            trig("::TRZSZ:TRANSFER:F:1.1.8:1755780000000\n").unwrap().mode,
            Mode::UploadSpecified
        );
        assert!(trig("::TRZSZ:TRANSFER:X:1.1.8:1755780000000\n").is_none(), "모르는 모드는 거절");
    }

    #[test]
    fn reads_version_id_and_port() {
        let t = trig("::TRZSZ:TRANSFER:S:1.1.8:1755780000000:8080\n").unwrap();
        assert_eq!(t.version, (1, 1, 8));
        assert_eq!(t.unique_id, "1755780000000");
        assert_eq!(t.tunnel_port, 8080);
        assert!(!t.win_server);
    }

    #[test]
    fn detects_windows_remote() {
        assert!(trig("::TRZSZ:TRANSFER:S:1.1.8:1755780000010\n").unwrap().win_server);
        assert!(trig("::TRZSZ:TRANSFER:S:1.1.8:1\n").unwrap().win_server);
        assert!(!trig("::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n").unwrap().win_server);
    }

    /// 스트림은 아무 데서나 잘린다 — 한 바이트씩 먹여도 결과가 같아야 한다.
    #[test]
    fn survives_any_chunk_split() {
        let input = b"hello\n::TRZSZ:TRANSFER:R:1.1.8:1755780000000\nAFTER";
        let mut sc = TriggerScanner::new();
        let (mut display, mut found, mut rest) = (Vec::new(), None, Vec::new());
        for i in 0..input.len() {
            let r = sc.feed(&input[i..=i]);
            display.extend_from_slice(&r.display);
            rest.extend_from_slice(&r.rest);
            if r.trigger.is_some() {
                found = r.trigger;
            }
        }
        assert_eq!(found.unwrap().mode, Mode::Upload);
        assert_eq!(display, b"hello\n", "매직 앞 내용은 그대로 보여야 한다");
        assert_eq!(rest, b"\nAFTER", "트리거 줄을 끝내는 줄바꿈부터가 전송 세션 몫이다");
        assert!(!display.windows(5).any(|w| w == b"TRZSZ"), "매직이 화면에 새면 안 된다");
    }

    /// 트리거 뒤에는 스스로 멈춘다 — 호출자가 실수해도 프로토콜 바이트가 화면에 안 샌다.
    #[test]
    fn suspends_until_resumed() {
        let mut sc = TriggerScanner::new();
        assert!(sc.feed(b"::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n").trigger.is_some());
        // 멈췄는지는 **하는 짓으로** 확인한다 — 다음 줄이 화면으로 안 가는 것이 곧 그것이다.
        // 상태를 묻는 함수를 따로 두었더니 이 시험 말고는 아무도 부르지 않았다.
        let r = sc.feed(b"#CFG:eJx\n");
        assert!(r.display.is_empty(), "전송 중에는 화면에 아무것도 가지 않는다");
        assert_eq!(r.rest, b"#CFG:eJx\n");
        sc.resume();
        assert_eq!(sc.feed(b"$ ").display, b"$ ");
    }

    #[test]
    fn keeps_bytes_after_trigger_for_the_session() {
        let r = TriggerScanner::new().feed(b"x::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n#CFG:abc\n");
        assert_eq!(r.display, b"x");
        assert_eq!(r.rest, b"\n#CFG:abc\n");
    }

    #[test]
    fn ignores_log_lines_that_merely_mention_the_magic() {
        let s = "::TRZSZ:TRANSFER:S:1.1.8:1755780000000 ... Cancelled by user, nothing was Saved\n";
        let r = TriggerScanner::new().feed(s.as_bytes());
        assert!(r.trigger.is_none());
        assert_eq!(r.display, s.as_bytes(), "가짜면 화면에 그대로 보여야 한다");
    }

    #[test]
    fn ignores_the_same_trigger_twice() {
        let mut sc = TriggerScanner::new();
        let line = b"::TRZSZ:TRANSFER:S:1.1.8:1755780000000\n";
        assert!(sc.feed(line).trigger.is_some());
        sc.resume();
        assert!(sc.feed(line).trigger.is_none(), "중계 구성에서 두 번 잡으면 안 된다");
    }

    #[test]
    fn holds_only_a_possible_prefix() {
        let mut sc = TriggerScanner::new();
        // 매직 접두사가 될 수 있는 꼬리만 붙든다.
        let r = sc.feed(b"plain text::TRZSZ:TRA");
        assert_eq!(r.display, b"plain text");
        // 나머지가 오면 이어붙여 판정한다.
        let r = sc.feed(b"NSFER:S:1.1.8:1755780000000\n");
        assert!(r.trigger.is_some());
    }

    #[test]
    fn ordinary_output_is_not_delayed() {
        let mut sc = TriggerScanner::new();
        let r = sc.feed(b"$ ls -al\r\ntotal 8\r\n");
        assert_eq!(r.display, b"$ ls -al\r\ntotal 8\r\n", "평범한 출력은 한 바이트도 붙들지 않는다");
        assert!(r.trigger.is_none());
    }

    #[test]
    fn prefix_tail_finds_longest_proper_prefix() {
        assert_eq!(prefix_tail(b"abc::TRZ", MAGIC), 5);
        assert_eq!(prefix_tail(b"abc:", MAGIC), 1);
        assert_eq!(prefix_tail(b"abcd", MAGIC), 0);
        assert_eq!(prefix_tail(b"", MAGIC), 0);
    }
}
