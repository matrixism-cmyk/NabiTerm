//! **거대한 파일에서 한 구간만 꺼내 편집한다**(EmEditor 의 Large File Controller 와 같은 답).
//!
//! ## 왜 필요한가
//!
//! 줄 색인이 메모리에 안 들어갈 만큼 큰 파일(수십 GB 로그)은 편집기가 아니라 **읽기 전용
//! 뷰어**로 열린다. 왜 그런지 화면에 말해 주기는 하지만, 사용자가 할 수 있는 일이 없다 —
//! 그 안의 한 토막을 고치고 싶어도 길이 없다.
//!
//! EmEditor 는 이 문제를 "한 번에 한 구간씩 연다"로 푼다. 여기가 그것에 해당한다.
//!
//! ## 되돌려 쓰지는 않는다 (일부러)
//!
//! 꺼낸 글은 **새 문서**가 된다. 원래 파일의 그 자리에 도로 써 넣지 않는다.
//!
//! 길이가 1바이트라도 달라지면 뒤쪽 전부를 밀어야 하는데, 그것은 수십 GB 를 다시 쓰는
//! 일이다. 도중에 멈추면 **파일이 반쯤 망가진 채로 남는다.** 남의 100GB 로그를 그렇게
//! 만들 위험을 지느니, 꺼내서 따로 저장하게 한다. 실제로 필요한 일(한 구간을 뽑아
//! 살펴보고 고쳐서 따로 두기)은 이것으로 다 된다.
//!
//! 언젠가 되돌려 쓰기를 넣는다면 **길이가 같을 때만**(제자리 덮어쓰기) 해야 한다.

/// 한 번에 꺼낼 수 있는 최대 바이트.
///
/// 꺼낸 글은 보통 편집기(rope)로 들어가므로 그쪽이 감당할 만해야 한다. 64MB 면
/// 로그 수십만 줄이고, 사람이 한 번에 볼 일로는 충분히 넘친다.
pub const MAX_SLICE: u64 = 64 * 1024 * 1024;

/// 단추 한 번으로 꺼낼 때의 크기.
///
/// 상한([`MAX_SLICE`])보다 훨씬 작게 잡는다 — 누르자마자 열려야 하고, 대개 원하는 것은
/// "지금 보고 있는 근처"이지 64MB 가 아니다. 더 필요하면 다시 누르면 된다.
pub const DEFAULT_SLICE: u64 = 4 * 1024 * 1024;

/// 어디서 얼마나 읽을지 정한다 — 파일 밖으로 나가지 않게 다듬는다.
///
/// 돌려주는 것은 `(시작, 길이)`. 시작이 파일 끝을 넘으면 길이는 0이다.
pub fn plan(total: u64, want_start: u64, want_len: u64) -> (u64, u64) {
    let start = want_start.min(total);
    let len = want_len.min(MAX_SLICE).min(total - start);
    (start, len)
}

/// 읽어 온 덩어리를 **줄 경계에 맞춘다**.
///
/// 가운데를 그냥 자르면 첫 줄과 끝 줄이 반쪽이 된다. 반쪽 줄은 보기에도 나쁘지만,
/// 그것을 고쳐서 저장하면 **원래 없던 줄이 생긴 것처럼** 보인다.
///
/// * 파일 맨 앞에서 시작했으면 앞을 자르지 않는다(자를 반쪽이 없다).
/// * 파일 끝까지 읽었으면 뒤를 자르지 않는다(마지막 줄에 개행이 없을 수 있다).
/// * 개행이 하나도 없으면 자르지 않는다 — 한 줄짜리 거대 파일에서 전부 버리면 안 된다.
///
/// 돌려주는 것은 `buf` 안의 `(시작, 끝)` 이다.
pub fn line_aligned(buf: &[u8], at_file_start: bool, at_file_end: bool) -> (usize, usize) {
    let start = match at_file_start {
        true => 0,
        // 첫 개행 **다음**부터. 개행이 없으면 통째로 한 줄이므로 그대로 둔다.
        false => memchr::memchr(b'\n', buf).map(|i| i + 1).unwrap_or(0),
    };
    let end = match at_file_end {
        true => buf.len(),
        // 마지막 개행까지 **포함**해서 끝낸다(그 뒤는 다음 줄의 반쪽이다).
        false => memchr::memrchr(b'\n', buf).map(|i| i + 1).unwrap_or(buf.len()),
    };
    // 다듬고 나니 시작이 끝을 넘어섰다면(개행 하나뿐인 아주 짧은 구간) 빈 것으로 둔다.
    match start <= end {
        true => (start, end),
        false => (0, 0),
    }
}

