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
