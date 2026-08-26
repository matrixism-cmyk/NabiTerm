//! **글에서 비밀로 보이는 줄을 찾는다** — 저장하거나 올리기 전에 알아채라고.
//!
//! 규칙은 새로 만들지 않는다. 배치 T의 가리기 규칙(`crate::redact`)이 **바꾼 줄**이 곧
//! "비밀이 든 줄"이다. 규칙이 하나면 가리는 것과 찾는 것이 어긋날 수 없다.
//!
//! ## 막지 않는다
//!
//! 배치 T에서 이 기능을 미룬 까닭이 "오탐이 곧바로 사용자를 귀찮게 한다"였다. 그래서
//! **저장·업로드를 가로막지 않는다.** 편집기에서는 눌러서 찾는 명령이고, 업로드에서는
//! 몇 줄이 걸렸는지 알리는 한 줄이다. 사용자가 판단한다.
//!
//! ## 넘겨보는 양을 제한한다
//!
//! 수백 MB 문서를 통째로 훑으면 화면이 멈춘다. 앞쪽 일부만 본다 — 붙여넣은 키는 보통
//! 파일 앞이나 설정 블록에 있고, 무엇보다 **못 본 부분이 있다고 말한다.**

/// 훑을 최대 줄 수. 넘으면 훑기를 멈추고 그 사실을 알린다.
pub(crate) const MAX_LINES: usize = 20_000;

/// 찾은 결과.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Found {
    /// 걸린 줄 번호(1부터).
    pub lines: Vec<usize>,
    /// 상한에 걸려 뒷부분을 못 봤나.
    pub truncated: bool,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// 글에서 비밀로 보이는 줄을 찾는다.
///
/// 판단 기준은 **가리기 규칙이 그 줄을 바꾸는가** 하나다.
pub(crate) fn scan(text: &str) -> Found {
    let mut out = Found::default();
    for (i, l) in text.lines().enumerate() {
        if i >= MAX_LINES {
            out.truncated = true;
            break;
        }
        if crate::redact::line_full(l) != l {
            out.lines.push(i + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{scan, MAX_LINES};

    #[test]
    fn a_clean_document_has_nothing() {
        let t = "fn main() {\n    println!(\"안녕\");\n}\n";
        assert!(scan(t).is_empty());
    }

    /// 줄 번호는 **1부터** — 편집기가 그렇게 센다.
    #[test]
    fn the_line_numbers_start_at_one() {
        let t = "ok\napi_key=sk-live-abcdefghijk\nok";
        assert_eq!(scan(t).lines, vec![2]);
    }

    #[test]
    fn several_lines_are_all_reported() {
        let t = "password: x\nok\n--token abcdefghijklmnop";
        assert_eq!(scan(t).lines.len(), 2);
    }

    /// **못 본 부분이 있으면 말한다** — 조용히 끊으면 "없다"로 읽힌다.
    #[test]
    fn a_huge_document_says_it_was_cut_short() {
        let t = "ok\n".repeat(MAX_LINES + 10);
        let f = scan(&t);
        assert!(f.truncated, "상한에 걸렸는데 말하지 않았다");
        assert!(f.is_empty(), "멀쩡한 줄을 잡았다");
    }

    /// 상한 안쪽이면 끊겼다고 하지 않는다.
    #[test]
    fn a_normal_document_is_not_marked_as_cut() {
        assert!(!scan("a\nb\nc").truncated);
    }

    /// 가리기 규칙과 **같은 답**을 낸다 — 규칙이 하나라는 것이 이 기능의 전제다.
    #[test]
    fn it_agrees_with_the_redaction_rules() {
        for l in ["mysql -pS3cr3t!", "export TOKEN=ghp_abcdefghijklmnopqrst"] {
            assert_ne!(crate::redact::line_full(l), l, "규칙이 안 잡으면 이 시험이 틀렸다");
            assert_eq!(scan(l).lines, vec![1]);
        }
    }
}
