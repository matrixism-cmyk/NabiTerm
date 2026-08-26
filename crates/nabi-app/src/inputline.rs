//! 지금 **치고 있는 명령줄**을 화면에서 읽어 낸다.
//!
//! 엔터를 가로채 확인하려면 무엇을 치는 중인지 알아야 하는데, 우리는 셸의 입력 버퍼를
//! 볼 수 없다. 볼 수 있는 것은 화면뿐이다. 그래서 **커서가 있는 줄에서 프롬프트 뒤부터
//! 커서 앞까지**를 명령으로 본다.
//!
//! ## 이것은 추측이다
//!
//! 프롬프트는 사람마다 다르고(`$` · `#` · `>` · `%` · 색·아이콘·여러 줄), 줄이 접히면
//! 명령이 두 줄에 걸친다. 그러니 여기서 얻는 것은 **정확한 명령이 아니라 그 근사치**다.
//!
//! 그래서 부르는 쪽은 이 값을 **막는 근거로만** 쓰고 실행하지 않는다. 근사치가 틀려
//! 확인창이 안 뜨면 예전과 같아질 뿐이고, 잘못 떠도 사용자가 "보내기"를 누르면 그대로
//! 나간다. 어느 쪽으로 틀려도 **글자가 안 찍히는 일은 생기지 않는다** — 그것이 이
//! 배치에서 가장 나쁜 결과이기 때문이다.

/// 프롬프트 끝으로 볼 표시들. 뒤에 공백이 오는 것만 본다 — `$HOME`이나 `a>b`를
/// 프롬프트로 오인하지 않기 위해서다.
const PROMPT_MARKS: [&str; 4] = ["$ ", "# ", "> ", "% "];

/// 커서가 있는 줄에서 명령 부분을 뽑는다. 프롬프트를 못 찾으면 줄 전체를 쓴다.
///
/// `col`은 커서의 열(0부터). 커서 **앞까지만** 본다 — 뒤쪽은 아직 치지 않았거나
/// 이전 내용이 남아 있는 자리다.
pub(crate) fn command_at_cursor(line: &str, col: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    let upto: String = chars.iter().take(col.min(chars.len())).collect();
    // 마지막 프롬프트 표시 뒤가 명령이다. 여러 개면 가장 뒤엣것(`user@host:~$ ls | grep $ x`
    // 같은 줄에서 앞엣것을 잡으면 명령이 잘린다).
    let cut = PROMPT_MARKS.iter().filter_map(|m| upto.rfind(m).map(|i| i + m.len())).max();
    match cut {
        Some(i) => upto[i..].trim().to_string(),
        None => upto.trim().to_string(),
    }
}

/// 이 프레임의 입력이 **엔터로 끝나는가** — 그때만 명령이 실행된다.
///
/// 엔터는 CR(`\r`)로 인코딩된다. 앞에 글자가 함께 실려 있어도(붙여넣기·IME 확정 직후)
/// 마지막이 CR이면 이번 프레임에 실행이 일어난다.
pub(crate) fn ends_with_enter(bytes: &[u8]) -> bool {
    bytes.last().is_some_and(|b| *b == b'\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_after_the_prompt_is_taken() {
        let line = "user@host:~$ rm -rf /var";
        assert_eq!(command_at_cursor(line, line.chars().count()), "rm -rf /var");
    }

    /// 커서 **앞까지만** 본다 — 뒤에 남은 옛 글자를 명령으로 세면 안 된다.
    #[test]
    fn only_what_is_before_the_cursor_counts() {
        let line = "$ rm -rf /varOLDLEFTOVER";
        assert_eq!(command_at_cursor(line, 13), "rm -rf /var");
    }

    /// 프롬프트 표시가 여러 번 나오면 **가장 뒤엣것**을 쓴다.
    #[test]
    fn the_last_prompt_mark_wins() {
        let line = "PS C:\\> echo a > b.txt && rm -rf x";
        assert!(command_at_cursor(line, line.chars().count()).contains("rm -rf x"));
    }

    /// 프롬프트를 못 찾으면 줄 전체를 쓴다(우리가 모르는 프롬프트 모양이 많다).
    #[test]
    fn a_line_without_a_known_prompt_is_used_whole() {
        assert_eq!(command_at_cursor("rm -rf /tmp/x", 13), "rm -rf /tmp/x");
    }

    /// `$HOME` 같은 글자를 프롬프트로 오인하지 않는다(뒤에 공백이 있어야 한다).
    #[test]
    fn a_dollar_without_a_space_is_not_a_prompt() {
        assert_eq!(command_at_cursor("echo $HOME", 10), "echo $HOME");
    }

    #[test]
    fn a_한글_line_is_measured_in_characters_not_bytes() {
        let line = "$ 한글 명령 rm -rf x";
        let got = command_at_cursor(line, line.chars().count());
        assert_eq!(got, "한글 명령 rm -rf x", "바이트로 잘라 글자가 깨졌다");
    }

    #[test]
    fn an_empty_or_short_line_is_safe() {
        assert_eq!(command_at_cursor("", 0), "");
        assert_eq!(command_at_cursor("$ ", 99), "", "커서가 줄보다 뒤여도 터지지 않는다");
    }

    #[test]
    fn enter_is_recognised_only_at_the_end() {
        assert!(ends_with_enter(b"\r"));
        assert!(ends_with_enter("ls\r".as_bytes()));
        assert!(!ends_with_enter(b"ls"));
        assert!(!ends_with_enter(b""));
        assert!(!ends_with_enter(b"\rls"), "CR 뒤에 글자가 더 있으면 이번 실행이 아니다");
    }
}
