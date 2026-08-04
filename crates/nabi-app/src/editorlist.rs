//! nabiPad 목록/인용 도구 — 인용부호·따옴표 감싸기·쉼표 결합·마크다운 리스트.
//! 모두 순수 함수. "목록" 서브메뉴로 노출(Markdown/SQL/이메일 작업 벤치마킹).

use nabi_i18n::{tr, Lang};

/// 각 줄 앞에 인용부호 `> ` 추가(이메일/마크다운 인용).
pub(crate) fn quote_lines(t: &str) -> String {
    t.split('\n').map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n")
}

/// 각 줄 앞의 인용부호(`> ` 또는 `>`)를 제거.
pub(crate) fn unquote_lines(t: &str) -> String {
    t.split('\n')
        .map(|l| l.strip_prefix("> ").or_else(|| l.strip_prefix('>')).unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 비어있지 않은 각 줄을 큰따옴표로 감싼다.
pub(crate) fn quote_each(t: &str) -> String {
    map_nonempty(t, |l| format!("\"{l}\""))
}

/// 비어있지 않은 줄을 `, `로 한 줄에 연결(목록화).
pub(crate) fn comma_join(t: &str) -> String {
    t.split('\n').map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(", ")
}

/// 각 줄을 작은따옴표로 감싸 `, `로 연결(SQL IN 목록용).
pub(crate) fn comma_join_quoted(t: &str) -> String {
    t.split('\n')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| format!("'{l}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// 비어있지 않은 줄을 번호 매긴 마크다운 순서 목록(`1. `)으로.
pub(crate) fn md_ordered(t: &str) -> String {
    let mut n = 0;
    t.split('\n')
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                n += 1;
                format!("{n}. {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 비어있지 않은 줄을 마크다운 글머리 목록(`- `)으로.
pub(crate) fn md_bullet(t: &str) -> String {
    map_nonempty(t, |l| format!("- {l}"))
}

/// 각 줄 앞에 우측정렬 줄 번호 + 탭(`nl` 명령식, 빈 줄 포함 — md 순서목록과 구별).
pub(crate) fn number_lines(t: &str) -> String {
    let lines: Vec<&str> = t.split('\n').collect();
    let w = lines.len().to_string().len();
    lines.iter().enumerate().map(|(i, l)| format!("{:>w$}\t{l}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// 줄 앞의 글머리/번호(`- `,`* `,`+ `,`N. `)를 제거.
pub(crate) fn unbullet(t: &str) -> String {
    t.split('\n').map(strip_bullet).collect::<Vec<_>>().join("\n")
}

fn strip_bullet(l: &str) -> &str {
    let s = l.trim_start();
    for p in ["- ", "* ", "+ "] {
        if let Some(r) = s.strip_prefix(p) {
            return r;
        }
    }
    // "N. " 접두 제거.
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        if let Some(r) = s[digits..].strip_prefix(". ") {
            return r;
        }
    }
    l
}

fn map_nonempty(t: &str, f: impl Fn(&str) -> String) -> String {
    t.split('\n').map(|l| if l.trim().is_empty() { l.to_string() } else { f(l) }).collect::<Vec<_>>().join("\n")
}

/// "목록" 서브메뉴 — 클릭한 변환 함수를 돌려준다.
pub(crate) fn list_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    let b = |ui: &mut egui::Ui, k| ui.button(tr(lang, k)).clicked();
    if b(ui, "editor.quote") {
        Some(quote_lines)
    } else if b(ui, "editor.unquote") {
        Some(unquote_lines)
    } else if b(ui, "editor.quoteeach") {
        Some(quote_each)
    } else if b(ui, "editor.commajoin") {
        Some(comma_join)
    } else if b(ui, "editor.commajoinq") {
        Some(comma_join_quoted)
    } else if b(ui, "editor.mdordered") {
        Some(md_ordered)
    } else if b(ui, "editor.mdbullet") {
        Some(md_bullet)
    } else if b(ui, "editor.numberlines") {
        Some(number_lines)
    } else if b(ui, "editor.unbullet") {
        Some(unbullet)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_joining() {
        assert_eq!(quote_lines("a\nb"), "> a\n> b");
        assert_eq!(unquote_lines("> a\n>b\nc"), "a\nb\nc");
        assert_eq!(quote_each("a\n\nb"), "\"a\"\n\n\"b\"");
        assert_eq!(comma_join(" a \nb\n\nc"), "a, b, c");
        assert_eq!(comma_join_quoted("a\nb"), "'a', 'b'");
    }

    #[test]
    fn markdown_lists() {
        assert_eq!(md_ordered("a\n\nb"), "1. a\n\n2. b");
        assert_eq!(md_bullet("a\nb"), "- a\n- b");
        assert_eq!(unbullet("- a\n2. b\n* c"), "a\nb\nc");
    }

    #[test]
    fn numbering() {
        // 10줄 이상이면 폭 2로 우측정렬, 빈 줄도 번호. 탭 구분.
        assert_eq!(number_lines("a\nb"), "1\ta\n2\tb");
        let n = number_lines(&"x\n".repeat(9)); // 10줄(마지막 빈 줄 포함)
        assert!(n.starts_with(" 1\tx"));
        assert!(n.contains("10\t"));
    }
}
