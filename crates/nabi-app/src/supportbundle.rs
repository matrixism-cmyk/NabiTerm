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

/// 비밀로 보이는 것을 지운다 — 규칙은 `crate::redact`에 있다.
///
/// 원래 이 파일에 규칙이 있었는데, 명령 기록·세션 로그도 같은 보호가 필요해지면서
/// 밖으로 꺼냈다. 규칙이 두 벌이면 한쪽만 강해지고 다른 쪽은 조용히 뒤처진다.
pub(crate) use crate::redact::redact;
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

    /// 규칙 자체의 시험은 `crate::redact`로 옮겼다 — 그쪽이 사실의 출처다.
    /// 여기서는 **번들이 그 규칙을 실제로 통과시키는지**만 본다.
    #[test]
    fn the_bundle_uses_the_shared_rules() {
        let got = redact("password: hunter2");
        assert!(got.contains("[redacted]") && !got.contains("hunter2"), "{got}");
        let key = redact("-----BEGIN OPENSSH PRIVATE KEY-----");
        assert_eq!(key, "[redacted private key]");
    }

    /// 평범한 줄은 건드리지 않는다 — 다 지우면 진단이 안 된다.
    #[test]
    fn ordinary_lines_survive() {
        let l = "2026-08-25 INFO 연결 성공 host=example.com";
        assert_eq!(redact(l), l);
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
