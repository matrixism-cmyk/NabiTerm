//! AI CLI 최신 버전 확인 — 설치본과 배포본을 비교해 업데이트가 필요한지 판단한다.
//!
//! 버전 문자열은 CLI마다 제각각이라(`2.0.1 (Claude Code)`, `codex-cli 0.147.0`) 첫 semver만
//! 뽑아 비교한다. 비교는 순수 함수로 두고 네트워크는 한 군데(`latest_npm`)에만 둔다.

use std::path::Path;

/// 설치 경로가 알려 주는 갱신 통로.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Channel {
    /// npm 전역 패키지(윈도우에서는 `.cmd` 셔임으로 깔린다).
    Npm(&'static str),
    /// 공식 네이티브 설치본 — 자기 자신이 갱신 명령(`claude update`)을 갖고 있다.
    Native,
}

/// npm에 올라온 패키지 이름(없으면 npm으로 관리하지 않는 CLI).
pub(crate) fn npm_package(id: &str) -> Option<&'static str> {
    match id {
        "claude" => Some("@anthropic-ai/claude-code"),
        "codex" => Some("@openai/codex"),
        _ => None,
    }
}

/// 설치 경로로 갱신 통로를 고른다.
///
/// 같은 CLI라도 npm 전역 설치와 공식 설치본은 갱신 방법이 다르다. 통로를 잘못 고르면
/// **두 벌이 깔려** 서로 다른 버전이 PATH 앞자리를 다투게 된다. 윈도우에서 npm 전역 명령은
/// 항상 `.cmd` 셔임이고 공식 설치본은 `.exe`라 확장자로 가를 수 있다.
pub(crate) fn channel(id: &str, path: &Path) -> Option<Channel> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("exe") {
        // 네이티브 설치본을 npm으로 덮어쓰지 않는다. 자체 갱신이 없는 CLI는 수동으로 둔다.
        return (id == "claude").then_some(Channel::Native);
    }
    npm_package(id).map(Channel::Npm)
}

/// 버전 문자열에서 첫 semver(`숫자.숫자[.숫자]`)를 꺼낸다.
pub(crate) fn parse_version(s: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for tok in s.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        let dots = tok.matches('.').count();
        let ok = dots >= 1
            && !tok.starts_with('.')
            && !tok.ends_with('.')
            && tok.split('.').all(|p| !p.is_empty() && p.len() <= 9);
        if ok {
            best = Some(tok.to_string());
            break;
        }
    }
    best
}

/// 비교용 (major, minor, patch). 자리가 없으면 0으로 채운다.
fn parts(v: &str) -> (u64, u64, u64) {
    let mut it = v.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// 설치본이 배포본보다 낮은가. 어느 쪽이든 semver를 못 읽으면 **거짓**(모르면 건드리지 않는다).
pub(crate) fn is_outdated(installed: &str, latest: &str) -> bool {
    match (parse_version(installed), parse_version(latest)) {
        (Some(a), Some(b)) => parts(&a) < parts(&b),
        _ => false,
    }
}

/// npm 레지스트리에서 배포된 최신 버전을 읽는다(실패하면 None — 조용히 넘어간다).
pub(crate) fn latest_npm(pkg: &str) -> Option<String> {
    // 스코프 패키지의 `/`는 경로 구분자가 아니라 이름의 일부다 — 인코딩해야 404가 안 난다.
    let path = format!("/{}/latest", pkg.replace('/', "%2f"));
    // 레지스트리는 Accept를 깐깐하게 본다 — GitHub용 값을 그대로 주면 406으로 막힌다.
    let body =
        nabi_release::http_get_text_accept("registry.npmjs.org", &path, "application/json").ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v["version"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulls_version_out_of_assorted_banners() {
        assert_eq!(parse_version("2.0.1 (Claude Code)").as_deref(), Some("2.0.1"));
        assert_eq!(parse_version("codex-cli 0.147.0").as_deref(), Some("0.147.0"));
        assert_eq!(parse_version("v1.2").as_deref(), Some("1.2"));
        assert_eq!(parse_version("설치됨"), None);
        // 숫자 하나뿐이면 버전으로 보지 않는다(빌드 번호 등에 속지 않게).
        assert_eq!(parse_version("build 12345"), None);
    }

    #[test]
    fn compares_numerically_not_lexically() {
        assert!(is_outdated("0.9.0", "0.10.0"), "10 > 9 여야 한다(문자열 비교면 반대)");
        assert!(is_outdated("1.2.3", "1.2.4"));
        assert!(!is_outdated("1.2.4", "1.2.3"));
        assert!(!is_outdated("1.2.3", "1.2.3"));
        // 자리 수가 달라도 앞에서부터 채워 비교한다.
        assert!(is_outdated("1.2", "1.2.1"));
    }

    /// 버전을 못 읽으면 갱신 대상으로 보지 않는다 — 멋대로 재설치하지 않기 위한 안전판.
    #[test]
    fn unknown_version_never_triggers_update() {
        assert!(!is_outdated("알 수 없음", "1.0.0"));
        assert!(!is_outdated("1.0.0", "네트워크 실패"));
    }

    #[test]
    fn channel_follows_how_it_was_installed() {
        let npm = Path::new(r"C:\Users\u\AppData\Roaming\npm\codex.cmd");
        assert_eq!(channel("codex", npm), Some(Channel::Npm("@openai/codex")));
        let native = Path::new(r"C:\Users\u\.local\bin\claude.exe");
        assert_eq!(channel("claude", native), Some(Channel::Native));
        // 관리 통로를 모르는 CLI는 자동 갱신 대상에서 뺀다(수동 안내).
        assert_eq!(channel("antigravity", Path::new("agy.exe")), None);
        assert_eq!(channel("antigravity", Path::new("agy.cmd")), None);
    }

    /// 실제 레지스트리 조회(네트워크 필요) — 응답 형식이 바뀌면 여기서 걸린다.
    #[test]
    #[ignore = "네트워크 필요"]
    fn live_npm_latest() {
        for id in ["codex", "claude"] {
            let pkg = npm_package(id).expect("패키지 이름");
            let v = latest_npm(pkg).unwrap_or_else(|| panic!("{pkg} 최신 버전 조회 실패"));
            println!("{pkg} latest = {v}");
            assert!(parse_version(&v).is_some(), "semver로 읽혀야 한다: {v}");
        }
    }
}
