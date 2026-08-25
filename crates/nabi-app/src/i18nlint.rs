//! **번역을 거치지 않은 글자가 화면에 나가는 것**을 막는 검사.
//!
//! 우리에게는 이미 반대편 게이트가 있다 — 부르는 키에 번역이 없으면 `keys_exist`가 잡는다.
//! 그런데 **키를 아예 안 쓰고 글자를 그대로 박은 것**은 아무도 안 봤다. 그래서 영어·일본어로
//! 쓰는 사용자가 한국어 알림을 보고 있었다(2026-08-25에 여덟 곳 발견).
//!
//! ## 어떻게 오탐 없이 잡는가
//!
//! "화면에 나가는 문자열"을 기계가 완벽히 알 수는 없다. 그래서 **좁게** 잡는다.
//!
//! * 검사하는 자리는 사용자 알림으로 가는 것이 **확실한 곳**뿐이다(`notify = Some((...))`).
//! * **한글이 든 리터럴만** 본다. 영문은 식별자·기호·서식과 섞여 오탐이 너무 많다.
//! * **주석은 보지 않는다** — 주석의 한글은 오히려 권장한다.
//! * 예외는 그 줄에 `i18n-ok:` 와 이유를 적게 한다. 조용한 예외는 만들지 않는다.
//!
//! 완벽하지 않다. 그래도 **여덟 곳을 놓쳤던 상태보다는 낫고**, 새로 생기는 것은 잡는다.

/// 검사할 파일들을 훑어 문제 줄을 모은다. `(파일, 줄번호, 줄)`.
pub(crate) fn scan(dir: &std::path::Path) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    let mut files: Vec<std::path::PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rs")).collect();
    files.sort();
    for f in files {
        let Ok(src) = std::fs::read_to_string(&f) else { continue };
        let name = f.file_name().unwrap_or_default().to_string_lossy().into_owned();
        // 자기 자신은 건너뛴다 — 이 파일에는 일부러 만든 나쁜 예가 들어 있다.
        if name == "i18nlint.rs" {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if offends(line) {
                out.push((name.clone(), i + 1, line.trim().to_string()));
            }
        }
    }
    out
}

/// 이 줄이 문제인가 — 사용자 알림에 번역 없는 한글 리터럴이 있는가.
pub(crate) fn offends(line: &str) -> bool {
    let code = strip_comment(line);
    if !code.contains("notify = Some((") {
        return false;
    }
    if code.contains("i18n-ok:") || line.contains("i18n-ok:") {
        return false; // 이유를 적은 예외.
    }
    // 원시 문자열(r#"…"#)은 보지 않는다. 이 저장소에서 그건 경로·정규식·시험 데이터이지
    // 사용자에게 나가는 글이 아니고, 따옴표 규칙이 달라 아래 판정이 어긋난다.
    if code.contains("r#\"") {
        return false;
    }
    // `tr(`이 있으면 번역을 거친 것으로 본다(서식 안에 섞인 기호는 문제 삼지 않는다).
    !code.contains("tr(") && has_hangul_literal(&code)
}

/// 줄 주석을 떼어 낸다. 문자열 안의 `//`는 주석이 아니다.
fn strip_comment(line: &str) -> String {
    let (mut in_str, mut prev) = (false, '\0');
    let mut out = String::with_capacity(line.len());
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if !in_str && c == '/' && it.peek() == Some(&'/') {
            break;
        }
        out.push(c);
        prev = c;
    }
    out
}

/// 큰따옴표 안에 한글이 있는가.
fn has_hangul_literal(code: &str) -> bool {
    let mut in_str = false;
    let mut prev = '\0';
    for c in code.chars() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        } else if in_str && is_hangul(c) {
            return true;
        }
        prev = c;
    }
    false
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raw_korean_notification_is_caught() {
        assert!(offends(r#"self.notify = Some((format!("스케줄 등록: {x}"), now));"#));
    }

    /// 번역을 거쳤으면 문제가 아니다.
    #[test]
    fn a_translated_notification_is_fine() {
        assert!(!offends(r#"self.notify = Some((format!("{}", tr(lang, "sched.registered")), now));"#));
    }

    /// **주석의 한글은 문제가 아니다** — 오히려 우리는 주석을 한글로 쓴다.
    #[test]
    fn korean_in_a_comment_is_not_a_problem() {
        assert!(!offends(r#"self.notify = Some((msg, now)); // 사용자에게 알린다"#));
    }

    /// 알림이 아닌 자리는 보지 않는다(오탐을 줄이려고 좁게 잡는다).
    #[test]
    fn other_lines_are_left_alone() {
        assert!(!offends(r#"let s = "한글 문자열";"#));
        assert!(!offends("// 그냥 주석"));
        assert!(!offends(""));
    }

    /// 이유를 적은 예외는 통과한다.
    #[test]
    fn a_documented_exception_passes() {
        assert!(!offends(r#"self.notify = Some((format!("디버그"), now)); // i18n-ok: 개발용"#));
    }

    /// 문자열 안의 `//`를 주석으로 착각하면 안 된다.
    #[test]
    fn a_url_inside_a_string_is_not_a_comment() {
        let line = r#"self.notify = Some((format!("https://x 알림"), now));"#;
        assert!(offends(line), "문자열 안의 //를 주석으로 보고 넘어갔다");
    }

    /// 원시 문자열은 보지 않는다 — 시험 데이터와 정규식이 대부분이다.
    #[test]
    fn raw_strings_are_skipped() {
        assert!(!offends(r##"self.notify = Some((r#"한글"#, now));"##));
    }

    #[test]
    fn hangul_detection_covers_jamo_and_syllables() {
        assert!(is_hangul('가') && is_hangul('힣') && is_hangul('ㄱ'));
        assert!(!is_hangul('a') && !is_hangul('あ') && !is_hangul('中'));
    }

    /// **실제 소스에 문제가 없어야 한다.** 이 시험이 이 파일의 존재 이유다.
    #[test]
    fn the_real_sources_are_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let bad = scan(&dir);
        assert!(
            bad.is_empty(),
            "번역을 거치지 않은 한글이 사용자 알림에 있다:\n{}",
            bad.iter().map(|(f, n, l)| format!("  {f}:{n}  {l}")).collect::<Vec<_>>().join("\n")
        );
    }
}