/// 꺼낸 구간에 붙일 이름 — `로그.txt [1,000,000-1,065,536]`.
///
/// 이름에 자리를 적어 두지 않으면, 같은 파일에서 두 군데를 꺼냈을 때 어느 것이
/// 어디였는지 알 수 없다. 저장할 때도 그 이름이 기본값으로 따라간다.
pub fn slice_title(name: &str, start: u64, len: u64) -> String {
    format!("{name} [{}-{}]", group(start), group(start + len))
}

/// 자릿수를 셋씩 끊는다 — 열 자리 넘는 바이트 수는 끊지 않으면 못 읽는다.
fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 파일_밖으로_나가지_않는다() {
        assert_eq!(plan(100, 90, 50), (90, 10), "끝을 넘어 읽으려 한다");
        assert_eq!(plan(100, 200, 10), (100, 0), "시작이 이미 끝 밖이다");
        assert_eq!(plan(100, 0, 10), (0, 10));
    }

    /// 상한을 넘지 않는다 — 넘으면 편집기가 감당 못 한다.
    #[test]
    fn 상한을_넘지_않는다() {
        let huge = 100 * 1024 * 1024 * 1024;
        let (_, len) = plan(huge, 0, huge);
        assert_eq!(len, MAX_SLICE);
    }

    /// **반쪽 줄을 남기지 않는다.** 이것이 이 파일의 핵심이다.
    #[test]
    fn 반쪽_줄을_버린다() {
        // 한글이 든 시험감은 `b"..."` 로 못 쓴다(바이트 리터럴은 ASCII 전용).
        // 그렇다고 한글을 빼지는 않는다 — 우리 사용자의 로그가 한글이다.
        let buf = "쪽\n온전한 줄\n또 한 줄\n반쪽".as_bytes();
        let (a, b) = line_aligned(buf, false, false);
        assert_eq!(&buf[a..b], "온전한 줄\n또 한 줄\n".as_bytes());
    }

    /// 파일 맨 앞이면 앞을 자르지 않는다 — 자를 반쪽이 없다.
    #[test]
    fn 파일_맨_앞은_안_자른다() {
        let buf = "first\nsecond\n반쪽".as_bytes();
        let (a, b) = line_aligned(buf, true, false);
        assert_eq!(&buf[a..b], b"first\nsecond\n");
    }

    /// 파일 끝이면 뒤를 자르지 않는다 — 마지막 줄에 개행이 없을 수 있다.
    #[test]
    fn 파일_끝은_안_자른다() {
        let buf = "쪽\n마지막 줄에는 개행이 없다".as_bytes();
        let (a, b) = line_aligned(buf, false, true);
        assert_eq!(&buf[a..b], "마지막 줄에는 개행이 없다".as_bytes());
    }

    /// **개행이 하나도 없으면 통째로 준다.** 한 줄짜리 거대 파일에서 전부 버리면
    /// "열었는데 비어 있다"가 되고, 그건 고장으로 보인다.
    #[test]
    fn 개행이_없으면_통째로_준다() {
        let buf = b"newlines are nowhere to be found";
        assert_eq!(line_aligned(buf, false, false), (0, buf.len()));
    }

    /// 빈 덩어리를 줘도 죽지 않는다.
    #[test]
    fn 빈_덩어리도_견딘다() {
        assert_eq!(line_aligned(b"", false, false), (0, 0));
        assert_eq!(line_aligned(b"", true, true), (0, 0));
    }

    /// 개행 하나뿐이면 남는 글이 없다 — 그때는 빈 것으로 답한다(뒤집힌 범위 금지).
    #[test]
    fn 개행_하나뿐이면_빈_것() {
        let (a, b) = line_aligned(b"\n", false, false);
        assert!(a <= b, "시작이 끝을 넘었다: {a} > {b}");
        assert_eq!(&b"\n"[a..b], b"");
    }

    #[test]
    fn 이름에_자리를_적는다() {
        assert_eq!(slice_title("로그.txt", 1_000_000, 65_536), "로그.txt [1,000,000-1,065,536]");
        assert_eq!(slice_title("a", 0, 5), "a [0-5]");
    }
}
