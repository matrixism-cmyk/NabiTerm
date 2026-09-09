//! **문자열 안에 박힌 진짜 개행**을 찾는다 — 백슬래시-n 이 한 겹 벗겨진 자리.
//!
//! ## 왜 필요한가
//!
//! 힙독(`cat > x.rs <<'EOF'`)이나 파이썬 힙독으로 러스트 소스를 쓰면 백슬래시가 한 겹
//! 벗겨진다. 그래서 이렇게 적은 것이
//!
//! ```text
//! format!("{b}\n{a}")        ← 적으려던 것
//! ```
//!
//! 파일에는 **진짜 개행이 박힌 채** 들어간다(따옴표가 열린 줄 + 다음 줄에서 닫힘).
//!
//! 문제는 **아무도 못 잡는다**는 것이다. 러스트는 문자열 리터럴 안의 진짜 개행을
//! 허용하고, `format!` 의 결과도 똑같다. 컴파일이 되고 시험도 전부 통과한다.
//!
//! 2026-09-07에 새 파일 하나에서 스물두 곳이 그렇게 벌어져 있었고 `cargo test` 는 끝까지
//! 초록이었다. 이 저장소에서 같은 일이 벌어진 것이 **네 번째**이고, 그중 한 번은
//! 이 함정을 설명하는 **문서 자신**이었다.
//!
//! ## 무엇을 세는가
//!
//! 따옴표 문자열이 줄을 넘어가는 자리. 다만 아래 셋은 **정당한 러스트**라 빼야 한다.
//!
//! * **raw 문자열**(`r"…"`, `r#"…"#`) — 여러 줄로 쓰는 것이 원래 쓰임이다.
//!   도움말 본문·JSON 틀이 전부 이 모양이다.
//! * **백슬래시 줄잇기** — 줄 끝의 백슬래시는 "다음 줄로 이어라"라는 뜻이고 개행은
//!   들어가지 않는다. 긴 셸 명령을 그렇게 적는다.
//! * **주석** — 이 결함을 설명해 둔 글이 스스로 걸리곤 했다(`err-swallow` 도 같았다).
//!
//! 이 셋을 빼기 전에는 저장소에서 200건이 나왔고 그중 대부분이 거짓이었다.
//! **거짓이 섞이면 아무도 안 본다** — 그래서 좁게 잡는다.

use std::path::Path;

/// 한 파일에서 문자열이 줄을 넘어간 자리의 줄 번호(1부터).
///
/// 상태를 들고 줄을 따라간다 — 한 줄만 봐서는 raw 문자열 안인지 알 수 없기 때문이다.
pub fn scan(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut raw: Option<usize> = None; // raw 문자열 안이면 여는 `#` 개수.
    let mut in_str = false; // 보통 문자열이 줄을 넘어간 상태.
    for (i, line) in text.lines().enumerate() {
        let was_raw = raw.is_some();
        // raw 문자열은 **떼어 내고** 센다. 한 줄에서 열고 닫는 것(`r"C:\x\"`)도 있어서
        // "이 줄이 raw 를 여는가"만 보면 그 줄의 나머지를 놓친다.
        let code = strip_raw(line, &mut raw);
        if was_raw && code.is_empty() {
            continue;
        }
        let code = strip_line_comment(&code);
        // 이미 넘어간 문자열 안이면 **닫히는 줄은 다시 알리지 않는다.**
        // 한 번 벌어진 문자열을 두 줄로 세면 건수가 배가 되고, 배가 된 목록은 안 읽힌다.
        if in_str {
            in_str = !odd_quotes(&code);
            continue;
        }
        if !odd_quotes(&code) {
            continue;
        }
        in_str = true;
        // 줄 끝 백슬래시는 줄잇기다 — 개행이 들어가지 않으므로 정당하다.
        if code.trim_end().ends_with('\\') {
            continue;
        }
        out.push(i + 1);
    }
    out
}

