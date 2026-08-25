//! 이 PC에 **정말로 있는** 셸만 고른다.
//!
//! ## 왜 WSL이 늘 보였는가 (사용자 보고 2026-08-25)
//!
//! `wsl.exe`는 WSL을 한 번도 깔지 않은 윈도우에도 `System32`에 **항상 있다** — 실행하면
//! "설치하세요"라고 안내하는 껍데기다. 그래서 "PATH에 wsl.exe가 있는가"로 물으면 답은
//! 언제나 예다. 물음이 틀렸다. 옳은 물음은 **"쓸 수 있는 배포판이 하나라도 있는가"**다.
//!
//! ## 왜 있는데 안 보이는 셸이 있었는가
//!
//! 목록이 다섯 종류로 고정돼 있었다. Nushell·Cygwin·MSYS2·PowerShell 7 미리보기처럼
//! 흔히 쓰는 것들이 깔려 있어도 나올 자리가 없었다. 이것들은 `ShellKind::Custom`으로
//! 넣는다 — 열거형을 건드리지 않아도 되고, 판정은 "그 파일이 있는가" 하나뿐이다.

use nabi_proto::ShellKind;

/// 열거형에 없는 셸들 — (표시 이름, 후보 경로들, 인자).
///
/// 후보는 **앞에서부터** 찾는다. PATH에 있으면 그것을 쓰고, 없으면 흔한 설치 경로를 본다
/// (설치는 했는데 PATH에 안 넣는 사람이 많다).
const EXTRAS: &[(&str, &[&str], &[&str])] = &[
    ("Nushell", &["nu.exe"], &[]),
    (
        "PowerShell 7 (미리보기)",
        &["pwsh-preview.exe", r"%ProgramFiles%\PowerShell\7-preview\pwsh.exe"],
        &["-NoLogo"],
    ),
    (
        "Windows PowerShell (32비트)",
        &[r"%SystemRoot%\SysWOW64\WindowsPowerShell\v1.0\powershell.exe"],
        &["-NoLogo"],
    ),
    ("Cygwin Bash", &[r"%SystemDrive%\cygwin64\bin\bash.exe", r"%SystemDrive%\cygwin\bin\bash.exe"], &["-l"]),
    ("MSYS2 Bash", &[r"%SystemDrive%\msys64\usr\bin\bash.exe"], &["-l"]),
];

/// WSL 항목들. **배포판이 없으면 아무것도 내놓지 않는다** — 이것이 이 모듈의 요점이다.
pub(crate) fn wsl_entries(distros: &[String]) -> Vec<(String, ShellKind)> {
    distros
        .iter()
        .map(|d| (format!("WSL: {d}"), ShellKind::Wsl { distro: Some(d.clone()) }))
        .collect()
}

/// `%VAR%`를 풀고 후보 중 처음 있는 것을 고른다.
pub(crate) fn pick<'a>(cands: &[&'a str], exists: &dyn Fn(&str) -> bool) -> Option<String> {
    for c in cands {
        let full = crate::envpath::expand(c, &|n| std::env::var(n).ok());
        // 경로 없이 이름만 준 것은 PATH에서 찾는다.
        let found = match full.contains(['\\', '/']) {
            true => exists(&full).then(|| full.clone()),
            false => nabi_pty::resolve_program(&full).map(|p| p.to_string_lossy().into_owned()),
        };
        if let Some(p) = found {
            return Some(p);
        }
    }
    None
}

/// 이 PC에 있는 추가 셸들.
pub(crate) fn extras(exists: &dyn Fn(&str) -> bool) -> Vec<(String, ShellKind)> {
    EXTRAS
        .iter()
        .filter_map(|(label, cands, args)| {
            let program = pick(cands, exists)?;
            let args = args.iter().map(|a| a.to_string()).collect();
            Some((label.to_string(), ShellKind::Custom { program, args }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **배포판이 없으면 WSL은 목록에 없어야 한다** — 사용자가 보고한 바로 그 증상.
    #[test]
    fn no_distro_means_no_wsl_entry() {
        assert!(wsl_entries(&[]).is_empty());
    }

    #[test]
    fn each_distro_gets_its_own_entry() {
        let got = wsl_entries(&["Ubuntu".to_string(), "Debian".to_string()]);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "WSL: Ubuntu");
        assert!(matches!(&got[0].1, ShellKind::Wsl { distro: Some(d) } if d == "Ubuntu"));
    }

    /// 앞 후보가 없으면 뒤 후보로 넘어간다(설치는 했는데 PATH에 없는 경우).
    #[test]
    fn the_first_existing_candidate_wins() {
        let have = |p: &str| p.ends_with("second.exe");
        let got = pick(&[r"C:\a\first.exe", r"C:\b\second.exe"], &have);
        assert_eq!(got.as_deref(), Some(r"C:\b\second.exe"));
    }

    #[test]
    fn nothing_found_is_nothing() {
        assert!(pick(&[r"C:\nope\x.exe"], &|_| false).is_none());
        assert!(pick(&[], &|_| true).is_none());
    }

    /// 없는 셸이 목록에 끼면 클릭했을 때 실패한다 — 하나도 없으면 하나도 안 나와야 한다.
    #[test]
    fn extras_are_empty_when_nothing_is_installed() {
        // 경로 후보는 전부 없다고 답한다. 이름만 있는 후보(PATH 조회)는 이 PC 사정을 타므로
        // 개수 대신 "경로형 항목이 없다"만 본다.
        let got = extras(&|_| false);
        for (label, kind) in &got {
            match kind {
                ShellKind::Custom { program, .. } => {
                    assert!(!program.contains('\\'), "{label}: 없는 경로가 목록에 들어왔다");
                }
                _ => panic!("{label}: 추가 셸은 Custom이어야 한다"),
            }
        }
    }

    /// 라벨이 겹치면 사용자는 어느 것이 어느 것인지 알 수 없다.
    #[test]
    fn extra_labels_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (label, _, _) in EXTRAS {
            assert!(seen.insert(*label), "라벨 중복: {label}");
        }
    }

    /// 이 PC에서 실제로 무엇이 잡히는지 눈으로 본다. 순수 시험은 규칙만 볼 수 있다.
    ///
    /// ```text
    /// cargo test -p nabi-app what_this_pc_has -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "이 PC의 실제 상태를 찍어 본다"]
    fn what_this_pc_has() {
        for (label, kind) in crate::menu::installed_shells() {
            eprintln!("  {label}  ->  {kind:?}");
        }
        eprintln!("  WSL 배포판: {:?}", nabi_pty::wsl_distros());
    }
}
