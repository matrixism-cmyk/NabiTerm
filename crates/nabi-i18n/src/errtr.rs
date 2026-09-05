//! **코드로 온 오류를 사람 말로 바꾼다**(T8-1).
//!
//! 아래쪽 크레이트는 오류를 코드로만 올려 보낸다(`nabi_error::Coded`). 어떤 말로 보여
//! 줄지는 여기서 정한다 — 화면은 사용자 언어로, `nabi cli` 는 영어로.
//!
//! ## `nabi cli` 가 영어인 까닭
//!
//! 그 출력은 **AI 에이전트가 읽는다.** 설명서(`agentguide`)가 영어로 적혀 있고, 에이전트는
//! 그 문구를 보고 무엇이 잘못됐는지 판단한다. 화면 언어가 한국어라고 해서 `nabi cli` 의
//! 오류까지 한국어로 나가면, 에이전트는 자기가 아는 어떤 오류와도 맞지 않는 글을 받는다.
//!
//! 사람이 그 창을 함께 본다는 점도 있지만, 그때도 코드(`shell.notfound`)가 함께 나가므로
//! 무엇을 찾아봐야 하는지는 알 수 있다.
//!
//! ## 번역이 없으면
//!
//! 코드를 그대로 보여 준다(`Coded` 의 `Display`). 빈 줄이 나가는 것보다 낫고, 로그에서
//! 검색도 된다. 아래 `every_code_is_translated` 시험이 빠진 것을 잡는다.

use crate::{catalog::tr, Lang};
use nabi_error::Coded;

/// 오류 코드에 붙는 i18n 키 앞머리.
pub const PREFIX: &str = "err.";

/// 이 오류를 그 말로 적는다.
pub fn tr_error(lang: Lang, e: &Coded) -> String {
    let key = format!("{PREFIX}{}", e.code);
    let t = tr(lang, &key);
    // 없는 키에 `tr` 은 "?" 를 돌려준다. 그러면 번역이 없는 것이다.
    match t == "?" {
        true => e.to_string(),
        false => e.fill(t),
    }
}

/// 지금 화면 언어로 적는다(UI 알림용).
pub fn tr_error_current(e: &Coded) -> String {
    tr_error(crate::current(), e)
}

/// **기계가 읽을 오류** — 언제나 영어, 코드를 함께 적는다.
///
/// `nabi cli` 와 제어 평면 응답이 쓴다. 코드를 앞에 붙이는 까닭은, 에이전트가 문구를
/// 사람처럼 읽지 않고 **똑같은 글자를 찾기** 때문이다. 문구는 다듬을 수 있어도 코드는
/// 그대로 두면 그 찾기가 계속 맞는다.
pub fn tr_error_machine(e: &Coded) -> String {
    let en = tr_error(Lang::En, e);
    match en.starts_with(e.code) {
        true => en, // 번역이 없어 이미 코드로 시작한다.
        false => format!("{}: {en}", e.code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 번역이_있으면_그_말로_적는다() {
        let e = Coded::one("shell.notfound", "powershell.exe");
        let ko = tr_error(Lang::Ko, &e);
        let en = tr_error(Lang::En, &e);
        assert!(ko.contains("powershell.exe"), "인자가 빠졌다: {ko}");
        assert!(en.contains("powershell.exe"), "인자가 빠졌다: {en}");
        assert_ne!(ko, en, "세 나라 말이 같으면 번역이 아니다");
    }

    #[test]
    fn 번역이_없으면_코드를_보여_준다() {
        let e = Coded::one("no.such.code.here", "값");
        assert_eq!(tr_error(Lang::Ko, &e), "no.such.code.here: 값");
    }

    #[test]
    fn 기계용은_코드를_앞에_단다() {
        let e = Coded::one("shell.notfound", "pwsh.exe");
        let m = tr_error_machine(&e);
        assert!(m.starts_with("shell.notfound"), "{m}");
        assert!(m.contains("pwsh.exe"), "{m}");
        // 화면 언어가 무엇이든 영어여야 한다.
        crate::set_current(Lang::Ko);
        assert_eq!(tr_error_machine(&e), m);
        crate::set_current(Lang::En);
    }

    /// **쓰는 코드는 전부 번역이 있어야 한다.**
    ///
    /// 소스에서 `Coded::new("…")` / `one` / `with` 로 만드는 코드를 모아 목록과 대조한다.
    /// 손으로 관리하는 목록은 언젠가 실제와 달라진다 — 세어서 확인한다.
    #[test]
    fn 쓰는_코드는_전부_번역이_있다() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut used: Vec<String> = Vec::new();
        collect_codes(&root.join("crates"), &mut used);
        assert!(!used.is_empty(), "코드를 하나도 못 찾았다 — 검사가 헛돌고 있다");
        let mut missing: Vec<String> = used
            .into_iter()
            .filter(|c| tr(Lang::Ko, &format!("{PREFIX}{c}")) == "?")
            .collect();
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "번역이 없는 오류 코드: {missing:?}");
    }

    /// 소스를 훑어 `Coded::…("코드"` 의 코드를 모은다.
    fn collect_codes(dir: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_codes(&p, out);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let Ok(t) = std::fs::read_to_string(&p) else { continue };
            // **주석은 세지 않는다.** 설명에 적은 보기(`Coded::new("…")`)까지 세면
            // 있지도 않은 코드를 찾아 헤맨다 — 실제로 처음에 그렇게 걸렸다.
            for line in t.lines() {
                let st = line.trim_start();
                if st.starts_with("//") {
                    continue;
                }
                for m in ["Coded::new(\"", "Coded::one(\"", "Coded::with(\""] {
                    let mut at = 0usize;
                    while let Some(i) = line[at..].find(m) {
                        let s = at + i + m.len();
                        let Some(j) = line[s..].find('"') else { break };
                        let code = &line[s..s + j];
                        // 시험 안에서 일부러 쓰는 "없는 코드"는 세지 않는다.
                        if !code.starts_with("no.such") && !code.is_empty() && code != "x" {
                            out.push(code.to_string());
                        }
                        at = s + j;
                    }
                }
            }
        }
    }
}

/// `io::Error` 안에 담겨 온 오류를 사람 말로 옮긴다.
///
/// 낮은 크레이트는 서명을 바꾸지 않으려고 `io::Error::new(kind, Coded)` 로 코드를 안에
/// 담아 보낸다. 여기서 꺼내 옮긴다. 코드가 없으면(운영체제가 낸 오류 등) 원문 그대로.
pub fn tr_io(lang: Lang, e: &std::io::Error) -> String {
    match e.get_ref().and_then(|r| r.downcast_ref::<Coded>()) {
        Some(c) => tr_error(lang, c),
        None => e.to_string(),
    }
}

/// 지금 화면 언어로(`tr_io` + current).
pub fn tr_io_current(e: &std::io::Error) -> String {
    tr_io(crate::current(), e)
}

#[cfg(test)]
mod io_tests {
    use super::*;

    #[test]
    fn io_오류_안의_코드를_꺼내_옮긴다() {
        let e = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            Coded::one("shell.notfound", "pwsh.exe"),
        );
        let ko = tr_io(Lang::Ko, &e);
        assert!(ko.contains("pwsh.exe"), "{ko}");
        assert!(!ko.contains("shell.notfound"), "번역이 됐으면 코드는 안 보인다: {ko}");
    }

    #[test]
    fn 코드가_없으면_원문_그대로() {
        let e = std::io::Error::other("plain os error");
        assert_eq!(tr_io(Lang::Ko, &e), "plain os error");
    }
}