/// 이 줄에서 raw 문자열(`r"…"`·`r#"…"#`)을 떼어 낸다.
///
/// `raw` 는 줄을 넘어가는 상태다 — 들어갈 때 여는 `#` 개수를 담고, 닫히면 비운다.
/// raw 안에서는 백슬래시가 이스케이프가 아니므로 따옴표를 그냥 세면 어긋난다.
fn strip_raw(line: &str, raw: &mut Option<usize>) -> String {
    let b: Vec<char> = line.chars().collect();
    let (mut out, mut i) = (String::new(), 0usize);
    // 보통 문자열 안인가. 안에서는 `r"` 를 raw 의 시작으로 보면 안 된다 —
    // `b"\x1b[r"` 나 `.expect("rm -r")` 이 그렇게 걸렸다(둘 다 거짓이었다).
    let (mut in_q, mut esc) = (false, false);
    while i < b.len() {
        if let Some(h) = *raw {
            match close_at(&b, i, h) {
                Some(k) => {
                    *raw = None;
                    i = k;
                }
                None => return out, // 이 줄은 통째로 raw 안이다.
            }
            continue;
        }
        if esc {
            esc = false;
            out.push(b[i]);
            i += 1;
            continue;
        }
        match b[i] {
            '\\' if in_q => esc = true,
            '"' => in_q = !in_q,
            _ => {}
        }
        // `r`·`r#`·`r##` … 뒤에 따옴표가 오면 raw 의 시작이다. 앞 글자가 이름의 일부면
        // (`for` 의 r 처럼) 아니고, 문자열 안이면 그냥 글자다.
        if !in_q && b[i] == 'r' && !i.checked_sub(1).is_some_and(|k| is_ident(b[k])) {
            let mut j = i + 1;
            while b.get(j) == Some(&'#') {
                j += 1;
            }
            if b.get(j) == Some(&'"') {
                *raw = Some(j - i - 1);
                i = j + 1;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// `i` 이후에서 `"` + `#`×h 로 닫히는 자리(닫힌 **뒤** 색인). 없으면 None.
fn close_at(b: &[char], i: usize, h: usize) -> Option<usize> {
    (i..b.len()).find(|&k| {
        b[k] == '"' && b[k + 1..].iter().take(h).filter(|c| **c == '#').count() == h
    })
    .map(|k| k + 1 + h)
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 줄 주석을 떼어 낸다. 문자열 안의 `//` 는 주석이 아니므로 따옴표를 세어 가며 본다.
fn strip_line_comment(line: &str) -> String {
    let b: Vec<char> = line.chars().collect();
    let (mut q, mut esc) = (false, false);
    for i in 0..b.len() {
        if esc {
            esc = false;
            continue;
        }
        match b[i] {
            '\\' => esc = true,
            '"' => q = !q,
            '/' if !q && b.get(i + 1) == Some(&'/') => return b[..i].iter().collect(),
            _ => {}
        }
    }
    line.to_string()
}

/// 이스케이프와 문자 리터럴을 뺀 따옴표 수가 홀수인가 = 문자열이 줄을 넘어갔다.
fn odd_quotes(code: &str) -> bool {
    let b: Vec<char> = code.chars().collect();
    let (mut n, mut esc, mut i) = (0usize, false, 0usize);
    while i < b.len() {
        if esc {
            esc = false;
            i += 1;
            continue;
        }
        match b[i] {
            '\\' => esc = true,
            // 문자 리터럴 안의 따옴표(`'"'`)는 문자열이 아니다.
            '\'' if b.get(i + 1) == Some(&'"') && b.get(i + 2) == Some(&'\'') => i += 2,
            '"' => n += 1,
            _ => {}
        }
        i += 1;
    }
    n % 2 == 1
}

pub fn run() -> std::process::ExitCode {
    let mut files = crate::rswalk::rust_files(Path::new("crates"));
    files.extend(crate::rswalk::rust_files(Path::new("xtask")));
    let mut hits: Vec<(String, usize)> = Vec::new();
    for (path, text) in &files {
        // 벤더 크레이트는 우리가 쓴 것이 아니다.
        if path.replace('\\', "/").contains("/vendor/") {
            continue;
        }
        for n in scan(text) {
            hits.push((path.clone(), n));
        }
    }
    println!("검사 {} 파일 · 줄을 넘어간 문자열 {}", files.len(), hits.len());
    for (f, n) in &hits {
        println!("fail: {f}:{n} = 문자열이 줄을 넘어간다(백슬래시-n 이 벗겨졌을 수 있다)");
    }
    if hits.is_empty() {
        return std::process::ExitCode::SUCCESS;
    }
    // **경고가 아니라 막는다.** `unused`·`err-swallow` 는 경고만 하는데(막으면 예외
    // 목록이 생기고 예외 목록은 곧 낡는다), 이것은 다르다 — 일부러 여러 줄로 쓰고 싶으면
    // 러스트에 이미 출구가 있다. 여는 따옴표 앞에 `r` 한 글자를 붙이면 된다.
    //
    // 2026-09-09에 저장소를 처음 훑었을 때 마흔여덟 곳이 걸렸고, 그중 마흔한 곳이
    // 진짜였다(나머지 일곱은 시험용 PEM 키라 raw 문자열로 바꿨다). 그 마흔한 곳은
    // 컴파일도 시험도 통과하고 있었다 — 막지 않으면 다시 쌓인다.
    println!("→ 일부러 여러 줄로 쓴 것이면 여는 따옴표 앞에 `r` 을 붙일 것(raw 문자열)");
    println!("→ 아니면 백슬래시-n 이 벗겨진 것이다. `Write` 도구로 그 파일을 다시 쓸 것");
    std::process::ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::{odd_quotes, scan, strip_line_comment, strip_raw};

    /// 벗겨진 백슬래시-n = 문자열이 줄을 넘어간다. 이것이 잡으려는 바로 그 모양이다.
    ///
    /// **여는 줄만 알린다** — 닫는 줄까지 세면 건수가 배가 되고, 배가 된 목록은 안 읽힌다.
    #[test]
    fn 줄을_넘어간_문자열을_잡는다() {
        let src = "fn a() {\n    let x = format!(\"{b}\n{a}\");\n}\n";
        assert_eq!(scan(src), vec![2]);
    }

    /// 여러 곳이 벌어져 있으면 각각 한 번씩 알린다(닫힌 뒤 상태가 풀려야 한다).
    #[test]
    fn 여러_곳이면_각각_한_번씩() {
        let src = "let a = \"x\ny\";\nlet ok = \"fine\";\nlet b = \"p\nq\";\n";
        assert_eq!(scan(src), vec![1, 4]);
    }

    /// 제대로 쓴 것은 걸리지 않는다.
    #[test]
    fn 멀쩡한_것은_안_걸린다() {
        let src = "let x = format!(\"{b}\\n{a}\");\nlet y = \"ok\";\n";
        assert!(scan(src).is_empty());
    }

    /// raw 문자열은 여러 줄이 원래 쓰임이다 — 도움말 본문이 전부 이 모양이다.
    #[test]
    fn raw_문자열은_넘어가도_된다() {
        let src = "const G: &str = r#\"# 제목\n본문\n\"#;\nlet z = \"ok\";\n";
        assert!(scan(src).is_empty(), "{:?}", scan(src));
        // 한 줄에서 열고 닫는 raw 는 **떼어 내고** 나머지를 센다. 실제로 이것 때문에
        // `spawn.rs` 의 `r"\microsoft\windowsapps\"` 가 거짓으로 걸렸다 —
        // raw 안의 백슬래시를 이스케이프로 읽어 따옴표 하나를 놓쳤다.
        let mut raw = None;
        assert_eq!(strip_raw(r#"let p = r"C:\t\"; ok"#, &mut raw), "let p = ; ok");
        assert!(raw.is_none(), "한 줄에서 닫혔는데 열린 채로 뒀다");
        assert!(scan("let p = r\"C:\\t\\\";\nlet q = \"ok\";\n").is_empty());
        // `#` 개수가 맞아야 닫힌다 — `\"#` 하나로 `r##\"…\"##` 를 닫으면 안 된다.
        let mut raw2 = None;
        strip_raw("let g = r##\"열고 \"# 안 닫힘", &mut raw2);
        assert_eq!(raw2, Some(2), "해시 두 개짜리가 하나로 닫혔다");
    }

    /// **문자열 안의 `r` 은 raw 의 시작이 아니다.** 이스케이프 문자와 짧은 낱말에서
    /// 실제로 걸렸다 — `b"\x1b[r"`(escape 시퀀스)와 `.expect("rm -r")`.
    #[test]
    fn 문자열_안의_r_에_안_속는다() {
        assert!(scan("m.process(b\"\\x1b[r\");\nlet ok = \"fine\";\n").is_empty());
        assert!(scan("x.expect(\"rm -r\"); // 지운다\nlet ok = \"fine\";\n").is_empty());
    }

    /// 줄 끝 백슬래시는 줄잇기다 — 개행이 들어가지 않는다.
    #[test]
    fn 줄잇기는_안_걸린다() {
        let src = "let c = \"first part \\\n    second part\";\n";
        assert!(scan(src).is_empty(), "{:?}", scan(src));
    }

    /// 문자 리터럴 안의 따옴표에 속지 않는다.
    #[test]
    fn 문자_리터럴에_안_속는다() {
        assert!(!odd_quotes("let c = '\"';"));
        assert!(!odd_quotes("s.trim_matches('\"')"));
    }

    /// 이스케이프한 따옴표는 세지 않는다.
    #[test]
    fn 이스케이프한_따옴표는_안_센다() {
        assert!(!odd_quotes("let d = \"he said \\\"hi\\\"\";"));
    }

    /// 주석은 코드가 아니다 — 이 결함을 설명한 글이 스스로 걸리면 안 된다.
    #[test]
    fn 주석은_보지_않는다() {
        assert_eq!(strip_line_comment("let a = 1; // 따옴표 \" 하나"), "let a = 1; ");
        assert!(scan("// 여기에 따옴표 \" 하나\nlet a = 1;\n").is_empty());
        // 문자열 안의 // 는 주석이 아니다.
        assert_eq!(strip_line_comment("let u = \"http://x\";"), "let u = \"http://x\";");
    }
}
