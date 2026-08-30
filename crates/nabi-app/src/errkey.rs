//! **아래 층이 보낸 오류를 사람 말로.**
//!
//! SFTP·전송 층은 화면에 무슨 말을 띄울지 모른다 — 사용자가 어떤 언어로 쓰는지 아는 곳은
//! 여기뿐이다. 그래서 그 층은 **i18n 키를 그대로** 오류 문자열로 올려보내고, 화면에 닿기
//! 직전 이 함수 하나가 우리말로 바꾼다.
//!
//! 원래는 원격 명령 결과 창이 `"sftp.exec.ftp"` 하나만 따로 손보고 있었다. 같은 일이
//! 폴더 복사에서 또 필요해졌으므로, 자리마다 늘어놓는 대신 한 곳으로 모은다.
//!
//! 키가 아니면 **손대지 않는다.** 서버가 보낸 진짜 오류문을 우리 말로 덮으면 무엇이
//! 잘못됐는지 알 길이 사라진다.

use nabi_i18n::{tr, Lang};

/// 아래 층이 올려보낸 문자열이 i18n 키면 옮기고, 아니면 그대로 돌려준다.
pub(crate) fn human(lang: Lang, msg: &str) -> String {
    // `키:숫자` 꼴은 개수를 함께 보여 준다("건너뛴 것 3개").
    if let Some((k, n)) = msg.split_once(':') {
        if is_key(k) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            let t = tr(lang, k);
            if t != "?" && t != k {
                return format!("{t} ({n})");
            }
        }
    }
    if !is_key(msg) {
        return msg.to_string();
    }
    let t = tr(lang, msg);
    // **모르는 키에 tr는 "?"를 돌려준다.** 그대로 쓰면 화면에 물음표 하나만 뜨고,
    // 무엇이 잘못됐는지 아무도 모른다. 그럴 때는 원문(키)을 그대로 보여 준다.
    match t == "?" || t == msg {
        true => msg.to_string(),
        false => t.to_string(),
    }
}

/// 키처럼 생겼나 — 점으로 나뉜 소문자 낱말들. 서버 오류문에는 공백·대문자가 섞인다.
fn is_key(s: &str) -> bool {
    s.contains('.')
        && !s.contains(' ')
        && s.len() < 64
        && s.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{human, is_key};
    use nabi_i18n::Lang;

    #[test]
    fn a_known_key_becomes_a_sentence() {
        let s = human(Lang::Ko, "sftp.copy.ftp");
        assert!(s.contains("FTP"), "{s}");
        assert!(s != "sftp.copy.ftp", "키가 그대로 나왔다");
    }

    /// **서버가 보낸 진짜 오류문은 건드리지 않는다** — 덮으면 원인을 잃는다.
    #[test]
    fn a_real_server_message_is_left_alone() {
        for m in ["Permission denied", "No such file or directory", "취소됨"] {
            assert_eq!(human(Lang::Ko, m), m);
        }
    }

    /// 파일 이름이 든 오류문은 키가 아니다(점이 있다고 키가 아니다).
    #[test]
    fn a_filename_is_not_mistaken_for_a_key() {
        assert!(!is_key("failed to open report.txt"));
        assert!(!is_key("/srv/App.log"));
    }

    /// 모르는 키는 원문 그대로 — 빈 화면보다 낫다.
    #[test]
    fn an_unknown_key_falls_back_to_itself() {
        assert_eq!(human(Lang::Ko, "nope.not.a.real.key"), "nope.not.a.real.key");
    }
}

#[cfg(test)]
mod 개수가_붙은_키 {
    use nabi_i18n::Lang;

    /// `키:숫자` 는 사람 말 + 개수로 보여 준다.
    #[test]
    fn 개수를_함께_보여_준다() {
        let s = super::human(Lang::Ko, "sftp.skipped.unsafe:3");
        assert!(s.contains("(3)"), "개수가 빠졌다: {s}");
        assert!(!s.contains("sftp.skipped"), "키가 그대로 보인다: {s}");
    }

    /// 모르는 키는 원문 그대로 — 물음표 하나만 뜨면 아무도 뭘 모른다.
    #[test]
    fn 모르는_키는_그대로_둔다() {
        assert_eq!(super::human(Lang::Ko, "no.such.key:7"), "no.such.key:7");
    }

    /// 서버가 준 오류문에 우연히 콜론이 있어도 망가뜨리지 않는다.
    #[test]
    fn 보통_오류문은_건드리지_않는다() {
        let raw = "Permission denied: /etc/shadow";
        assert_eq!(super::human(Lang::Ko, raw), raw);
    }
}
