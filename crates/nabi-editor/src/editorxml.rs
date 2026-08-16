//! nabiPad XML/HTML 정형화(pretty-print) — 태그를 중첩 깊이로 들여쓴다. 휴리스틱(완전 파서 아님).
//! 닫는 태그·자기완결(`/>`)·선언(`<?`)·주석(`<!`)·인라인(`<a>x</a>`)을 구분해 깊이를 조절한다.

/// 태그 사이를 줄바꿈하고 중첩 깊이에 따라 2칸씩 들여쓴다. CDATA/혼합 텍스트는 완벽하지 않다(휴리스틱).
pub fn xml_pretty(t: &str) -> String {
    let split = t.replace("><", ">\n<");
    let mut depth: usize = 0;
    let mut out: Vec<String> = Vec::new();
    for raw in split.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("</") {
            depth = depth.saturating_sub(1); // 닫는 태그는 먼저 내어쓴다.
        }
        out.push(format!("{}{}", "  ".repeat(depth), line));
        // 여는 태그(자기완결·선언·주석·닫는태그·인라인 종료 제외)면 깊이 증가.
        let opening = line.starts_with('<')
            && !line.starts_with("</")
            && !line.starts_with("<?")
            && !line.starts_with("<!")
            && !line.ends_with("/>")
            && line.contains('>')
            && !line.contains("</"); // 한 줄에 <a>text</a> 면 깊이 불변.
        if opening {
            depth += 1;
        }
    }
    out.join("\n")
}

/// XML/HTML 압축(xml_pretty의 역) — 태그 사이 공백/개행만 제거(`>···<` → `><`). 태그 내부 텍스트는 보존.
pub fn xml_minify(t: &str) -> String {
    regex::Regex::new(r">\s+<").map(|re| re.replace_all(t.trim(), "><").into_owned()).unwrap_or_else(|_| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::{xml_minify, xml_pretty};

    #[test]
    fn nests_and_indents() {
        assert_eq!(xml_pretty("<a><b>x</b></a>"), "<a>\n  <b>x</b>\n</a>");
        // 자기완결·선언은 깊이 불변.
        assert_eq!(xml_pretty("<?xml version=\"1.0\"?><r><i/></r>"), "<?xml version=\"1.0\"?>\n<r>\n  <i/>\n</r>");
    }

    #[test]
    fn minify_collapses_between_tags() {
        assert_eq!(xml_minify("<a>\n  <b>x</b>\n</a>"), "<a><b>x</b></a>"); // 태그 사이 공백 제거.
        assert_eq!(xml_minify("<p>hello world</p>"), "<p>hello world</p>"); // 텍스트 내 공백 보존.
        assert_eq!(xml_pretty(&xml_minify("<a>\n <b/>\n</a>")), "<a>\n  <b/>\n</a>"); // 왕복.
    }
}
