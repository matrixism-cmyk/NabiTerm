//! WSL 배포판 목록 — **선택지를 대신 읽어다 준다.**
//!
//! `wsl --install -d <이름>`을 알려 줘도 어떤 이름을 쓸 수 있는지 모르면 소용이 없다.
//! 그 목록을 사람이 명령으로 캐게 하지 않는 것이 이 모듈의 존재 이유다.
//!
//! ## 함정: wsl.exe는 UTF-16LE로 쓴다
//!
//! 출력 바이트를 그대로 UTF-8로 읽으면 글자 사이에 NUL이 낀 쓰레기가 나온다(2026-08-25에
//! 직접 확인했다). 반드시 UTF-16LE로 디코딩해야 한다.
//!
//! ## 함정: 머리말이 번역된다
//!
//! 안내 문장은 시스템 언어로 나온다("설치할 수 있는 배포…"). 그래서 특정 낱말을 찾아
//! 자르면 한국어 PC에서만, 또는 영어 PC에서만 동작한다. 대신 **줄의 생김새**로 가른다 —
//! 배포판 이름은 언제나 ASCII 식별자다.

/// 설치할 수 있는 배포판 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Distro {
    /// `wsl --install -d`에 넘길 이름.
    pub name: String,
    /// 사람이 읽는 이름.
    pub friendly: String,
}

/// wsl.exe가 뱉은 바이트를 글자로 되돌린다. UTF-16LE이 아니면 UTF-8로 본다.
pub(crate) fn decode(raw: &[u8]) -> String {
    // 홀수 길이거나 NUL이 거의 없으면 UTF-16이 아니다(순수 ASCII UTF-8은 NUL이 없다).
    let nulls = raw.iter().filter(|b| **b == 0).count();
    if raw.len() < 2 || !raw.len().is_multiple_of(2) || nulls * 3 < raw.len() {
        return String::from_utf8_lossy(raw).into_owned();
    }
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

/// `wsl --list --online` 출력에서 배포판 줄만 골라낸다.
pub(crate) fn parse_online(text: &str) -> Vec<Distro> {
    text.lines().filter_map(online_row).collect()
}

/// 한 줄이 배포판 줄이면 (이름, 사람이 읽는 이름).
fn online_row(line: &str) -> Option<Distro> {
    let t = line.trim().trim_start_matches('*').trim();
    let (name, rest) = t.split_once(char::is_whitespace)?;
    let friendly = rest.trim();
    // 이름은 ASCII 식별자다 — 번역된 안내 문장과 이것으로 갈린다.
    let ok = !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
        && !name.eq_ignore_ascii_case("NAME")
        && !friendly.is_empty()
        && friendly.chars().all(|c| c != '.' || !friendly.ends_with('.'));
    ok.then(|| Distro { name: name.to_string(), friendly: friendly.to_string() })
}

/// `wsl --list --quiet` 출력 → 이미 깔린 배포판 이름들.
pub(crate) fn parse_installed(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_start_matches('*').trim().to_string())
        .filter(|l| !l.is_empty() && l.is_ascii())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 실제 wsl.exe가 내놓는 UTF-16LE 바이트(한국어 안내 + 영어 표 머리말).
    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    #[test]
    fn utf16_output_is_decoded() {
        let raw = utf16("Ubuntu  Ubuntu\n");
        assert_eq!(decode(&raw), "Ubuntu  Ubuntu\n");
    }

    /// UTF-8로 오는 경우도 있다 — 그때 UTF-16으로 우기면 전부 깨진다.
    #[test]
    fn plain_utf8_output_survives() {
        assert_eq!(decode(b"Ubuntu  Ubuntu\n"), "Ubuntu  Ubuntu\n");
    }

    /// **번역된 안내 문장을 배포판으로 착각하면 안 된다** — 이게 이 파서의 전부다.
    #[test]
    fn translated_prose_is_not_a_distro() {
        let text = "\
설치할 수 있는 유효한 배포 목록입니다.
'wsl --install -d <Distro>'을(를) 사용하여 설치하십시오.

  NAME                    FRIENDLY NAME
* Ubuntu                  Ubuntu
  Debian                  Debian GNU/Linux
  kali-linux              Kali Linux Rolling
  OracleLinux_9_5         Oracle Linux 9.5
";
        let got = parse_online(text);
        assert_eq!(got.len(), 4, "가려낸 것: {got:?}");
        assert_eq!(got[0], Distro { name: "Ubuntu".into(), friendly: "Ubuntu".into() });
        assert_eq!(got[3].name, "OracleLinux_9_5");
        assert!(!got.iter().any(|d| d.name.eq_ignore_ascii_case("NAME")), "머리말이 섞였다");
    }

    /// 영어 PC에서도 같은 결과여야 한다.
    #[test]
    fn the_english_header_is_also_skipped() {
        let text = "The following is a list of valid distributions.\n\n  NAME     FRIENDLY NAME\n  Ubuntu   Ubuntu\n";
        assert_eq!(parse_online(text), vec![Distro { name: "Ubuntu".into(), friendly: "Ubuntu".into() }]);
    }

    #[test]
    fn installed_list_is_read() {
        let text = "Ubuntu\ndocker-desktop\n\n";
        assert_eq!(parse_installed(text), vec!["Ubuntu".to_string(), "docker-desktop".to_string()]);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(parse_online("").is_empty());
        assert!(parse_installed("").is_empty());
        assert_eq!(decode(b""), "");
    }
}
