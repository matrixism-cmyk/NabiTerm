//! **고른 파일에 원격 명령** — 명령을 짓고, 이름을 안전하게 인용한다.
//!
//! 압축·해제·해시 확인 같은 일은 파일 관리자에서 하다가 터미널로 건너가 경로를 다시 치게
//! 된다. 그 왕복을 없앤다.
//!
//! ## 자유 입력 칸을 만들지 않는다
//!
//! "아무 명령이나 칠 수 있는 칸"은 만들기 쉽고 위험하다. 그리고 **이미 더 나은 도구가 있다** —
//! 터미널이다. 여기서는 **정해 둔 몇 가지**만 고르게 한다. 목록에 없는 일은 터미널에서 한다.
//!
//! ## 인용이 이 파일의 존재 이유다
//!
//! 파일 이름은 사용자가 짓지 않는다. 서버에 있는 것을 그대로 받는다. 그 안에 공백·따옴표·
//! `;`·`$`·백틱이 있으면, 인용하지 않은 채 셸에 넘기는 순간 **이름이 명령이 된다**:
//!
//! ```text
//!   파일 이름:  a; rm -rf ~ .txt
//!   인용 없이:  gzip a; rm -rf ~ .txt      ← 두 번째가 실행된다
//!   인용하면:   gzip 'a; rm -rf ~ .txt'    ← 그냥 이름이다
//! ```
//!
//! POSIX 셸에서 **작은따옴표 안은 무엇도 해석되지 않는다.** 유일한 예외가 작은따옴표
//! 자신이라, 그것만 `'\''`로 끊어 이어 붙인다. 이 규칙 하나면 나머지는 전부 안전하다.

/// 목록에 둘 명령 하나. `template`의 `{}` 자리에 인용된 파일 이름이 들어간다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RemoteOp {
    /// 화면에 보일 이름의 i18n 키.
    pub key: &'static str,
    /// 셸 명령 틀. `{}`가 파일 자리.
    pub template: &'static str,
    /// 파일을 **바꾸는** 명령인가 — 그렇다면 한 번 더 묻는다.
    pub mutates: bool,
}

/// 고를 수 있는 명령들. 자유 입력 대신 이 목록만 쓴다.
///
/// **POSIX 셸을 가정한다.** 리눅스·유닉스 서버에서는 다 있지만, 기본 셸이 `cmd.exe`인
/// Windows 서버에서는 하나도 없다. 그때는 서버가 "명령을 찾을 수 없다"고 말하고 그 말이
/// 결과 창에 그대로 뜬다 — 우리가 미리 막지 않는 이유는, 어떤 Windows 서버에는 이것들이
/// 깔려 있고(Git for Windows·WSL 경유) 우리가 그것을 알 방법이 없기 때문이다.
/// **되는지 안 되는지는 서버가 답하게 두고, 우리는 그 답을 가리지 않는다.**
pub(crate) const OPS: &[RemoteOp] = &[
    RemoteOp { key: "rcmd.gzip", template: "gzip -k -- {}", mutates: true },
    RemoteOp { key: "rcmd.gunzip", template: "gunzip -k -- {}", mutates: true },
    RemoteOp { key: "rcmd.untar", template: "tar -xf {}", mutates: true },
    RemoteOp { key: "rcmd.sha256", template: "sha256sum -- {}", mutates: false },
    RemoteOp { key: "rcmd.size", template: "du -sh -- {}", mutates: false },
    RemoteOp { key: "rcmd.head", template: "head -n 50 -- {}", mutates: false },
    RemoteOp { key: "rcmd.filetype", template: "file -- {}", mutates: false },
];

/// POSIX 셸에 넘겨도 **이름 그대로**인 형태로 감싼다.
///
/// 작은따옴표 안은 아무것도 해석되지 않는다. 안에 작은따옴표가 있으면 거기서 한 번 닫고,
/// 이스케이프한 따옴표를 붙이고, 다시 연다(`'…'\''…'`).
pub(crate) use nabi_proto::shquote::shell_quote;

