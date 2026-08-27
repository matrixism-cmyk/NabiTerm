//! 단축키 표가 **실제 구현과 어긋나지 않게** 지킨다(배치 AA).
//!
//! `helppages::KEYS`는 손으로 적은 표다. 그런데 손으로 관리하는 표는 반드시 어긋난다 —
//! 이 저장소는 설정 색인에서 이미 겪었고(`settingsearch`), 그래서 그쪽은 **소스를 다시 훑어
//! 대조하는 시험**을 두었다. 단축키에는 그것이 없었다.
//!
//! 어긋나면 두 방향 모두 나쁘다:
//!
//! * 구현에는 있는데 표에 없으면 — **있는 기능을 사용자가 모른다.**
//! * 표에는 있는데 구현에 없으면 — 눌러도 아무 일이 없다. 사용자는 자기가 잘못 눌렀다고 생각한다.
//!
//! ## 왜 소스를 글자로 훑는가
//!
//! `consume_key(mods, Key::X)`는 실행 중에만 일어난다. 화면 없이 확인하려면 소스를 읽는 수밖에
//! 없다. 거친 방법이지만 **표를 손으로만 관리하는 것보다는 훨씬 낫다** — 새 단축키를 넣고
//! 표를 잊으면 여기서 걸린다.

// 이 모듈은 **검사기**다 — 실행 중에는 하는 일이 없다. 그래서 시험 빌드에만 넣는다.
// 일반 빌드에 두면 아무도 안 부르는 코드로 남아 clippy 가 짚는데, 그 지적이 맞다.
#![cfg(test)]

/// 소스에서 실제로 처리하는 키 이름을 모은다(`Key::X` → `"X"`).
///
/// 조합키(Ctrl/Shift/Alt)까지 보지는 않는다. 표기 방식이 여러 가지라 글자로 맞추려 들면
/// 시험이 표기 취향에 걸려 자꾸 깨진다. **키가 표에 등장하는지**만 본다 — 그것만으로도
/// "새 단축키를 넣고 도움말을 잊는" 가장 흔한 실수는 잡힌다.
pub(crate) fn keys_in_source(src: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let mut rest = src;
    while let Some(i) = rest.find("Key::") {
        rest = &rest[i + 5..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if !name.is_empty() {
            out.insert(name.to_string());
        }
        rest = &rest[end..];
    }
    out
}

/// 키 이름이 표기 문자열 안에 나타나는가 — `Num3` → `"Alt+1~9"`처럼 **범위 표기**도 받는다.
///
/// 범위를 일일이 적게 하면 표가 길어져 읽기 어려워진다. 사람이 읽기 좋은 표기를 지키면서도
/// 빠뜨림을 잡으려면 이 정도의 느슨함이 필요하다.
pub(crate) fn label_covers(label: &str, key: &str) -> bool {
    let l = label.to_ascii_lowercase();
    let k = key.to_ascii_lowercase();
    if l.contains(&k) {
        return true;
    }
    // `Num3` 같은 숫자 키는 `1~9`·`0`처럼 숫자만 적힌 표기에 들어 있다.
    if let Some(d) = k.strip_prefix("num") {
        return l.contains(d) || l.contains("1~9");
    }
    // `Equals`/`Plus`/`Minus`는 표에 기호로 적힌다.
    match k.as_str() {
        "equals" | "plus" => l.contains('='),
        "minus" => l.contains('-'),
        "backslash" => l.contains('\\'),
        "pageup" | "pagedown" => l.contains("pgup") || l.contains("pgdn"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_keys_in_a_snippet() {
        let src = "consume_key(m, Key::F11); consume_key(m, Key::Num1);";
        let got = keys_in_source(src);
        assert!(got.contains("F11") && got.contains("Num1"));
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn ignores_text_that_merely_mentions_keys() {
        // "Key::" 가 없으면 세지 않는다 — 주석에 적힌 설명까지 세면 시험이 헛돈다.
        assert!(keys_in_source("// F11 로 전체화면").is_empty());
    }

    #[test]
    fn range_labels_cover_their_members() {
        assert!(label_covers("Alt+1~9", "Num3"), "범위 표기가 가운데 숫자를 덮어야 한다");
        assert!(label_covers("Ctrl+Shift+0", "Num0"));
        assert!(label_covers("Ctrl + =  /  -  /  0", "Equals"));
        assert!(label_covers("Ctrl+Shift+\\  /  -", "Backslash"));
        assert!(label_covers("Ctrl+PgUp  /  PgDn", "PageUp"));
        assert!(!label_covers("Ctrl+Shift+T", "F11"));
    }

    /// **이 시험이 이 파일의 핵심이다.** 실제로 처리하는 키가 도움말 표에 다 있는가.
    ///
    /// 새 단축키를 넣고 표를 잊으면 여기서 걸린다. 반대로 표에만 있고 구현이 없는 것은
    /// 이 방식으로 잡히지 않는다 — 그쪽은 눌러 보면 바로 드러나므로 덜 급하다.
    #[test]
    fn every_handled_key_appears_in_the_help_table() {
        let src = include_str!("shortcuts.rs");
        let handled = keys_in_source(src);
        assert!(handled.len() > 20, "훑기가 망가졌다(찾은 키 {}개)", handled.len());
        let labels: Vec<&str> = crate::helppages::KEYS.iter().map(|(k, _)| *k).collect();
        let missing: Vec<&String> = handled
            .iter()
            .filter(|k| !labels.iter().any(|l| label_covers(l, k)))
            .collect();
        assert!(missing.is_empty(), "처리하는데 도움말에 없는 키: {missing:?}");
    }
}
