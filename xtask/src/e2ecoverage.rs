//! **e2e 가 실제 동사를 다 보는가** — 프로브 목록을 손이 아니라 소스와 대조한다.
//!
//! ## 왜 필요한가
//!
//! `e2everbs` 는 부를 동사를 손으로 적어 두고, 안 부르는 것은 **주석으로** 이유를 적어
//! 두었다. 주석은 아무도 지키지 않는다. 동사를 새로 만든 사람이 그 주석을 읽을 이유가
//! 없고, 안 읽어도 아무 일도 일어나지 않는다.
//!
//! 실제로 그렇게 됐다. 동사가 마흔일곱 개인데 스모크가 보는 것은 열아홉 개였고, 빠진
//! 것 중에 **두 번이나 깨졌던 `web`** 이 있었다. 깨질 때마다 사용자가 먼저 발견했다.
//!
//! 그래서 표를 소스에서 파생시킨다. 동사를 만들면 여기 걸리고, 부르지 않기로 했다면
//! **왜 안 부르는지를 코드에 적어야** 넘어간다. 주석이 아니라 목록이라서 낡지 않는다.
//!
//! ## 무엇을 검사하는가
//!
//! 실제 동사(= 클라이언트가 파는 낱말) 중에 프로브에도 없고 [`SKIP`] 에도 없는 것.
//! 반대 방향도 본다 — [`SKIP`] 에 적혔는데 실제로는 없는 동사(낡은 목록).

/// 스모크에서 **일부러 부르지 않는** 동사와 그 까닭.
///
/// 되돌릴 수 없거나, 바깥을 건드리거나, 왕복 하나로 볼 수 없는 것들이다.
/// 스모크가 사용자의 컴퓨터를 바꾸면 안 된다.
pub(crate) const SKIP: &[(&str, &str)] = &[
    ("update", "프로그램을 갈아 끼우고 다시 켠다"),
    ("integration", "사용자의 ~/.claude/settings.json 을 고친다"),
    ("install", "위와 같은 동사의 뒷낱말"),
    ("schedule", "일정이 파일에 남는다"),
    ("create", "schedule 의 뒷낱말"),
    ("open-sftp", "원격 연결이 있어야 한다(실서버 시험이 따로 있다)"),
    ("sftp-get", "원격 연결 필요"),
    ("sftp-put", "원격 연결 필요"),
    ("sftp-list", "원격 연결 필요"),
    ("events", "끝나지 않는 흐름이라 왕복 하나로 볼 수 없다"),
    ("tail", "끝나지 않는 흐름"),
    ("send", "스모크 본문이 이미 직접 부른다(e2e.rs)"),
    ("spawn", "스모크 본문이 이미 직접 부른다"),
    ("kill", "스모크 본문이 마지막에 부른다"),
    ("open-file", "스모크 본문이 큰 파일로 이미 부른다"),
    ("status", "pane-status-set/clear 프로브가 같은 동사다"),
    ("set", "status 의 뒷낱말"),
    ("clear", "status 의 뒷낱말"),
    ("report", "agent 의 뒷낱말 — 상태를 남긴다"),
    ("release", "agent 의 뒷낱말"),
    ("session", "agent 의 뒷낱말 — 훅이 부른다"),
    ("prompt", "다른 pane 에 글자를 밀어 넣는다"),
    ("agent", "위 낱말들의 앞낱말"),
    ("layout", "앞낱말이다(layout export/apply) — layout-export 프로브가 실제 왕복을 본다"),
    ("export", "layout 의 뒷낱말"),
    ("apply", "layout 의 뒷낱말 — 배치를 바꾼다"),
    ("goto", "web 의 뒷낱말"),
    ("web-eval", "쪽에 스크립트를 넣는다 — 스모크에서 할 일이 아니다"),
    ("pdf", "web 의 뒷낱말 — 파일을 만든다"),
    ("shot", "web 의 뒷낱말 — screenshot 프로브가 같은 일을 한다"),
    ("zoom", "web 의 뒷낱말"),
    // 아래 넷은 **파이프를 타지 않는다** — 클라이언트가 그 자리에서 답한다.
    // 서버에 던질 요청 자체가 없으므로 스모크가 볼 수 있는 것이 없다.
    ("security", "클라이언트가 설정을 직접 읽어 답한다(서버 왕복 없음)"),
    ("audit", "security 의 뒷낱말"),
    ("api", "클라이언트가 규격 문서를 그 자리에서 찍는다(서버 왕복 없음)"),
    ("schema", "api 의 뒷낱말"),
];

