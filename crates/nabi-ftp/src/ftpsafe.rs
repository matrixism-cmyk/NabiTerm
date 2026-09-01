//! FTP 제어 채널에 **줄바꿈을 흘려보내지 않는다**(RUSTSEC-2026-0271 완화).
//!
//! ## 무슨 일인가
//!
//! FTP 는 명령을 한 줄에 하나씩 `CRLF` 로 끝내 보낸다. 우리가 쓰는 `suppaftp` 8.0.3 은
//! 인자(사용자 이름·비밀번호·경로·`SITE` 인자)를 **검사 없이** 그 줄에 이어 붙였다.
//! 그래서 인자 안에 `\r` 이나 `\n` 이 하나 있으면 거기서 명령이 끝나고, **그다음 글이
//! 두 번째 명령**이 된다 — 이미 로그인한 그 세션의 권한으로.
//!
//! ```text
//! CWD 폴더\r\nDELE 중요파일     →  서버는 CWD 와 DELE 두 개를 받는다
//! ```
//!
//! 권고문은 두 가지를 말한다: 10.0.2 이상으로 올리거나, **올릴 수 없으면 넘기기 전에
//! CR·LF 를 막으라**고. 여기가 그 막는 자리다.
//!
//! ## 왜 경계에서도 막나
//!
//! 라이브러리를 올려도 이 검사는 남긴다. 우리가 쓰는 값은 사람이 적은 것(호스트·계정·
//! 경로)과 서버가 준 목록에서 온 것이라, **어느 쪽도 우리가 만든 것이 아니다.**
//! 오늘 ssh 설정 내보내기에서도 같은 모양을 막았다 — 줄로 이루어진 규약에 값을 그대로
//! 이어 붙이는 곳은 전부 같은 함정을 갖는다.

/// FTP 명령 인자로 써도 되는가 — 줄을 가르는 글자가 없어야 한다.
///
/// `\r`·`\n` 뿐 아니라 NUL 도 막는다(C 문자열로 넘어가는 서버에서 잘릴 수 있다).
/// 그 밖의 글자는 건드리지 않는다 — 한글 파일 이름은 FTP 에서 흔하다.
pub fn arg_ok(v: &str) -> bool {
    !v.contains(['\r', '\n', '\0'])
}

/// 인자를 검사하고, 안 되면 사람이 읽을 오류를 돌려준다.
///
/// `#[must_use]` 인 이유: 검사해 놓고 답을 버리면 막은 것이 없다.
#[must_use = "막았는지 확인하지 않으면 그대로 흘러간다"]
pub fn check(v: &str) -> Result<(), String> {
    match arg_ok(v) {
        true => Ok(()),
        false => Err(nabi_i18n::trc("net.ftp.badarg").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{arg_ok, check};

    /// ① 평범한 값은 통과해야 한다 — 여기서 막으면 멀쩡한 사람이 파일을 못 올린다.
    #[test]
    fn ordinary_arguments_pass() {
        for v in ["/pub/data", "내 문서/보고서.hwp", "user@example.com", "p@ssw0rd!#$", ""] {
            assert!(arg_ok(v), "{v:?} 는 통과해야 한다");
        }
    }

    /// ② 줄을 가르는 것은 막는다 — 이것이 두 번째 명령을 만든다.
    #[test]
    fn a_newline_would_start_a_second_command() {
        for v in [
            "폴더\r\nDELE 중요파일", // 전형적인 주입
            "path\nDELE x",          // LF 만으로도 되는 서버가 있다
            "path\rDELE x",
            "user\r\nPASS wrong",
            "a\0b",
        ] {
            assert!(!arg_ok(v), "{v:?} 는 막아야 한다");
            assert!(check(v).is_err());
        }
    }
}
