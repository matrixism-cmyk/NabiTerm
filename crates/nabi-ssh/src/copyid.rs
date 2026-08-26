//! **공개키를 서버에 설치한다**(ssh-copy-id) — 키를 만들어 주고 붙여넣기는 손으로 하던 일.
//!
//! 키 만들기는 이미 있다(`keygen`). 정작 그다음 한 걸음 — 서버의 `~/.ssh/authorized_keys`에
//! 넣는 일 — 이 없어서, 사용자는 공개키를 복사해 서버에 붙여 놓고 권한까지 손으로 맞춰야
//! 했다. SSH 설정에서 가장 자주 막히는 자리다.
//!
//! ## 남의 서버 파일을 고치는 일이다
//!
//! 잘못하면 그 서버에 못 들어간다. 그래서 규칙을 좁게 못 박고 순수 함수로 시험한다.
//!
//! * **덮어쓰지 않는다.** 언제나 덧붙이기다. `>` 한 글자 차이로 남의 키가 전부 날아간다.
//! * **이미 있으면 넣지 않는다.** 같은 키가 두 줄이 되면 나중에 지울 때 헷갈린다.
//! * **권한을 맞춘다.** `~/.ssh`는 700, `authorized_keys`는 600이어야 sshd가 받아들인다.
//!   여기서 안 맞추면 "넣었는데 안 되는" 가장 흔한 실패가 된다.
//! * 키 글자는 **인용한다.** 주석에 무엇이 들어 있을지 모른다.

/// 공개키 한 줄에서 **비교에 쓸 부분**만 뽑는다 — `타입 base64`(주석 제외).
///
/// 주석은 사람이 바꾼다(`user@laptop` → `user@새노트북`). 주석까지 견주면 같은 키를
/// 두 번 넣게 된다. 실제로 sshd가 보는 것도 앞의 두 조각뿐이다.
pub fn key_ident(line: &str) -> Option<String> {
    let mut it = line.split_whitespace();
    let (t, b64) = (it.next()?, it.next()?);
    // 타입은 `ssh-`나 `ecdsa-`/`sk-`로 시작한다. 아니면 공개키 줄이 아니다.
    if !t.starts_with("ssh-") && !t.starts_with("ecdsa-") && !t.starts_with("sk-") {
        return None;
    }
    (!b64.is_empty()).then(|| format!("{t} {b64}"))
}

/// 그 키가 이미 파일 안에 있나. 주석 차이는 무시한다.
pub fn already_present(authorized: &str, key_line: &str) -> bool {
    let Some(want) = key_ident(key_line) else {
        return false;
    };
    authorized.lines().filter_map(key_ident).any(|k| k == want)
}

/// 설치 명령을 만든다. **한 줄로 이어 붙이되 각 단계가 실패하면 멈춘다**(`&&`).
///
/// 순서가 뜻을 가진다: 폴더를 만들고 → 권한을 맞추고 → 덧붙이고 → 파일 권한을 맞춘다.
/// 덧붙이기를 먼저 하면 파일이 700 폴더 밖에서 만들어져 sshd가 거부할 수 있다.
pub fn install_command(key_line: &str) -> String {
    let q = quote(key_line.trim());
    // `>>`(덧붙이기)다. `>`가 되면 남의 키가 전부 날아간다 — 이 한 글자가 이 함수의 전부다.
    format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && printf '%s\\n' {q} >> ~/.ssh/authorized_keys \
         && chmod 600 ~/.ssh/authorized_keys && echo nabi-copyid-ok"
    )
}

/// 서버의 현재 `authorized_keys`를 읽는 명령(없으면 빈 글).
pub fn read_command() -> &'static str {
    "cat ~/.ssh/authorized_keys 2>/dev/null || true"
}

/// 설치가 실제로 됐는지 알리는 표시. 명령 끝의 `echo`가 이것을 찍는다.
pub const OK_MARK: &str = "nabi-copyid-ok";

