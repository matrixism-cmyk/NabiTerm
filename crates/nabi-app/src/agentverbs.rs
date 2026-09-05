//! 도움말 ▸ AI 제어에 **모든 `nabi cli` 명령**을 내놓는다.
//!
//! ## 목록을 또 손으로 만들지 않는다
//!
//! 도움말에는 자주 쓰는 명령 열세 개를 손으로 적어 두고 있었다. 사용자가 "전부 나오면
//! 좋겠다"고 했는데(2026-09-05), 여기에 마흔 줄을 더 적으면 그 순간부터 실제와 어긋나기
//! 시작한다. 설정 색인에서 이미 두 번 겪은 일이다.
//!
//! 그래서 **설명서에서 뽑는다.** 설명서(`agentguide`)는 이미 양방향 대조 시험으로 실제
//! 낱말과 같음이 보장돼 있다 — 없는 것을 적으면 걸리고, 있는데 빠뜨려도 걸린다.
//! 그 목록을 그대로 쓰면 여기는 저절로 따라온다.
//!
//! ## 무엇을 뽑나
//!
//! 설명서의 명령 줄은 이렇게 생겼다.
//!
//! ```text
//! - (read) `nabi cli capture --pane <id> [--lines <n>]`
//! ```
//!
//! 괄호 안은 **무엇을 하는 명령인가**다(read=보기만, act=바꿈, inject=입력 주입,
//! local=이 PC 에서 처리). 그 표시와 명령을 함께 돌려준다 — 목록에서 위험한 것과
//! 안전한 것을 눈으로 가를 수 있어야 한다.

/// 명령 한 줄 — (등급, 명령).
pub(crate) type Verb = (&'static str, &'static str);

/// 설명서에 적힌 `nabi cli` 명령을 전부, 적힌 차례대로 돌려준다.
pub(crate) fn all_verbs() -> Vec<Verb> {
    crate::agentguide::AGENT_GUIDE_MD.lines().filter_map(parse_line).collect()
}

/// `- (등급) \`nabi cli …\`` 한 줄을 (등급, 명령)으로. 아니면 None.
fn parse_line(line: &'static str) -> Option<Verb> {
    let rest = line.strip_prefix("- (")?;
    let (kind, rest) = rest.split_once(')')?;
    let rest = rest.trim_start().strip_prefix('`')?;
    let (cmd, _) = rest.split_once('`')?;
    cmd.starts_with("nabi cli ").then_some((kind, cmd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 설명서에서_명령을_뽑는다() {
        assert_eq!(
            parse_line("- (read) `nabi cli list [--json]`"),
            Some(("read", "nabi cli list [--json]"))
        );
        // 명령 줄이 아니면 뽑지 않는다.
        assert_eq!(parse_line("보통 글줄"), None);
        assert_eq!(parse_line("- (read) `git status`"), None);
    }

    /// **비어 있으면 안 된다.** 설명서 모양이 바뀌면 목록이 조용히 사라진다 —
    /// 화면에는 빈 자리만 남고 아무도 오류를 못 본다.
    #[test]
    fn 목록이_비어_있지_않다() {
        let v = all_verbs();
        assert!(v.len() > 30, "명령이 {}개뿐이다 — 설명서 모양이 바뀌었나?", v.len());
        assert!(v.iter().any(|(_, c)| c.starts_with("nabi cli list")));
        assert!(v.iter().all(|(k, _)| !k.is_empty()));
    }
}