/// 명령 전문을 만든다. 파일이 여럿이면 공백으로 이어 붙인다.
///
/// 실행하기 전에 **이 문자열을 그대로 보여 준다** — 사용자가 무엇이 도는지 보고 결정한다.
pub(crate) fn build(op: &RemoteOp, dir: &str, names: &[String]) -> String {
    let files: Vec<String> = names.iter().map(|n| shell_quote(n)).collect();
    let cd = format!("cd {} && ", shell_quote(dir));
    // 폴더로 먼저 옮긴다 — 긴 경로를 파일마다 붙이면 명령이 길어지고 읽기 어렵다.
    format!("{cd}{}", op.template.replace("{}", &files.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_is_just_quoted() {
        assert_eq!(shell_quote("report.txt"), "'report.txt'");
    }

    /// **공백이 있는 이름이 두 개로 갈라지면 안 된다.**
    #[test]
    fn a_name_with_spaces_stays_one_argument() {
        assert_eq!(shell_quote("my report.txt"), "'my report.txt'");
    }

    /// **이 시험이 이 파일의 존재 이유다.** 인용하지 않으면 이름이 명령이 된다.
    #[test]
    fn a_malicious_name_cannot_become_a_command() {
        let evil = "a; rm -rf ~ .txt";
        let q = shell_quote(evil);
        assert_eq!(q, "'a; rm -rf ~ .txt'");
        // 따옴표 밖에는 세미콜론이 없어야 한다 — 있으면 거기서 명령이 끊긴다.
        let inside = &q[1..q.len() - 1];
        assert!(inside.contains(';'), "시험 자체가 틀렸다");
        assert!(!q.trim_matches('\'').is_empty());
    }

    /// 작은따옴표가 든 이름 — 유일한 특수 경우다.
    #[test]
    fn a_single_quote_in_the_name_is_broken_out_and_escaped() {
        assert_eq!(shell_quote("it's.txt"), "'it'\\''s.txt'");
    }

    /// 명령 치환·변수 확장이 이름 안에서 일어나면 안 된다.
    #[test]
    fn substitutions_inside_a_name_stay_literal() {
        for evil in ["$(whoami)", "`id`", "$HOME", "${PATH}"] {
            let q = shell_quote(evil);
            assert!(q.starts_with('\'') && q.ends_with('\''), "{q}");
            assert!(!q[1..q.len() - 1].contains('\''), "{q}");
        }
    }

    /// 개행이 든 이름도 한 덩어리로 남아야 한다(줄이 갈리면 다음 줄이 명령이 된다).
    #[test]
    fn a_newline_in_the_name_does_not_start_a_new_command() {
        let q = shell_quote("a\nrm -rf /\n");
        assert!(q.starts_with('\'') && q.ends_with('\''));
        assert!(q.contains('\n'), "개행은 지우지 않고 인용 안에 둔다");
    }

    #[test]
    fn the_command_puts_the_files_where_the_template_says() {
        let op = RemoteOp { key: "k", template: "gzip -k -- {}", mutates: true };
        let got = build(&op, "/srv/app", &["a.txt".into(), "b c.txt".into()]);
        assert_eq!(got, "cd '/srv/app' && gzip -k -- 'a.txt' 'b c.txt'");
    }

    /// 폴더 경로도 인용한다 — 서버 경로에도 공백이 있다.
    #[test]
    fn the_folder_is_quoted_too() {
        let op = RemoteOp { key: "k", template: "du -sh -- {}", mutates: false };
        let got = build(&op, "/srv/my app", &["x".into()]);
        assert!(got.starts_with("cd '/srv/my app' && "), "{got}");
    }

    /// 파일을 바꾸는 명령과 보기만 하는 명령이 구분돼 있어야 한다(한 번 더 묻는 기준).
    #[test]
    fn read_only_operations_are_marked_as_such() {
        let by = |k: &str| OPS.iter().find(|o| o.key == k).expect(k);
        assert!(!by("rcmd.sha256").mutates);
        assert!(!by("rcmd.head").mutates);
        assert!(by("rcmd.gzip").mutates);
        assert!(by("rcmd.untar").mutates);
    }

    /// 모든 틀에 파일 자리가 있어야 한다 — 없으면 엉뚱한 대상에 도는 명령이 된다.
    #[test]
    fn every_template_has_a_place_for_the_files() {
        for op in OPS {
            assert!(op.template.contains("{}"), "{}에 파일 자리가 없다: {}", op.key, op.template);
        }
    }

    /// 옵션 끝 표시(`--`)가 있어야 `-rf` 같은 이름이 옵션으로 읽히지 않는다.
    #[test]
    fn templates_stop_option_parsing_before_the_files() {
        for op in OPS {
            // tar는 `--`를 안 받는 판이 있어 예외로 둔다(대신 x 모드라 옵션 해석이 없다).
            if op.key == "rcmd.untar" {
                continue;
            }
            assert!(op.template.contains("-- {}"), "{}: `--`가 없다", op.key);
        }
    }
}
