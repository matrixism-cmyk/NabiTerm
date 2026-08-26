//! **알던 호스트키가 바뀌었을 때** 필요한 순수 계산 — 옛 줄 읽기, 그 줄만 지우기.
//!
//! 지금까지 이 경우는 `Err(_) => Ok(false)`로 뭉개져 있었다. 거부한다는 판단 자체는 옳다.
//! 틀린 것은 **아무 말도 하지 않는다**는 점이었다 — 사용자에게는 그냥 접속이 안 되는
//! 서버로 보이고, 왜인지 알 길도 고칠 길도 없었다.
//!
//! ## 우리는 원인을 모른다
//!
//! 중간자 공격과 서버 재설치는 **겉으로 똑같이 생겼다.** 그러니 프로그램이 대신 판단해서는
//! 안 된다. 여기서 하는 일은 사실을 모으는 것뿐이다: 어느 줄이 걸렸나, 그 줄의 지문은
//! 무엇이었나. 판단은 지문을 서버 관리자와 대조할 수 있는 사람이 한다.
//!
//! ## 지울 때는 그 줄만
//!
//! 이미 있는 `known_hosts_remove`는 **그 호스트의 항목을 전부** 지운다. 한 호스트가 여러
//! 알고리즘 키를 가질 수 있으므로(ed25519 · rsa …), 바뀐 하나 때문에 나머지까지 지우면
//! 멀쩡한 신뢰를 잃는다. 그래서 줄 번호로 하나만 지운다.

/// `known_hosts`의 그 줄에 적혀 있던 지문(SHA256). 읽지 못하면 빈 문자열.
///
/// `line`은 1부터 센다(russh가 그렇게 돌려준다).
pub fn old_fingerprint(content: &str, line: usize) -> String {
    // 줄 번호는 1부터다. 0을 빼기로 뭉개면 **1번 줄의 지문**이 나와, 사용자에게 엉뚱한
    // 옛 지문을 보여 주게 된다 — 대조하는 화면에서 이보다 나쁜 결함은 없다.
    let Some(text) = line.checked_sub(1).and_then(|i| content.lines().nth(i)) else {
        return String::new();
    };
    // 파일에는 base64로 적혀 있다 — 지문을 내려면 키로 되돌려야 한다.
    let Some(e) = crate::knownhosts::parse_known_hosts_line(text) else {
        return String::new();
    };
    let line = format!("{} {}", e.key_type, e.key_b64);
    match russh::keys::PublicKey::from_openssh(&line) {
        Ok(k) => crate::fingerprint::sha256_fingerprint(&k),
        // 우리가 못 읽는 꼴이면 **비워 둔다**. 못 읽은 것을 아는 척하면 대조가 거짓이 된다.
        Err(_) => String::new(),
    }
}

/// 그 줄 **하나만** 지운 새 내용. 줄 번호가 범위를 벗어나면 그대로 돌려준다.
///
/// 끝의 줄바꿈은 살린다 — 없애면 다음에 덧붙일 때 두 항목이 한 줄로 붙는다.
pub fn remove_line(content: &str, line: usize) -> String {
    let n = content.lines().count();
    if line == 0 || line > n {
        return content.to_string();
    }
    let kept: Vec<&str> = content
        .lines()
        .enumerate()
        .filter(|(i, _)| *i + 1 != line)
        .map(|(_, l)| l)
        .collect();
    match kept.is_empty() {
        true => String::new(),
        false => format!("{}\n", kept.join("\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::{old_fingerprint, remove_line};

    /// 실제 known_hosts 한 줄(공개키라 적어도 무해하다 — 시험용 값).
    const A: &str = "example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb7GQ2p7DbFPuhVpzOSVQXHDzHfF1lVMSCUmJ8UN0Rp";
    const B: &str = "other.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL0mBUvbdSQVwrpBnCcxvB7YkAKAP1DsBOTFdHRqNZKm";

    #[test]
    fn the_old_fingerprint_comes_from_that_line() {
        let content = format!("{A}\n{B}\n");
        let f1 = old_fingerprint(&content, 1);
        let f2 = old_fingerprint(&content, 2);
        assert!(f1.starts_with("SHA256:"), "{f1}");
        assert_ne!(f1, f2, "다른 줄인데 같은 지문이 나왔다");
    }

    /// 없는 줄을 물으면 **지어내지 않는다**(빈 값이 곧 "모른다"는 답이다).
    #[test]
    fn a_line_out_of_range_yields_nothing() {
        let content = format!("{A}\n");
        assert_eq!(old_fingerprint(&content, 0), "");
        assert_eq!(old_fingerprint(&content, 9), "");
        assert_eq!(old_fingerprint("# 주석뿐\n", 1), "");
    }

    /// **그 줄만 지운다** — 같은 호스트의 다른 키까지 지우면 멀쩡한 신뢰를 잃는다.
    #[test]
    fn only_the_named_line_is_removed() {
        let content = format!("{A}\n{B}\n");
        let out = remove_line(&content, 1);
        assert!(!out.contains("example.com"), "{out}");
        assert!(out.contains("other.com"), "옆 줄까지 지웠다");
    }

    /// 지운 뒤에도 끝에 줄바꿈이 남아야 한다(다음 항목이 붙어 버리지 않게).
    #[test]
    fn the_trailing_newline_survives() {
        let out = remove_line(&format!("{A}\n{B}\n"), 2);
        assert!(out.ends_with('\n'), "{out:?}");
    }

    #[test]
    fn a_bad_line_number_changes_nothing() {
        let content = format!("{A}\n{B}\n");
        assert_eq!(remove_line(&content, 0), content);
        assert_eq!(remove_line(&content, 99), content);
    }

    /// 한 줄뿐이면 빈 파일이 된다(빈 줄 하나를 남기지 않는다).
    #[test]
    fn removing_the_only_line_empties_the_file() {
        assert_eq!(remove_line(&format!("{A}\n"), 1), "");
    }
}
