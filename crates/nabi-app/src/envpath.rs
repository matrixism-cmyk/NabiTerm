//! 설치 직후 **PATH를 다시 읽어들인다.**
//!
//! 설치 프로그램은 레지스트리의 PATH를 고치지만, 이미 떠 있는 프로세스의 환경은 건드리지
//! 못한다. 그래서 우리 창에서 gh를 깔고 나면 — 실제로 깔렸는데도 — 우리 눈에는 안 보이고
//! 새로 여는 pane에도 안 잡힌다. 사용자에게는 "설치했는데 안 되네"로 보인다
//! (2026-08-25에 gh를 실제로 깔아 보고 확인했다: 레지스트리에는 들어갔는데 PATH엔 없었다).
//!
//! 그래서 설치가 끝나면 레지스트리에서 기계·사용자 PATH를 다시 읽어 우리 프로세스에
//! 얹는다. 새 pane은 우리 환경을 물려받으므로 이것 하나로 둘 다 해결된다.

/// `reg query … /v Path` 출력에서 값만 뽑는다.
///
/// 출력은 `    Path    REG_EXPAND_SZ    C:\a;C:\b` 꼴이다. 값 자체에 공백이 흔하니
/// 타입 이름 뒤를 통째로 가져와야 한다 — 공백으로 자르면 첫 폴더만 남는다.
pub(crate) fn parse_reg_path(out: &str) -> Option<String> {
    for line in out.lines() {
        let t = line.trim();
        if !t.starts_with("Path") {
            continue;
        }
        for ty in ["REG_EXPAND_SZ", "REG_SZ"] {
            if let Some(i) = t.find(ty) {
                let v = t[i + ty.len()..].trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// `%VAR%`를 푼다(REG_EXPAND_SZ는 풀리지 않은 채로 온다). 모르는 이름은 그대로 둔다.
pub(crate) fn expand(s: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        match after.find('%') {
            Some(b) => {
                let name = &after[..b];
                match lookup(name) {
                    Some(v) => out.push_str(&v),
                    // 못 찾으면 원문 그대로 — 지워 버리면 경로가 조용히 망가진다.
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[b + 1..];
            }
            None => {
                out.push('%');
                out.push_str(after);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 기계·사용자·현재 PATH를 합친다.
///
/// **현재 값을 버리지 않는다.** 이 세션에서만 더한 경로(개발용 도구 폴더 같은 것)가
/// 있을 수 있고, 그걸 날리면 지금 잘 돌던 것이 갑자기 안 돈다. 순서는 현재 → 기계 →
/// 사용자로, 중복은 처음 것만 남긴다.
pub(crate) fn merge(current: &str, machine: &str, user: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    for part in [current, machine, user].iter().flat_map(|s| s.split(';')) {
        let p = part.trim_end_matches('\\').trim();
        if p.is_empty() {
            continue;
        }
        if !seen.iter().any(|q| q.eq_ignore_ascii_case(p)) {
            seen.push(p.to_string());
        }
    }
    seen.join(";")
}

/// 레지스트리에서 PATH를 다시 읽어 이 프로세스에 얹는다. 바뀌었으면 true.
pub(crate) fn refresh() -> bool {
    let machine = read(r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment");
    let user = read(r"HKCU\Environment");
    if machine.is_empty() && user.is_empty() {
        return false;
    }
    let current = std::env::var("PATH").unwrap_or_default();
    let merged = merge(&current, &machine, &user);
    if merged == current {
        return false;
    }
    // SAFETY: 시작 직후가 아니라 UI 스레드에서 한 번씩 부르는 호출이고, 우리 프로세스는
    // 환경을 동시에 읽는 다른 스레드를 두지 않는다(자식 spawn은 이 뒤에 일어난다).
    unsafe { std::env::set_var("PATH", &merged) };
    true
}

/// 레지스트리 키 하나에서 Path 값을 읽어 `%VAR%`까지 푼다.
fn read(key: &str) -> String {
    let Ok(out) = crate::aicli::hidden("reg.exe").args(["query", key, "/v", "Path"]).output() else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    parse_reg_path(&text)
        .map(|v| expand(&v, &|n| std::env::var(n).ok()))
        .unwrap_or_default()
}

/// 시험에서 쓰는 경로 구분자(역슬래시 한 글자).
#[cfg(test)]
const SEP: &str = "\\";

#[cfg(test)]
mod tests {
    use super::*;

    /// **공백이 든 경로를 잘라 먹으면 안 된다** — Program Files가 첫 항목이면 바로 터진다.
    #[test]
    fn a_value_with_spaces_survives() {
        let out = concat!(
            "HKEY_LOCAL_MACHINE@@@...@@@Environment
",
            "    Path    REG_EXPAND_SZ    C:@@@Program Files@@@GitHub CLI@@@;C:@@@Windows
",
        )
        .replace("@@@", SEP);
        let want = format!("C:{SEP}Program Files{SEP}GitHub CLI{SEP};C:{SEP}Windows");
        assert_eq!(parse_reg_path(&out).unwrap(), want);
    }

    #[test]
    fn a_plain_string_value_also_works() {
        assert_eq!(parse_reg_path(r"    Path    REG_SZ    C:@bin".replace('@', SEP).as_str()).unwrap(), format!("C:{SEP}bin"));
    }

    #[test]
    fn junk_yields_nothing() {
        assert!(parse_reg_path("").is_none());
        assert!(parse_reg_path("ERROR: The system was unable to find").is_none());
        assert!(parse_reg_path("    Path    REG_SZ    ").is_none(), "빈 값은 값이 아니다");
    }

    #[test]
    fn variables_are_expanded() {
        let win = format!("C:{SEP}Windows");
        let look = |n: &str| (n == "SystemRoot").then(|| win.clone());
        assert_eq!(expand(&format!("%SystemRoot%{SEP}system32"), &look), format!("C:{SEP}Windows{SEP}system32"));
    }

    /// 모르는 이름을 지워 버리면 경로가 조용히 망가진다 — 그대로 둔다.
    #[test]
    fn an_unknown_variable_is_left_alone() {
        let look = |_: &str| None;
        assert_eq!(expand("%NOPE%/x", &look), "%NOPE%/x");
        assert_eq!(expand("50%% off", &look), "50%% off");
        assert_eq!(expand("no percent", &look), "no percent");
    }

    /// **이 세션에서만 더한 경로를 잃으면 안 된다.**
    #[test]
    fn the_current_path_is_never_dropped() {
        let mingw = format!("C:{SEP}mingw64{SEP}bin");
        let got = merge(&format!("{mingw};C:{SEP}Windows"), &format!("C:{SEP}Windows"), &format!("C:{SEP}Users{SEP}me{SEP}bin"));
        assert!(got.starts_with(&mingw), "{got}");
        assert!(got.contains(&format!("C:{SEP}Users{SEP}me{SEP}bin")));
    }

    /// 중복은 한 번만 — 안 그러면 PATH가 설치할 때마다 부푼다.
    #[test]
    fn duplicates_collapse_case_insensitively() {
        let got = merge(&format!("C:{SEP}Windows"), &format!("c:{SEP}windows{SEP}"), &format!("C:{SEP}WINDOWS"));
        assert_eq!(got, format!("C:{SEP}Windows"));
    }

    #[test]
    fn empty_segments_are_ignored() {
        assert_eq!(merge("C:/a;;", ";", ""), "C:/a");
    }
}
