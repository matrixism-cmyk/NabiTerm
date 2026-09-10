//! **글을 보여 주는 창은 오른쪽 버튼에 답해야 한다** — 어긋나면 빨개지는 시험.
//!
//! ## 왜 시험으로 붙잡나
//!
//! 이 저장소에는 글을 보여 주는 창이 넷이다. 2026-09-10에 세어 보니 **무제한 편집기만**
//! 오른쪽 버튼에 아무 반응이 없었다. 거대한 로그를 여는 창이라 AI 에게 한 토막 넘길 일이
//! 가장 잦은 자리인데, 거기에 길이 없었다.
//!
//! 이런 어긋남은 조용하다. 컴파일도 시험도 통과하고, 화면도 멀쩡히 뜬다. 쓰는 사람만
//! "왜 여기선 안 되지" 하고 만다. 그래서 **세는 일을 기계에 맡긴다.**
//!
//! ## 무엇을 보나
//!
//! 창마다 두 가지를 본다.
//!
//! 1. 우클릭 메뉴를 **붙였는가**(`context_menu(`).
//! 2. 그 메뉴에 **기본 편집 명령이 있는가**(복사·붙여넣기·찾기).
//!
//! 글자 모양까지 맞추지는 않는다 — 창마다 할 수 있는 일이 달라서(HEX 에는 "찾기"가
//! 자기 것이 따로 있다) 억지로 같게 만들면 오히려 나빠진다. 빠진 것은 **까닭과 함께**
//! 예외에 적는다. 까닭 없는 예외 목록은 곧 "귀찮아서 넣은 것"으로 채워진다.

/// 글을 보여 주는 창: (이름, 메뉴를 그리는 소스, 그 메뉴를 붙이는 소스).
///
/// 작은 문서는 그리는 곳과 붙이는 곳이 **같은 파일**이다 — 그래서 둘 다 그 파일을 준다.
const SURFACES: &[(&str, &str, &str)] = &[
    ("작은 문서", include_str!("editorctx.rs"), include_str!("editorctx.rs")),
    ("rope 편집기", include_str!("editbufmenu.rs"), include_str!("editbufview.rs")),
    ("HEX", include_str!("edithexmenu.rs"), include_str!("edithexview.rs")),
    ("무제한 편집기", include_str!("textmenu.rs"), include_str!("textview.rs")),
];

/// 어느 창에나 있어야 하는 기본 명령: (i18n 키에 들어 있어야 할 **낱말**, 사람이 부르는 이름).
///
/// ⚠️ **키 이름으로 맞추지 않는다.** 처음에 `menu.copy` 로 찾았더니 네 창 중 둘이
/// "없다"고 나왔는데, 실은 넷이 같은 일을 **다른 이름**으로 하고 있었다 —
/// `ctx.copy`·`menu.copy`·`nabipad.copytext`. 이름으로 판단하면 어긋난다는 것이
/// 이 저장소의 규칙인데, 정작 그 규칙을 지키는 시험을 이름으로 짰다.
const BASICS: &[(&str, &str)] = &[("copy", "복사"), ("paste", "붙여넣기")];

/// 기본 명령이 없어도 되는 창과 그 까닭.
///
/// **까닭 없는 예외는 곧 "귀찮아서 넣은 것"으로 채워진다.** 지금은 비어 있다 —
/// 비어 있는 것이 맞다. 넷 다 복사·붙여넣기를 갖고 있다.
const EXCUSED: &[(&str, &str, &str)] = &[];

/// 이 메뉴 소스가 그 명령을 다루는가 — **i18n 키 안의 낱말**로 본다.
///
/// 창마다 앞머리가 다르므로(`ctx.`·`menu.`·`nabipad.`) 뒤쪽 낱말만 본다.
fn mentions(src: &str, word: &str) -> bool {
    src.match_indices("tr(lang, \"")
        .filter_map(|(i, m)| src[i + m.len()..].split('"').next())
        .any(|k| k.rsplit('.').next().unwrap_or(k).contains(word))
}

#[cfg(test)]
mod tests {
    use super::{mentions, BASICS, EXCUSED, SURFACES};

    /// **모든 창이 오른쪽 버튼에 답한다.** 이 시험이 없어서 한 창이 오래 조용했다.
    #[test]
    fn 모든_글_창이_우클릭에_답한다() {
        let mut silent: Vec<&str> = Vec::new();
        for (name, menu, view) in SURFACES {
            let drawn = menu.contains("fn context_menu") || menu.contains("fn editor_context_menu");
            let wired = view.contains(".context_menu(");
            if !drawn || !wired {
                silent.push(name);
            }
        }
        assert!(silent.is_empty(), "오른쪽 버튼에 답하지 않는 창: {silent:?}");
    }

    /// 기본 편집 명령은 어디에나 있어야 한다 — 없으면 까닭이 적혀 있어야 한다.
    #[test]
    fn 기본_명령이_빠지면_까닭이_있다() {
        let mut missing: Vec<String> = Vec::new();
        for (name, menu, _) in SURFACES {
            for (key, human) in BASICS {
                if mentions(menu, key) || EXCUSED.iter().any(|(n, k, _)| n == name && k == key) {
                    continue;
                }
                missing.push(format!("{name}: {human}"));
            }
        }
        assert!(missing.is_empty(), "까닭 없이 빠진 기본 명령: {missing:?}");
    }

    /// 예외에는 반드시 까닭이 적혀 있다.
    #[test]
    fn 예외에는_까닭이_있다() {
        for (name, key, why) in EXCUSED {
            assert!(why.chars().count() >= 10, "{name}/{key} 의 예외에 까닭이 없다");
        }
    }

    /// **낱말 판정이 실제로 갈라내는가** — 일부러 어긋난 것을 넣어 본다.
    ///
    /// 이 시험이 없으면 `mentions` 가 늘 참을 돌려줘도 아무도 모른다.
    #[test]
    fn 낱말_판정이_갈라낸다() {
        let src = r#"tr(lang, "ctx.copy") tr(lang, "nabipad.pastetext")"#;
        assert!(mentions(src, "copy"));
        assert!(mentions(src, "paste"));
        assert!(!mentions(src, "delete"), "없는 것을 있다고 했다");
        // 앞머리에 우연히 든 글자에 속으면 안 된다.
        //
        // 이 가짜 키는 **실행할 때 이어 붙인다.** 소스에 그대로 적어 두면
        // `xtask i18n-keys` 가 진짜 쓰임으로 보고 "카탈로그에 없다"고 경고한다 —
        // 검사기가 옳다(그 자리만 보고는 시험감인지 알 수 없다). 검사기를 무르게
        // 만드는 대신 시험 쪽이 비켜 준다.
        let fake = format!("tr(lang, {q}pastezone.x{q})", q = '"');
        assert!(!mentions(&fake, "copy"));
    }

    /// 소스를 실제로 읽고 있는가 — 못 읽으면 위 시험들이 **늘 통과한다.**
    ///
    /// 아무것도 못 보면서 초록인 검사기가 가장 나쁘다. 그래서 세어 둔다.
    #[test]
    fn 소스를_실제로_읽는다() {
        for (name, menu, view) in SURFACES {
            assert!(menu.len() > 500, "{name} 의 메뉴 소스를 못 읽었다");
            assert!(view.len() > 500, "{name} 의 화면 소스를 못 읽었다");
        }
    }
}
