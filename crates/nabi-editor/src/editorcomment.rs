//! nabiPad 줄 주석 토글 — 확장자(또는 구문 언어 모드)로 줄 주석 기호를 정하고 선택 줄을 주석/해제한다.
//! 모두 순수 함수(기호 매핑·토글 로직) + 테스트. 선택 영역에 적용(editorctx 우클릭 ▸ 변환).

/// 확장자(소문자 무관)에 맞는 줄 주석 기호. 모르면 "//"(C 계열 기본).
/// 무제목/오탐 문서의 구문 언어 모드(syntax_ext)를 호출측이 확장자 대신 넘길 수 있다.
pub fn line_comment_marker(ext: &str) -> &'static str {
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "py" | "rb" | "sh" | "bash" | "zsh" | "pl" | "r" | "toml" | "yaml" | "yml" | "conf" | "ini" | "ps1"
        | "tcl" | "mk" | "gitignore" | "dockerfile" => "#",
        "lua" | "sql" | "hs" | "elm" | "ada" => "--",
        "lisp" | "clj" | "el" | "scm" | "asm" => ";",
        "vb" | "bas" => "'",
        "bat" | "cmd" => "REM",
        _ => "//", // rs/c/cpp/h/js/ts/go/java/cs/kt/swift/php/scala/dart 등 C 계열.
    }
}

/// 선택(여러 줄)을 줄 주석 토글한다. 비공백 줄이 모두 이미 주석이면 해제, 아니면 주석 처리.
/// 들여쓰기는 보존(주석 기호는 들여쓰기 뒤에 삽입). 빈 줄은 그대로 둔다.
pub fn toggle_line_comment(text: &str, marker: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let has_code = lines.iter().any(|l| !l.trim().is_empty());
    let all_commented = has_code && lines.iter().filter(|l| !l.trim().is_empty()).all(|l| l.trim_start().starts_with(marker));
    lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return (*l).to_string();
            }
            let indent = l.len() - l.trim_start().len();
            let (lead, rest) = l.split_at(indent);
            if all_commented {
                let r = rest.strip_prefix(marker).unwrap_or(rest);
                format!("{lead}{}", r.strip_prefix(' ').unwrap_or(r)) // 기호 뒤 공백 1개 흡수.
            } else {
                format!("{lead}{marker} {rest}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 줄 주석이 없는 언어의 블록 주석 기호. html/xml=`<!-- -->`, css류=`/* */`. 줄 주석이 적합하면 None.
pub fn block_comment_markers(ext: &str) -> Option<(&'static str, &'static str)> {
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" | "xml" | "svg" | "vue" | "xhtml" => Some(("<!--", "-->")),
        "css" | "scss" | "less" => Some(("/*", "*/")),
        _ => None,
    }
}

/// 선택 전체를 블록 주석으로 감싸거나(이미 감싸였으면) 해제한다. 들여쓰기는 기호 안쪽에 포함.
pub fn toggle_block_comment(text: &str, open: &str, close: &str) -> String {
    let t = text.trim();
    if t.starts_with(open) && t.ends_with(close) && t.len() >= open.len() + close.len() {
        t[open.len()..t.len() - close.len()].trim().to_string()
    } else {
        format!("{open} {text} {close}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_toggle_roundtrip() {
        assert_eq!(block_comment_markers("html"), Some(("<!--", "-->")));
        assert_eq!(block_comment_markers("css"), Some(("/*", "*/")));
        assert_eq!(block_comment_markers("rs"), None); // 줄 주석 언어는 블록 아님.
        let on = toggle_block_comment("<div>", "<!--", "-->");
        assert_eq!(on, "<!-- <div> -->");
        assert_eq!(toggle_block_comment(&on, "<!--", "-->"), "<div>"); // 해제.
        assert_eq!(toggle_block_comment("a { }", "/*", "*/"), "/* a { } */");
    }

    #[test]
    fn marker_by_ext() {
        assert_eq!(line_comment_marker("rs"), "//");
        assert_eq!(line_comment_marker("go"), "//");
        assert_eq!(line_comment_marker("py"), "#");
        assert_eq!(line_comment_marker("PY"), "#"); // 대소문자 무관.
        assert_eq!(line_comment_marker("sql"), "--");
        assert_eq!(line_comment_marker("unknown"), "//");
    }

    #[test]
    fn toggle_roundtrip() {
        let src = "  let x = 1;\nfoo();";
        let on = toggle_line_comment(src, "//");
        assert_eq!(on, "  // let x = 1;\n// foo();"); // 들여쓰기 보존.
        assert_eq!(toggle_line_comment(&on, "//"), src); // 다시 토글 → 원복.
    }

    #[test]
    fn blank_lines_kept_and_hash() {
        assert_eq!(toggle_line_comment("a\n\nb", "#"), "# a\n\n# b"); // 빈 줄은 그대로.
        // 일부만 주석이면 전체를 주석 처리(해제 아님).
        assert_eq!(toggle_line_comment("# a\nb", "#"), "# # a\n# b");
    }
}
