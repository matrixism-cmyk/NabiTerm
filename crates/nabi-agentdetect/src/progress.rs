//! 화면 글에서 **진행률을 읽어 낸다**(배치 AM).
//!
//! ## 왜 필요한가
//!
//! 빌드가 몇 분씩 도는 동안 상태 표시줄에는 "1 shell" 밖에 없다. 얼마나 남았는지 알 수
//! 없어 답답하다(사용자 보고 2026-08-28).
//!
//! 진행률을 보여 주는 길은 이미 끝까지 이어져 있다 — 프로그램이 `OSC 9;4` 를 보내면
//! 상태 표시줄에 뜬다. 문제는 **그것을 보내는 프로그램이 거의 없다**는 것이다.
//! cargo 도 npm 도 보내지 않는다.
//!
//! 그래서 우리가 화면을 읽는다.
//!
//! ## 아무 숫자나 읽으면 안 된다
//!
//! 화면에는 퍼센트처럼 보이는 것이 널려 있다. 디스크 사용량, 시험 통과율, 남의 로그.
//! 그것들을 진행률로 읽으면 **막대가 아무 뜻 없이 춤춘다.** 그러면 안 보여 주느니만 못하다.
//!
//! 그래서 **아는 프로그램의 아는 모양만** 읽는다. 새 도구를 더할 때는 규칙을 하나 더 쓴다.
//!
//! ```text
//! cargo   Building [====>      ] 45/200: syn, quote
//! cmake   [ 45%] Building CXX object ...
//! pytest  tests/test_a.py ....                    [ 45%]
//! docker  Step 3/12 : RUN apt-get update
//! ```

/// 이 줄에서 진행률을 읽는다. 아는 모양이 아니면 `None`.
pub fn read_line(line: &str) -> Option<u8> {
    let s = line.trim();
    cargo(s).or_else(|| docker(s)).or_else(|| bracket_pct(s))
}

/// cargo — `Building [...] 45/200: ...` · `Downloading 3/9 crates`.
fn cargo(s: &str) -> Option<u8> {
    let rest = s.strip_prefix("Building ").or_else(|| s.strip_prefix("Downloading "))?;
    // 막대가 있으면 건너뛴다. 막대 안의 `=` 개수는 세지 않는다 — 칸 수가 창 너비를 타므로
    // 같은 진행률이 창 크기에 따라 다른 값이 된다.
    let rest = match rest.split_once(']') {
        Some((_, after)) => after,
        None => rest,
    };
    ratio(rest.trim())
}

/// docker — `Step 3/12 : ...`.
fn docker(s: &str) -> Option<u8> {
    ratio(s.strip_prefix("Step ")?)
}

/// cmake 는 줄 앞에, pytest 는 줄 끝에 `[ 45%]` 를 둔다.
fn bracket_pct(s: &str) -> Option<u8> {
    let inner = if let Some(r) = s.strip_prefix('[') {
        r.split(']').next()?
    } else if s.ends_with(']') {
        s.rsplit('[').next()?.trim_end_matches(']')
    } else {
        return None;
    };
    let n: u16 = inner.trim().strip_suffix('%')?.trim().parse().ok()?;
    (n <= 100).then_some(n as u8)
}

/// `45/200` 으로 시작하면 백분율로 바꾼다.
fn ratio(s: &str) -> Option<u8> {
    let head: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '/').collect();
    let (a, b) = head.split_once('/')?;
    let (a, b): (u64, u64) = (a.parse().ok()?, b.parse().ok()?);
    if b == 0 || a > b {
        return None;
    }
    Some((a * 100 / b) as u8)
}

/// 새 값을 받아들일까.
///
/// 뒤로 가는 값은 버린다. 여러 도구의 출력이 섞이면 값이 오락가락하는데, 그렇게 흔들리는
/// 막대는 아무것도 알려 주지 않는다.
///
/// 다만 **처음부터 다시 시작하는 것은 받아들인다.** 빌드를 다시 걸었는데 이전 회차의
/// 99% 가 남아 있으면 끝난 줄 알게 된다.
pub fn accept(prev: Option<u8>, next: u8) -> bool {
    match prev {
        None => true,
        Some(p) if next >= p => true,
        // 바닥까지 떨어졌으면 새로 시작한 것이다.
        Some(_) => next <= RESTART,
    }
}

/// 이 값 아래로 떨어지면 새로 시작한 것으로 본다.
const RESTART: u8 = 5;

#[cfg(test)]
mod tests {
    use super::{accept, read_line};

    #[test]
    fn cargo_build_counts_are_read() {
        assert_eq!(read_line("    Building [====>      ] 45/200: syn, quote"), Some(22));
        assert_eq!(read_line("   Downloading 3/9 crates"), Some(33));
    }

    #[test]
    fn the_bar_itself_is_not_counted() {
        // 막대 칸 수는 창 너비를 탄다. 같은 진행률이 창 크기마다 다른 값이 되면 안 된다.
        let narrow = read_line("Building [=>  ] 50/100: a");
        let wide = read_line("Building [=========>          ] 50/100: a");
        assert_eq!(narrow, wide, "창 너비가 진행률을 바꾸면 안 된다");
        assert_eq!(narrow, Some(50));
    }

    #[test]
    fn cmake_puts_it_in_front_and_pytest_at_the_end() {
        assert_eq!(read_line("[ 45%] Building CXX object foo.o"), Some(45));
        assert_eq!(read_line("tests/test_a.py ....                    [ 45%]"), Some(45));
    }

    #[test]
    fn docker_steps_are_read() {
        assert_eq!(read_line("Step 3/12 : RUN apt-get update"), Some(25));
    }

    #[test]
    fn numbers_that_are_not_progress_are_ignored() {
        // 이 시험이 이 파일의 요점이다. 아무 숫자나 읽으면 막대가 뜻 없이 춤춘다.
        assert_eq!(read_line("/dev/sda1  50G  23G  27G  46% /"), None);
        assert_eq!(read_line("커버리지 87% 달성"), None);
        assert_eq!(read_line("총 45/200 건 처리"), None);
        assert_eq!(read_line("error: 3 warnings emitted"), None);
        assert_eq!(read_line(""), None);
    }

    #[test]
    fn impossible_values_are_refused() {
        assert_eq!(read_line("[ 150%] 뭔가"), None);
        assert_eq!(read_line("Step 13/12 : 뒤집힌 값"), None);
        assert_eq!(read_line("Step 3/0 : 0으로 나눌 수 없다"), None);
    }

    #[test]
    fn going_backwards_is_ignored_but_starting_over_is_not() {
        assert!(accept(None, 40), "처음은 무조건 받는다");
        assert!(accept(Some(40), 55), "앞으로 가면 받는다");
        assert!(accept(Some(40), 40), "제자리도 받는다");
        assert!(!accept(Some(90), 40), "뒤로 가면 버린다 — 다른 도구의 출력이 섞인 것이다");
        assert!(accept(Some(99), 2), "바닥까지 떨어졌으면 새로 시작한 것이다");
    }
}
