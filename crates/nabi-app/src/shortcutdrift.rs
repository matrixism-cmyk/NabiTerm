//! 도움말의 **단축키 표**가 실제 단축키와 어긋나지 않게 붙잡는 시험.
//!
//! ## 왜 시험으로 붙잡나
//!
//! 단축키는 `shortcuts.rs` 가 처리하고, 도움말은 `helppages::KEYS` 라는 **손으로 적은
//! 표**가 보여 준다. 두 벌이므로 반드시 어긋난다 — 이 저장소의 규칙이 그렇게 말하고
//! 있는데(자동 메모리 `feedback-derive-tables-from-source`), 정작 단축키 표에는
//! 검사가 없었다(2026-09-10에 확인).
//!
//! 어긋나면 조용하다. 도움말에 적힌 대로 눌렀는데 아무 일도 없거나, 되는 기능이
//! 도움말에 없어 아무도 모른다. 둘 다 컴파일도 시험도 통과한다.
//!
//! ## 왜 표를 없애고 파생하지 않나
//!
//! 그게 제일 좋다. 그런데 실제 단축키에는 표로 뽑을 수 없는 것이 섞여 있다 —
//! `Alt+1~9`(숫자 아홉 개를 한 줄로), `Ctrl + =  /  -  /  0`(세 키가 한 기능),
//! 그리고 egui 위젯이 자기 안에서 처리하는 것들. 기계로 뽑으면 그 묶음이 풀려
//! 표가 오히려 읽기 어려워진다.
//!
//! 그래서 표는 사람이 읽기 좋게 두되, **빠진 것이 없는지만** 기계가 센다.
//!
//! ## 무엇을 보나
//!
//! `shortcuts.rs` 에서 `consume_key(<수식어>, Key::X)` 를 뽑아, 그 키가 도움말 표의
//! 어느 줄엔가 적혀 있는지 본다. 글자 모양까지 맞추지는 않는다 — 표는 사람이 읽는
//! 것이라 `/` 로 묶거나 `~` 로 줄이는 자유가 있어야 한다.
//!
//! ## 이 파일 전체가 시험 전용이다
//!
//! 그래서 `#![cfg(test)]` 로 통째로 잠근다. 안 그러면 검사기 둘이 서로 반대로 민다 —
//! `pub` 이면 `xtask unused` 가 "밖에서 아무도 안 쓴다"고 하고, 좁히면 컴파일러가
//! `dead_code` 로 짚는다. 파일 하나가 시험만을 위한 것이면 파일째 잠그는 게 답이다.
#![cfg(test)]

/// 단축키를 처리하는 소스. 새 파일이 생기면 여기 한 줄을 더한다.
const SOURCES: &[(&str, &str)] = &[("shortcuts.rs", include_str!("shortcuts.rs"))];

/// 도움말 표가 이 키를 다루지 않아도 되는 것들.
///
/// **예외에는 반드시 까닭을 적는다.** 까닭 없는 예외 목록은 곧 "귀찮아서 넣은 것"으로
/// 채워지고, 그러면 검사가 있으나 마나가 된다.
const EXCUSED: &[(&str, &str)] = &[
    // 붙여넣기 가로채기는 단축키가 아니라 **안전장치**다(여러 줄 붙여넣기 확인).
    // 사용자가 누르는 것은 그냥 Ctrl+V 이고, 그것은 표에 이미 있다.
    ("V", "여러 줄 붙여넣기 확인 - 표의 Ctrl+Shift+V 줄이 같은 키다"),
];