/// 클라이언트 소스에서 **실제로 파는 낱말**을 모은다.
///
/// `agentguide` 의 대조 시험과 같은 방식이다 — 파는 자리가 세 파일에 나뉘어 있어
/// 하나만 보면 있는 것을 없다고 한다.
pub(crate) fn known_verbs() -> Vec<String> {
    let src = [
        include_str!("../../crates/nabi-control/src/clientverbs.rs"),
        include_str!("../../crates/nabi-control/src/client.rs"),
        include_str!("../../crates/nabi-control/src/clientagent.rs"),
    ]
    .concat();
    let mut v: Vec<String> = src
        .split("Some(\"")
        .skip(1)
        .filter_map(|p| p.split('"').next().map(str::to_string))
        .filter(|w| !w.is_empty() && !w.starts_with("--"))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// 낱말 → 서버가 보는 `op` 이름. 소스에서 뽑는다.
///
/// 낱말과 op 이름은 자주 다르다 — `list` 는 `list-panes` 를, `web` 은 `open-web` 을 보낸다.
/// 그 짝을 손으로 적어 두면 그 표가 또 낡는다. `Some("낱말") => Ok(ControlRequest::갈래`
/// 라는 모양에서 갈래 이름을 읽어 kebab 으로 바꾸면 그것이 곧 `op` 다(serde 규칙).
///
/// 짝이 안 보이는 낱말(뒷낱말·클라이언트 전용)은 낱말 자신을 그대로 쓴다 — 그러면
/// [`SKIP`] 이 받아 준다.
pub(crate) fn verb_to_op(verb: &str) -> String {
    let src = [
        include_str!("../../crates/nabi-control/src/clientverbs.rs"),
        include_str!("../../crates/nabi-control/src/clientagent.rs"),
    ]
    .concat();
    let needle = format!("Some(\"{verb}\") => Ok(ControlRequest::");
    match src.split(&needle).nth(1) {
        Some(rest) => {
            let name: String =
                rest.chars().take_while(|c| c.is_alphanumeric()).collect();
            match name.is_empty() {
                true => verb.to_string(),
                false => kebab(&name),
            }
        }
        None => verb.to_string(),
    }
}

/// `OpenWeb` → `open-web`. serde 의 `rename_all = "kebab-case"` 와 같은 규칙이다.
fn kebab(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// 프로브가 실제로 던지는 `op` 이름들.
///
/// 프로브 이름이 아니라 **보내는 JSON 의 `op`** 을 본다 — 이름은 사람이 붙인 별칭이라
/// 동사와 다를 수 있다(`wait-idle` 이 `wait` 을 부르는 것처럼).
pub(crate) fn probed_ops(pane: u64) -> Vec<String> {
    let mut v: Vec<String> = crate::e2everbs::probe_reqs(pane)
        .iter()
        .filter_map(|r| r.split("\"op\":\"").nth(1))
        .filter_map(|r| r.split('"').next())
        .map(str::to_string)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// 빠진 동사들 — 프로브에도 없고 [`SKIP`] 에도 없는 것.
pub(crate) fn uncovered(pane: u64) -> Vec<String> {
    let probed = probed_ops(pane);
    known_verbs()
        .into_iter()
        .filter(|v| !probed.contains(&verb_to_op(v)) && !SKIP.iter().any(|(s, _)| s == v))
        .collect()
}

/// [`SKIP`] 에 적혔는데 실제로는 없는 동사 — 낡은 목록이다.
pub(crate) fn stale_skips() -> Vec<String> {
    let known = known_verbs();
    SKIP.iter()
        .map(|(s, _)| s.to_string())
        .filter(|s| !known.contains(s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **동사를 만들면 스모크가 보든지, 왜 안 보는지 적든지 해야 한다.**
    ///
    /// 이 시험이 없으면 새 동사는 조용히 검사 밖에 남는다. 실제로 `web` 이 그렇게 남아
    /// 두 번 깨졌고 두 번 다 사용자가 먼저 발견했다.
    #[test]
    fn 모든_동사는_시험되거나_이유가_적혀_있다() {
        let missing = uncovered(1);
        assert!(
            missing.is_empty(),
            "스모크가 안 보는 동사: {missing:?}\n\
             프로브를 더하거나, e2ecoverage::SKIP 에 까닭과 함께 적을 것"
        );
    }

    /// 낱말과 op 이름이 다른 것들을 실제로 짝지어 주는가.
    ///
    /// 이 짝이 어긋나면 프로브가 있는데도 "안 본다"고 나오고, 그러면 사람은 검사기를
    /// 끄고 싶어진다. 검사기가 틀리면 검사기부터 의심할 것.
    #[test]
    fn 낱말과_op_이름의_짝을_소스에서_읽는다() {
        assert_eq!(verb_to_op("list"), "list-panes");
        assert_eq!(verb_to_op("web"), "open-web");
        assert_eq!(verb_to_op("capture"), "capture");
        assert_eq!(verb_to_op("scroll"), "scroll");
        // 짝이 없는 낱말(뒷낱말)은 자기 이름 그대로 — SKIP 이 받는다.
        assert_eq!(verb_to_op("audit"), "audit");
    }

    /// 반대 방향 — 없는 동사를 뺀다고 적어 두면 목록이 낡았다는 뜻이다.
    #[test]
    fn 제외_목록에_없는_동사가_적혀_있지_않다() {
        let stale = stale_skips();
        assert!(stale.is_empty(), "SKIP 에 적혔는데 실제로는 없는 동사: {stale:?}");
    }
}
