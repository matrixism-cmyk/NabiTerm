//! 도구가 **정말 실행되는지** 확인하고 버전을 읽는다(배치 AK).
//!
//! ## 무엇이 잘못돼 있었나
//!
//! 지금까지는 `where` 로 찾은 경로가 마이크로소프트 스토어 별칭이면 "설치되지 않음"으로
//! 봤다. 그 판단은 PowerShell 7 때문에 생겼다. 스토어판 PowerShell 7 은 그 계정에 앱
//! 라이선스가 없으면 실행되지 않는데, 파일은 있으니 목록에는 뜬다. 뜨는데 안 열리는 것이
//! 가장 나쁘므로 아예 없는 것으로 봤다.
//!
//! 그런데 그 규칙을 모든 도구에 똑같이 적용한 것이 문제였다. **`winget` 도 스토어 별칭인데
//! 잘 실행된다**(윈도우에 딸려 오는 것이라 그렇다). 그래서 설치를 마쳐도 환경 관리자에는
//! 계속 "설치되지 않음"으로 보였다(사용자 보고 2026-08-29).
//!
//! ## 그래서 물음을 바꿨다
//!
//! "별칭인가"가 아니라 **"실행되는가"** 를 묻는다. 파일 모양으로는 `winget` 과 `pwsh` 를
//! 구분할 수 없다. 실행해 봐야 안다.
//!
//! ## 왜 셸은 이 방식을 쓰지 않는가
//!
//! 셸을 실행해 보면 창이 열리거나 대화형으로 멈춘다. 그래서 셸 목록은 지금처럼 파일로
//! 판단한다(`nabi_pty::is_store_alias`). **도구는 실행해서 묻고, 셸은 파일로 본다.**
//!
//! ## 덤으로 버전을 얻는다
//!
//! 실행해서 확인하는 김에 그 출력이 곧 버전이다. 따로 한 번 더 실행할 이유가 없다.

/// 버전 문자열에서 쓸 만한 한 줄을 뽑는다.
///
/// 도구마다 출력이 제각각이다.
///
/// ```text
/// winget --version   →  v1.29.290
/// git --version      →  git version 2.43.0.windows.1
/// rg --version       →  ripgrep 14.1.0 (여러 줄)
/// ```
///
/// **첫 줄만** 쓰고, 너무 길면 자른다. 화면 한 줄에 들어가야 하고, 긴 글이 필요한 사람은
/// 그 도구를 직접 실행한다.
pub(crate) fn tidy_version(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    // 도구 이름이 앞에 붙어 나오면 뗀다 — 옆에 이미 이름이 적혀 있다.
    let line = line
        .strip_prefix("git version ")
        .or_else(|| line.strip_prefix("ripgrep "))
        .unwrap_or(line);
    let short: String = line.chars().take(24).collect();
    (!short.is_empty()).then_some(short)
}

/// 이 이름을 실행해 버전을 받아 본다. 실행되지 않으면 `None`.
///
/// 실행되면 **설치된 것이다.** 별칭이든 아니든 상관없다 — 사용자에게 중요한 것은
/// 그 도구를 쓸 수 있는가이지, 파일이 어떤 모양인가가 아니다.
pub(crate) fn probe_version(name: &str, flag: &str) -> Option<String> {
    let out = crate::aicli::hidden(name).arg(flag).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // 어떤 도구는 버전을 stderr 로 낸다.
    let text = if text.trim().is_empty() {
        String::from_utf8_lossy(&out.stderr).into_owned()
    } else {
        text.into_owned()
    };
    tidy_version(&text)
}

#[cfg(test)]
mod tests {
    use super::tidy_version;

    #[test]
    fn the_first_useful_line_is_taken() {
        assert_eq!(tidy_version("v1.29.290\n").as_deref(), Some("v1.29.290"));
        assert_eq!(tidy_version("\n\n  1.2.3  \n뒷줄").as_deref(), Some("1.2.3"));
    }

    #[test]
    fn a_leading_tool_name_is_dropped() {
        // 옆에 이미 도구 이름이 적혀 있다. 두 번 쓸 이유가 없다.
        assert_eq!(tidy_version("git version 2.43.0").as_deref(), Some("2.43.0"));
        assert_eq!(tidy_version("ripgrep 14.1.0").as_deref(), Some("14.1.0"));
    }

    #[test]
    fn a_long_line_is_cut_so_it_fits_on_screen() {
        let long = "a".repeat(80);
        assert_eq!(tidy_version(&long).map(|s| s.chars().count()), Some(24));
    }

    #[test]
    fn nothing_useful_gives_nothing() {
        assert_eq!(tidy_version(""), None);
        assert_eq!(tidy_version("\n \n"), None);
    }

    #[test]
    fn hangul_output_is_counted_by_characters_not_bytes() {
        // 바이트로 자르면 한글이 반 토막 난다.
        let s = "버전 ".repeat(20);
        assert_eq!(tidy_version(&s).map(|v| v.chars().count()), Some(24));
    }
}