/// 소스에서 `consume_key(.., Key::X)` 의 `X` 를 모은다.
fn keys_in_source(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in src.match_indices("consume_key(") {
        let rest = &src[i..];
        let Some(k) = rest.find("Key::") else { continue };
        // 같은 호출 안에서 찾은 것만 본다(닫는 괄호를 넘어가면 다음 호출이다).
        if rest[..k].contains(')') {
            continue;
        }
        let name: String = rest[k + 5..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// 도움말 표의 한 줄이 이 키를 가리키는가.
///
/// 표는 사람이 읽는 것이라 표기가 자유롭다(`Ctrl+Shift+\  /  -`, `Alt+1~9`,
/// `Ctrl+PgUp  /  PgDn`). 그래서 **키 이름이 그 줄에 나타나는가**만 본다.
fn table_mentions(combo: &str, key: &str) -> bool {
    let c = combo.to_ascii_uppercase();
    match key {
        // egui 의 이름과 사람이 쓰는 이름이 다른 것들.
        "Backtick" => c.contains('`'),
        "Backslash" => c.contains('\\'),
        // egui 는 `=`·`0` 을 여러 이름으로 준다(자판·숫자패드에 따라 다르다).
        "Equals" | "Plus" => c.contains('=') || c.contains('+'),
        "Minus" => c.contains('-'),
        "Num0" => c.contains('0'),
        "PageUp" => c.contains("PGUP"),
        "PageDown" => c.contains("PGDN"),
        "Insert" => c.contains("INSERT"),
        "Escape" => c.contains("ESC"),
        // 숫자 키는 표에서 `Alt+1~9` 처럼 묶는다.
        n if n.len() == 1 && n.chars().all(|x| x.is_ascii_digit()) => {
            c.contains(n) || c.contains('~')
        }
        // 한 글자 키(T·W·…)는 낱말 안에 우연히 들어가지 않게 경계를 본다.
        n if n.len() == 1 => c
            .split(|x: char| !x.is_ascii_alphanumeric())
            .any(|w| w == n || (w.len() > 1 && w.ends_with(n) && w.starts_with("SHIFT+"))),
        n => c.contains(&n.to_ascii_uppercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::{keys_in_source, table_mentions, EXCUSED, SOURCES};
    use crate::helppages::KEYS;

    /// **실제로 되는 단축키는 도움말에 있어야 한다.**
    ///
    /// 없으면 그 기능은 아무도 모른다 — 우리는 단축키를 늘리지 않는 대신 있는 것을
    /// 잘 알리기로 했으므로(규율 §3), 알리지 못하면 만든 뜻이 없다.
    #[test]
    fn 되는_단축키는_도움말에_있다() {
        let mut missing: Vec<String> = Vec::new();
        for (file, src) in SOURCES {
            for key in keys_in_source(src) {
                if EXCUSED.iter().any(|(k, _)| *k == key) {
                    continue;
                }
                if !KEYS.iter().any(|(combo, _)| table_mentions(combo, &key)) {
                    missing.push(format!("{file}: Key::{key}"));
                }
            }
        }
        assert!(missing.is_empty(), "도움말 표에 없는 단축키: {missing:?}");
    }

    /// 소스에서 키를 실제로 뽑아내는가 — 못 뽑으면 위 시험이 **늘 통과한다.**
    ///
    /// 검사기가 아무것도 못 보면서 초록인 것이 가장 나쁘다. 그래서 세어 둔다.
    #[test]
    fn 소스에서_키를_실제로_뽑는다() {
        let n: usize = SOURCES.iter().map(|(_, s)| keys_in_source(s).len()).sum();
        assert!(n >= 10, "단축키를 {n}개밖에 못 찾았다 — 뽑는 규칙이 깨졌다");
    }

    /// 예외에는 까닭이 적혀 있어야 한다.
    #[test]
    fn 예외에는_까닭이_있다() {
        for (key, why) in EXCUSED {
            assert!(why.chars().count() >= 10, "Key::{key} 의 예외에 까닭이 없다");
        }
    }

    /// 표기 판정이 실제로 갈라내는가 — 일부러 어긋난 것을 넣어 본다.
    #[test]
    fn 표기_판정이_갈라낸다() {
        assert!(table_mentions("Ctrl+Shift+T", "T"));
        assert!(table_mentions("Ctrl+`", "Backtick"));
        assert!(table_mentions("Alt+1~9", "1"));
        assert!(table_mentions("Ctrl+PgUp  /  PgDn", "PageDown"));
        assert!(table_mentions("Ctrl + =  /  -  /  0", "Equals"));
        // 낱말 안에 우연히 든 글자에 속으면 안 된다.
        assert!(!table_mentions("Ctrl+Shift+T", "R"), "Ctrl 의 r 에 속았다");
        assert!(!table_mentions("F11", "T"));
    }
}