/// POSIX 셸에 넘겨도 글자 그대로인 형태로 감싼다.
///
/// `remotecmd::shell_quote`와 같은 규칙이지만 그쪽은 앱 크레이트에 있다. 여기서 다시
/// 적은 이유는 **이 크레이트가 앱에 기대면 안 되기 때문**이고, 규칙이 짧아 베끼는 값이
/// 의존성보다 싸다. 시험은 양쪽에 다 있다.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\'' => out.push_str("'\\''"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::{already_present, install_command, key_ident, OK_MARK};

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb7GQ2p7DbFPuhVpzOSVQXHDzHfF1lVMSCUmJ8UN0Rp";

    #[test]
    fn the_type_and_body_make_the_identity() {
        assert_eq!(key_ident(&format!("{ED} user@host")).as_deref(), Some(ED));
    }

    /// **주석이 달라도 같은 키다** — 주석까지 견주면 같은 키를 두 번 넣게 된다.
    #[test]
    fn a_different_comment_is_still_the_same_key() {
        let a = format!("{ED} user@laptop");
        let b = format!("{ED} user@새노트북");
        assert_eq!(key_ident(&a), key_ident(&b));
        assert!(already_present(&a, &b));
    }

    #[test]
    fn a_line_that_is_not_a_key_has_no_identity() {
        assert_eq!(key_ident(""), None);
        assert_eq!(key_ident("# 주석"), None);
        assert_eq!(key_ident("ssh-ed25519"), None, "본문이 없다");
        assert_eq!(key_ident("hello world"), None);
    }

    #[test]
    fn a_missing_key_is_reported_as_missing() {
        let other = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL0mBUvbdSQVwrpBnCcxvB7YkAKAP1DsBOTFdHRqNZKm";
        assert!(!already_present(other, ED));
        assert!(!already_present("", ED));
    }

    /// 주석·빈 줄이 섞인 진짜 파일에서도 찾아낸다.
    #[test]
    fn it_finds_the_key_in_a_real_looking_file() {
        let file = format!("# keys\n\nssh-rsa AAAAB3Nz other@host\n{ED} me@here\n");
        assert!(already_present(&file, ED));
    }

    /// **덮어쓰기가 아니라 덧붙이기여야 한다.** 이 시험이 이 파일의 존재 이유다.
    #[test]
    fn the_command_appends_and_never_overwrites() {
        let c = install_command(ED);
        assert!(c.contains(">> ~/.ssh/authorized_keys"), "{c}");
        // `>>`는 `> `를 품으므로 글자 찾기로는 못 가른다. **꺾쇠가 딱 둘이고 붙어 있는지**를 본다.
        let angles: Vec<usize> = c.char_indices().filter(|(_, ch)| *ch == '>').map(|(i, _)| i).collect();
        assert_eq!(angles.len(), 2, "꺾쇠가 둘이 아니다(덮어쓰기가 섞였을 수 있다): {c}");
        assert_eq!(angles[1], angles[0] + 1, "떨어진 꺾쇠 = 덮어쓰기: {c}");
    }

    /// 권한을 맞추지 않으면 sshd가 거부한다 — "넣었는데 안 되는" 가장 흔한 실패.
    #[test]
    fn the_command_fixes_the_permissions() {
        let c = install_command(ED);
        assert!(c.contains("chmod 700 ~/.ssh"), "{c}");
        assert!(c.contains("chmod 600 ~/.ssh/authorized_keys"), "{c}");
    }

    /// 각 단계가 실패하면 멈춘다 — 폴더를 못 만들었는데 덧붙이면 안 된다.
    #[test]
    fn the_steps_stop_on_failure() {
        assert!(!install_command(ED).contains(';'), "세미콜론으로 이으면 실패해도 계속 간다");
    }

    /// 끝났다는 표시가 있어야 성공을 확인할 수 있다.
    #[test]
    fn the_command_says_when_it_worked() {
        assert!(install_command(ED).contains(OK_MARK));
    }

    /// 키 줄에 든 따옴표가 명령을 깨뜨리면 안 된다(주석은 사람이 적는다).
    #[test]
    fn a_quote_in_the_comment_cannot_break_the_command() {
        let c = install_command(&format!("{ED} it's mine"));
        // 인용 안에 있으므로 셸이 해석하지 않는다.
        assert!(c.contains("'\\''"), "{c}");
    }
}

#[cfg(test)]
mod print_cmd {
    /// 실서버 손검증용 — 실제 명령을 눈으로 보려고 찍는다(`--nocapture`).
    #[test]
    #[ignore = "명령 확인용(수동)"]
    fn print_install_command() {
        let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb7GQ2p7DbFPuhVpzOSVQXHDzHfF1lVMSCUmJ8UN0Rp nabi-copyid-test";
        println!("{}", super::install_command(key));
    }
}
