//! **진단 묶음** — 문제를 남에게 물어볼 때 필요한 것을 한 파일로 모은다.
//!
//! 로그 보기 창은 이미 있다. 그런데 "이 화면을 복사해서 보내 주세요"는 사람마다 다르게
//! 잘라 보내고, 정작 필요한 판 번호나 환경이 빠진다. 한 번에 모아 주는 편이 낫다.
//!
//! ## 무엇을 넣지 않는가 — 이쪽이 더 중요하다
//!
//! 진단 묶음은 **남에게 보내는 것**이다. 그래서 넣지 않는 것을 먼저 정한다.
//!
//! * 비밀번호·토큰·키는 **어떤 형태로도** 넣지 않는다. 볼트는 통째로 제외한다.
//! * 호스트 이름·사용자 이름은 남의 인프라 정보다 — 세션 **개수**만 센다.
//! * 로그에 섞여 들어갔을 수 있는 비밀 꼴은 **지운다**(아래 `redact`).
//!
//! 그리고 **보내기 전에 무엇이 들어가는지 보여 준다.** 모르고 보내게 하지 않는다.

/// 묶음에 담을 한 조각.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    /// 사람이 읽는 이름(미리보기 목록에 뜬다).
    pub title: String,
    pub body: String,
}

/// 비밀로 보이는 것을 지운다.
///
/// 넉넉하게 지운다 — 지나치게 지워서 진단이 조금 어려워지는 쪽이, 토큰 하나가 새는 쪽보다
/// 훨씬 낫다. 되돌릴 수 없는 것은 유출이지 불편이 아니다.
pub(crate) fn redact(text: &str) -> String {
    text.lines().map(redact_line).collect::<Vec<_>>().join("\n")
}

/// 이 낱말 뒤에 오는 값은 지운다.
const SECRET_KEYS: &[&str] = &[
    "password", "passwd", "passphrase", "secret", "token", "authorization",
    "api_key", "apikey", "credential", "bearer",
];

fn redact_line(line: &str) -> String {
    let low = line.to_ascii_lowercase();
    if SECRET_KEYS.iter().any(|k| low.contains(k)) {
        // 낱말은 남기고 값만 지운다 — 무엇이 지워졌는지는 보여야 진단에 쓸모가 있다.
        return match line.find([':', '=']) {
            // 구분자 앞의 공백은 떼어 낸다 — "Authorization : [redacted]"처럼 보이면 지저분하다.
            Some(i) => format!("{}: [redacted]", line[..i].trim_end()),
            None => "[redacted]".to_string(),
        };
    }
    // 개인키 본문이 통째로 섞여 들어간 경우.
    if low.contains("begin openssh private key") || low.contains("begin rsa private key") {
        return "[redacted private key]".to_string();
    }
    line.to_string()
}

/// 묶음 전체를 하나의 글로 엮는다.
pub(crate) fn assemble(pieces: &[Piece]) -> String {
    let mut out = String::new();
    for p in pieces {
        out.push_str(&format!("===== {} =====\n", p.title));
        out.push_str(p.body.trim_end());
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_line_keeps_its_label_but_loses_its_value() {
        assert_eq!(redact_line("password: hunter2"), "password: [redacted]");
        assert_eq!(redact_line("  Authorization = Bearer abc123"), "  Authorization: [redacted]");
    }

    /// **대소문자를 가리지 않는다** — 로그는 온갖 표기로 온다.
    #[test]
    fn redaction_ignores_case() {
        assert!(redact_line("PASSWORD=x").contains("[redacted]"));
        assert!(redact_line("Api_Key: x").contains("[redacted]"));
    }

    /// 개인키가 통째로 섞여 들어간 경우도 지운다.
    #[test]
    fn a_private_key_block_is_removed() {
        assert_eq!(redact_line("-----BEGIN OPENSSH PRIVATE KEY-----"), "[redacted private key]");
    }

    /// 평범한 줄은 건드리지 않는다 — 다 지우면 진단이 안 된다.
    #[test]
    fn ordinary_lines_survive() {
        let l = "2026-08-25 INFO 연결 성공 host=example.com";
        assert_eq!(redact_line(l), l);
    }

    /// 값에 구분자가 없어도 통째로 지운다(값이 남는 것보다 낫다).
    #[test]
    fn a_secret_without_a_separator_is_dropped_whole() {
        assert_eq!(redact_line("token abc123"), "[redacted]");
    }

    #[test]
    fn redaction_works_across_lines() {
        let got = redact("ok\npassword: x\nfine");
        assert_eq!(got, "ok\npassword: [redacted]\nfine");
    }

    /// 엮은 결과에 각 조각의 제목이 있어야 받는 사람이 읽을 수 있다.
    #[test]
    fn the_bundle_labels_each_piece() {
        let p = vec![
            Piece { title: "판".into(), body: "0.1.468".into() },
            Piece { title: "로그".into(), body: "line".into() },
        ];
        let out = assemble(&p);
        assert!(out.contains("===== 판 =====") && out.contains("===== 로그 ====="));
        assert!(out.contains("0.1.468") && out.contains("line"));
    }

    #[test]
    fn an_empty_bundle_is_empty() {
        assert_eq!(assemble(&[]), "");
    }
}
