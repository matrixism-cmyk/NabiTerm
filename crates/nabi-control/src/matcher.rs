//! `--match "title:x,cwd:y,kind:z,state:idle"` 속성 주소지정(CP-6 — Kitty 모델).
//! 매칭 0건/2건 이상은 명시적 오류(모호성 거부).

use crate::protocol::PaneInfo;

/// 파싱된 매치 조건(키 검증됨, negate=부정 매칭 `key:!value`).
pub struct Match(Vec<(MatchKey, String, bool)>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum MatchKey {
    Title,
    Cwd,
    Kind,
    State,
    Id,
}

/// "k:v[,k:v…]" 파싱. 미지의 키는 오류.
pub fn parse(s: &str) -> Result<Match, String> {
    let mut out = Vec::new();
    for part in s.split(',').filter(|p| !p.trim().is_empty()) {
        let (k, v) = part
            .split_once(':')
            .ok_or_else(|| format!("--match 항목 '{part}'은 k:v 형식이어야 함"))?;
        let key = match k.trim() {
            "title" => MatchKey::Title,
            "cwd" => MatchKey::Cwd,
            "kind" => MatchKey::Kind,
            "state" => MatchKey::State,
            "id" => MatchKey::Id,
            other => return Err(format!("--match 키 '{other}' 미지원(title/cwd/kind/state/id)")),
        };
        // 값이 `!`로 시작하면 부정 매칭(해당 조건을 만족하지 '않는' pane).
        let v = v.trim();
        let (negate, v) = v.strip_prefix('!').map_or((false, v), |rest| (true, rest));
        out.push((key, v.to_string(), negate));
    }
    if out.is_empty() {
        return Err("--match 조건이 비어 있음".into());
    }
    Ok(Match(out))
}

impl Match {
    /// pane이 모든 조건을 충족하는지. title=부분일치, cwd=접두, 나머지=일치.
    pub fn matches(&self, p: &PaneInfo) -> bool {
        self.0.iter().all(|(k, v, negate)| {
            let hit = match k {
                MatchKey::Title => p.title.to_lowercase().contains(&v.to_lowercase()),
                MatchKey::Cwd => p.cwd.as_deref().is_some_and(|c| c.to_lowercase().starts_with(&v.to_lowercase())),
                MatchKey::Kind => p.kind == *v,
                MatchKey::State => p.state == *v,
                MatchKey::Id => p.id.to_string() == *v,
            };
            hit != *negate // negate면 조건 반전.
        })
    }
}

/// 정확히 1개 매칭의 pane ID. 0건/2건 이상이면 오류(후보 ID 나열).
pub fn resolve(panes: &[PaneInfo], m: &Match) -> Result<u64, String> {
    let hits: Vec<&PaneInfo> = panes.iter().filter(|p| m.matches(p)).collect();
    match hits.as_slice() {
        [one] => Ok(one.id),
        [] => Err("매칭되는 pane 없음".into()),
        many => Err(format!(
            "모호함 — {}개 매칭: {}",
            many.len(),
            many.iter().map(|p| p.id.to_string()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// 매칭되는 모든 pane ID(브로드캐스트/일괄 동작용 — 모호성 거부 없음).
pub fn resolve_all(panes: &[PaneInfo], m: &Match) -> Vec<u64> {
    panes.iter().filter(|p| m.matches(p)).map(|p| p.id).collect()
}

/// `--match` 를 **맞는 모든 pane** 으로 펼친다 — pane 하나에 인자 한 벌.
///
/// ## 왜 필요한가
///
/// `--match` 는 여럿이 걸리면 거절한다. 엉뚱한 pane 에 글자를 밀어 넣는 것보다 묻는 편이
/// 낫기 때문이다. 그런데 **일부러 여럿에 시키고 싶을 때**가 있다 — "지금 놀고 있는 SSH
/// 창 전부에 같은 명령을 보내라" 같은 것. 그때 거절은 방해가 된다.
///
/// 그래서 `--all` 을 함께 주면 거절 대신 펼친다. 여럿임을 **부르는 쪽이 알고 있다**는
/// 표시라, 사고로 여러 창에 들어가는 일은 여전히 막힌다.
///
/// `--match` 가 없으면 원본 그대로 한 벌만 돌려준다.
pub fn expand_args(
    args: &[String],
    fetch: impl FnOnce() -> Result<Vec<PaneInfo>, String>,
) -> Result<Vec<Vec<String>>, String> {
    let wants_all = args.iter().any(|a| a == "--all");
    if !wants_all {
        return resolve_args(args, fetch).map(|a| vec![a]);
    }
    let Some(i) = args.iter().position(|a| a == "--match") else {
        return Err("--all 은 --match 와 함께 쓴다".into());
    };
    let expr = args.get(i + 1).ok_or("--match 다음에 조건이 필요함")?;
    let m = parse(expr)?;
    let ids = resolve_all(&fetch()?, &m);
    if ids.is_empty() {
        return Err(format!("조건에 맞는 pane 이 없다: {expr}"));
    }
    // 남는 인자에서 --match 두 칸과 --all 을 뺀 뒤, pane 마다 한 벌씩 만든다.
    let rest: Vec<String> = args[..i]
        .iter()
        .chain(args[i + 2..].iter())
        .filter(|a| *a != "--all")
        .cloned()
        .collect();
    Ok(ids
        .into_iter()
        .map(|id| {
            let mut v = rest.clone();
            v.push("--pane".into());
            v.push(id.to_string());
            v
        })
        .collect())
}

/// CLI 인자에서 `--match <expr>`를 찾아 단일 pane으로 해석하고
/// `--pane <id>`로 치환한 인자 목록을 돌려준다(없으면 원본 그대로).
/// fetch는 ListPanes 결과를 가져오는 클로저(테스트 주입 가능).
pub fn resolve_args(
    args: &[String],
    fetch: impl FnOnce() -> Result<Vec<PaneInfo>, String>,
) -> Result<Vec<String>, String> {
    let Some(i) = args.iter().position(|a| a == "--match") else {
        return Ok(args.to_vec());
    };
    let expr = args.get(i + 1).ok_or("--match 다음에 조건이 필요함")?;
    let m = parse(expr)?;
    let id = resolve(&fetch()?, &m)?;
    let mut out: Vec<String> = args[..i].to_vec();
    out.extend(args[i + 2..].iter().cloned());
    out.push("--pane".into());
    out.push(id.to_string());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u64, title: &str, cwd: &str, state: &str) -> PaneInfo {
        PaneInfo {
            id,
            title: title.into(),
            kind: "local".into(),
            cwd: Some(cwd.into()),
            state: state.into(),
            ..Default::default()
        }
    }

    /// `--all` 이면 거절하지 않고 pane 마다 한 벌씩 펼친다.
    #[test]
    fn all_expands_to_every_match() {
        let panes = vec![
            pane(1, "pwsh", r"C:\proj", "idle"),
            pane(2, "cmd", r"C:\proj", "idle"),
            pane(3, "pwsh", r"D:\x", "working"),
        ];
        let args: Vec<String> = ["send", "--match", "state:idle", "--all", "--data", "hi"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = expand_args(&args, || Ok(panes)).unwrap();
        assert_eq!(out.len(), 2, "맞는 pane 마다 한 벌");
        assert!(out[0].ends_with(&["--pane".to_string(), "1".to_string()]));
        assert!(out[1].ends_with(&["--pane".to_string(), "2".to_string()]));
        // 원래 인자는 그대로 남는다.
        assert!(out[0].contains(&"--data".to_string()) && out[0].contains(&"hi".to_string()));
        // --all 과 --match 는 빠진다.
        assert!(!out[0].contains(&"--all".to_string()));
        assert!(!out[0].contains(&"--match".to_string()));
    }

    /// `--all` 없이 여럿이 걸리면 예전처럼 거절한다 — 사고를 막는 규칙은 그대로다.
    #[test]
    fn without_all_it_still_refuses_ambiguity() {
        let panes = vec![pane(1, "a", "/x", "idle"), pane(2, "b", "/x", "idle")];
        let args: Vec<String> =
            ["send", "--match", "state:idle"].iter().map(|s| s.to_string()).collect();
        assert!(expand_args(&args, || Ok(panes)).is_err());
    }

    /// 맞는 것이 없으면 조용히 아무것도 안 하지 않고 그렇다고 말한다.
    #[test]
    fn all_with_no_match_says_so() {
        let panes = vec![pane(1, "a", "/x", "working")];
        let args: Vec<String> = ["send", "--match", "state:idle", "--all"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(expand_args(&args, || Ok(panes)).is_err());
    }

    #[test]
    fn resolves_unique_and_rejects_ambiguity() {
        let panes = vec![
            pane(1, "pwsh", r"C:\proj", "idle"),
            pane(2, "cmd", r"C:\proj\sub", "working"),
            pane(3, "pwsh", r"D:\other", "idle"),
        ];
        // 유일 매칭.
        let m = parse(r"cwd:C:\proj,state:working").unwrap();
        assert_eq!(resolve(&panes, &m).unwrap(), 2);
        // 0건.
        assert!(resolve(&panes, &parse("title:zsh").unwrap()).is_err());
        // 2건 이상 → 모호성 거부.
        let err = resolve(&panes, &parse("title:pwsh").unwrap()).unwrap_err();
        assert!(err.contains("1, 3"), "{err}");
        // 미지 키 거부.
        assert!(parse("color:red").is_err());
        // 부정 매칭: pwsh가 '아닌' 유일 pane = id 2(cmd).
        assert_eq!(resolve(&panes, &parse("title:!pwsh").unwrap()).unwrap(), 2);
        // state:idle 이면서 title이 pwsh가 아닌 pane 없음(1,3은 pwsh) → 0건.
        assert!(resolve(&panes, &parse("state:idle,title:!pwsh").unwrap()).is_err());
        // resolve_all: pwsh 2개(1,3).
        assert_eq!(resolve_all(&panes, &parse("title:pwsh").unwrap()), vec![1, 3]);
    }

    #[test]
    fn rewrites_match_into_pane_flag() {
        let args: Vec<String> =
            ["send", "--match", "title:cmd", "--data", "x"].iter().map(|s| s.to_string()).collect();
        let out = resolve_args(&args, || {
            Ok(vec![pane(2, "cmd", r"C:\", "idle")])
        })
        .unwrap();
        assert_eq!(out, ["send", "--data", "x", "--pane", "2"]);
        // --match 없으면 원본 그대로(fetch 미호출).
        let plain: Vec<String> = ["list".to_string()].to_vec();
        assert_eq!(resolve_args(&plain, || Err("호출되면 안 됨".into())).unwrap(), plain);
    }
}
