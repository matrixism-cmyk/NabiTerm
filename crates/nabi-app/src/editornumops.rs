//! nabiPad 숫자 산술 — 텍스트 내 모든 정수를 ±1 한다(진수 변환은 editornum). 순수 함수 + 테스트.

/// 텍스트의 모든 정수(연속 숫자)를 ±1 한다. 비숫자 텍스트는 보존, 감소는 0에서 멈춤(음수 미생성).
fn bump_numbers(t: &str, inc: bool) -> String {
    let (mut out, mut num) = (String::with_capacity(t.len()), String::new());
    let flush = |out: &mut String, num: &mut String| {
        if num.is_empty() {
            return;
        }
        match num.parse::<u64>() {
            Ok(n) => out.push_str(&(if inc { n + 1 } else { n.saturating_sub(1) }).to_string()),
            Err(_) => out.push_str(num), // 오버플로 등 → 원본 유지.
        }
        num.clear();
    };
    for c in t.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            flush(&mut out, &mut num);
            out.push(c);
        }
    }
    flush(&mut out, &mut num);
    out
}

/// 모든 정수를 1 증가시킨다(코드/설정 일괄 편집).
pub(crate) fn increment_numbers(t: &str) -> String {
    bump_numbers(t, true)
}

/// 모든 정수를 1 감소시킨다(0에서 멈춤).
pub(crate) fn decrement_numbers(t: &str) -> String {
    bump_numbers(t, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_decrement() {
        assert_eq!(increment_numbers("x9 y10"), "x10 y11"); // 자리수 증가 처리.
        assert_eq!(decrement_numbers("a5 b0"), "a4 b0"); // 0에서 멈춤(음수 미생성).
        assert_eq!(increment_numbers("no digits"), "no digits"); // 숫자 없으면 그대로.
        assert_eq!(increment_numbers("v1.2.3"), "v2.3.4"); // 각 숫자 런 개별 증가.
    }
}
