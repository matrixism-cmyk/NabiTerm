//! **에이전트가 든 키 보기** — 붙기 전에 에이전트 인증이 될지 안다.
//!
//! 빠른 연결에서 비밀번호와 키 파일을 둘 다 비우면 ssh-agent로 붙는다(`sshauth`). 그런데
//! 에이전트에 키가 하나도 없으면 그대로 실패한다. 그 사실을 **시도한 뒤에** 알게 되는 것은
//! 늦다 — 특히 실패 이유가 "Not authenticated" 한 줄로 오던 시절에는 원인을 짚기도 어려웠다.
//!
//! `nabi_ssh::agent::agent_identities()`는 이 화면을 위해 만들어졌고(주석에 그렇게 적혀
//! 있다) **한 번도 불리지 않았다.** 여기서 잇는다.
//!
//! ## 왜 배경에서 한 번만 묻는가
//!
//! 에이전트에 묻는 것은 이름 있는 파이프를 열고 왕복하는 일이다. 대화상자를 그리는 매
//! 프레임마다 하면 UI가 끊긴다. 대화상자가 열릴 때 한 번 묻고, 결과를 들고 있는다.

/// 조회 결과. 아직이면 `None`.
pub(crate) type Keys = std::sync::Arc<std::sync::Mutex<Option<Vec<String>>>>;

/// 배경에서 에이전트에 키 목록을 묻는다.
///
/// 실패(에이전트 없음·응답 없음)는 **빈 목록**으로 답한다 — "모른다"와 "없다"를 나눌 필요가
/// 없다. 둘 다 사용자에게는 "에이전트로는 못 붙는다"는 같은 뜻이다.
pub(crate) fn probe(ctx: &egui::Context) -> Keys {
    let out: Keys = std::sync::Arc::new(std::sync::Mutex::new(None));
    let (store, ctx) = (out.clone(), ctx.clone());
    std::thread::spawn(move || {
        // 런타임 세우기는 nabi-ssh에 맡긴다 — GUI 크레이트에 tokio를 들이지 않기 위해서다.
        let keys = nabi_ssh::agent::agent_identities_blocking();
        if let Ok(mut s) = store.lock() {
            *s = Some(keys);
        }
        ctx.request_repaint();
    });
    out
}

/// 화면에 낼 한 줄과 그것이 경고인지.
///
/// * 아직 안 왔으면 `None` — 빈 자리를 두지, "없음"이라고 단정하지 않는다.
/// * 하나도 없으면 **경고**(이 상태로 붙으면 실패한다).
pub(crate) fn summary(keys: &Keys) -> Option<(String, bool)> {
    let g = keys.lock().ok()?;
    let list = g.as_ref()?;
    if list.is_empty() {
        return Some((nabi_i18n::trc("qc.agent.nokeys").to_string(), true));
    }
    Some((format!("{} {}", nabi_i18n::trc("qc.agent.keys"), list.len()), false))
}

/// 도움말 풍선에 넣을 자세한 목록(너무 길면 자른다).
pub(crate) fn detail(keys: &Keys) -> String {
    let Ok(g) = keys.lock() else { return String::new() };
    let Some(list) = g.as_ref() else { return String::new() };
    const MAX: usize = 8;
    let mut s: Vec<String> = list.iter().take(MAX).cloned().collect();
    if list.len() > MAX {
        s.push(format!("… +{}", list.len() - MAX));
    }
    s.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys_of(v: Option<Vec<String>>) -> Keys {
        std::sync::Arc::new(std::sync::Mutex::new(v))
    }

    /// **아직 안 온 것을 "없음"으로 단정하지 않는다.**
    #[test]
    fn an_unfinished_probe_shows_nothing() {
        assert!(summary(&keys_of(None)).is_none());
        assert_eq!(detail(&keys_of(None)), "");
    }

    /// 키가 없으면 경고여야 한다 — 이 상태로 붙으면 실패한다.
    #[test]
    fn no_keys_is_a_warning() {
        let (_, warn) = summary(&keys_of(Some(Vec::new()))).unwrap();
        assert!(warn, "키가 없는데 경고로 표시하지 않았다");
    }

    #[test]
    fn keys_present_is_not_a_warning() {
        let (text, warn) = summary(&keys_of(Some(vec!["ed25519 …".into()]))).unwrap();
        assert!(!warn);
        assert!(text.contains('1'), "{text}");
    }

    /// 목록이 길면 자르되 **몇 개를 줄였는지 말한다**.
    #[test]
    fn a_long_list_is_trimmed_and_says_so() {
        let many: Vec<String> = (0..20).map(|i| format!("key{i}")).collect();
        let d = detail(&keys_of(Some(many)));
        assert_eq!(d.lines().count(), 9, "{d}");
        assert!(d.contains("+12"), "{d}");
    }

    #[test]
    fn a_short_list_is_shown_whole() {
        let d = detail(&keys_of(Some(vec!["a".into(), "b".into()])));
        assert_eq!(d, "a\nb");
    }
}
