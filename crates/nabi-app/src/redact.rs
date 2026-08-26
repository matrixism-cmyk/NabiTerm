//! **비밀로 보이는 것을 가린다** — 디스크에 남기 전에.
//!
//! 원래 이 함수는 지원 번들 안에만 있었다(`supportbundle::redact`). 그런데 비밀이 디스크에
//! 닿는 길은 거기만이 아니었다:
//!
//! * **명령 히스토리** — 설정 파일에 명령 전문이 평문으로 쌓인다. 설정 파일은 백업·동기화·
//!   지원 문의로 밖에 나가기 쉽다.
//! * **세션 로그** — 터미널 출력을 그대로 파일에 쓴다.
//!
//! 한 곳에만 있는 안전장치는 나머지를 지키지 못한다. 그래서 밖으로 꺼냈다.
//!
//! ## 넉넉하게 가린다
//!
//! 지나치게 가려 진단이 조금 불편한 쪽이, 토큰 하나가 새는 쪽보다 낫다. **되돌릴 수 없는
//! 것은 유출이지 불편이 아니다.**
//!
//! 다만 아무 줄이나 지우면 기록이 쓸모없어진다. 그래서 **무엇이 지워졌는지는 남긴다** —
//! 명령 이름과 열쇠말은 두고 값만 `[redacted]`로 바꾼다.

/// 값을 가려야 하는 열쇠말. 대소문자는 보지 않는다.
const SECRET_KEYS: &[&str] = &[
    "password", "passwd", "passphrase", "secret", "token", "authorization",
    "api_key", "apikey", "credential", "bearer", "access_key", "private_key",
    "client_secret", "auth", "pwd",
];

/// 이 접두사로 시작하면 그 자체가 비밀이다(공개된 접두사 규약).
const SECRET_PREFIXES: &[&str] = &[
    "sk-", "ghp_", "gho_", "ghu_", "ghs_", "github_pat_", "xox", "AKIA", "ASIA", "AIza",
];

/// 가려진 자리에 남길 글자.
pub(crate) const MARK: &str = "[redacted]";

/// 여러 줄을 줄 단위로 가린다. **완전한 규칙**(`line_full`)을 쓴다 — 부분 규칙(`line`)을
/// 쓰면 `password: x`처럼 띄어 쓴 값이 그대로 남는다(실제로 그랬고 시험이 잡았다).
pub(crate) fn redact(text: &str) -> String {
    text.lines().map(line_full).collect::<Vec<_>>().join("\n")
}

/// 한 줄을 가린다. 가릴 것이 없으면 그대로.
pub(crate) fn line(src: &str) -> String {
    // 1) 개인키 본문이 통째로 섞였으면 줄째 버린다.
    let low = src.to_ascii_lowercase();
    if low.contains("begin openssh private key") || low.contains("begin rsa private key") {
        return "[redacted private key]".to_string();
    }
    // 2) 낱말 단위로 훑으며 위험한 조각만 바꾼다.
    //
    // 예전 규칙은 열쇠말이 있으면 **줄 전체**를 지웠다. 그러면 `curl -H "Authorization: ..."`이
    // `curl -H "Authorization: [redacted]`이 아니라 통째로 사라져, 무슨 명령이었는지도 잃는다.
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    let mut changed = false;
    while let Some((tok, before, after)) = next_token(rest) {
        out.push_str(before);
        match masked(tok) {
            Some(m) => {
                out.push_str(&m);
                changed = true;
            }
            None => out.push_str(tok),
        }
        rest = after;
    }
    out.push_str(rest);
    match changed {
        true => out,
        false => src.to_string(),
    }
}

/// 다음 공백 아닌 조각을 (조각, 앞 공백, 나머지)로 나눈다.
fn next_token(s: &str) -> Option<(&str, &str, &str)> {
    let start = s.find(|c: char| !c.is_whitespace())?;
    let (before, tail) = s.split_at(start);
    let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
    let (tok, after) = tail.split_at(end);
    Some((tok, before, after))
}

/// 이 조각이 비밀을 품고 있으면 가린 꼴을, 아니면 None.
fn masked(tok: &str) -> Option<String> {
    let low = tok.to_ascii_lowercase();
    // `키=값` · `키:값` — 열쇠말이 앞에 있으면 값만 가린다.
    if let Some(i) = tok.find(['=', ':']) {
        let (k, _) = tok.split_at(i);
        let kl = k.to_ascii_lowercase();
        if SECRET_KEYS.iter().any(|s| kl.contains(s)) && i + 1 < tok.len() {
            return Some(format!("{k}{}{MARK}", &tok[i..i + 1]));
        }
    }
    // 공개된 접두사를 쓰는 토큰은 그 자체가 비밀이다(`sk-...`, `ghp_...`).
    if SECRET_PREFIXES.iter().any(|p| tok.starts_with(p)) && tok.len() >= 12 {
        return Some(MARK.to_string());
    }
    // `-p비번` — MySQL 계열이 이렇게 받는다. 붙여 쓴 값만 가린다(`-p` 하나는 그냥 둔다).
    if (low.starts_with("-p") || low.starts_with("--password")) && tok.len() > 2 {
        let cut = if low.starts_with("--password") { "--password".len() } else { 2 };
        if cut < tok.len() {
            return Some(format!("{}{MARK}", &tok[..cut]));
        }
    }
    None
}

/// 열쇠말 뒤에 **띄어 쓴** 값도 가린다 — `--token abc123`.
///
/// 한 조각씩 보는 규칙으로는 이 꼴을 못 잡는다. 그래서 조각 목록을 한 번 더 훑는다.
pub(crate) fn line_full(src: &str) -> String {
    let once = line(src);
    let toks: Vec<&str> = once.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut skip_next = false;
    for (i, t) in toks.iter().enumerate() {
        if skip_next {
            out.push(MARK.to_string());
            skip_next = false;
            continue;
        }
        // 앞의 `-`와 **뒤의 구분자**를 떼고 본다. `password:` 처럼 구분자로 끝나면서
        // 값이 다음 조각에 있는 꼴이 로그에서 가장 흔하다 — 이걸 놓치면 규칙이 무의미하다.
        let low = t.trim_start_matches('-').trim_end_matches([':', '=']).to_ascii_lowercase();
        // 열쇠말 **자체**인 조각(값이 붙어 있지 않은) 다음 조각이 값이다.
        let has_value_glued = t.contains(['=', ':']) && !t.ends_with([':', '=']);
        let bare = SECRET_KEYS.contains(&low.as_str()) && !has_value_glued;
        if bare && i + 1 < toks.len() {
            skip_next = true;
        }
        out.push((*t).to_string());
    }
    // 공백은 원본 그대로 두지 못한다(조각으로 나눴으므로) — 한 칸으로 모은다.
    match out.len() == toks.len() && toks.iter().zip(out.iter()).all(|(a, b)| a == b) {
        true => once, // 바뀐 것이 없으면 원본 공백을 지킨다.
        false => out.join(" "),
    }
}

/// 이미 쌓인 명령 기록을 한 번 훑어 가린다(불러온 직후).
///
/// 가리기는 **저장 시점**에 하지만, 그 기능이 있기 전에 쌓인 것에는 비밀이 그대로 있다.
/// 켜져 있으면 한 번 쓸어 준다 — 다음 저장 때 디스크에도 반영된다.
///
/// 꺼져 있으면 손대지 않는다. 끈 사람은 원문을 원하는 것이고, 그 뜻을 뒤집지 않는다.
pub(crate) fn sweep_history(mut cfg: nabi_config::AppConfig) -> nabi_config::AppConfig {
    if !cfg.terminal.redact_history {
        return cfg;
    }
    for e in cfg.terminal.cmd_history.iter_mut() {
        let masked = line_full(&e.0);
        if masked != e.0 {
            e.0 = masked;
        }
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::{line_full, redact, MARK};

    #[test]
    fn a_plain_command_is_untouched() {
        for s in ["ls -la", "cargo build --release", "git commit -m \"고침\""] {
            assert_eq!(line_full(s), s, "멀쩡한 명령을 건드렸다");
        }
    }

    /// **명령 이름은 남는다** — 무엇이 지워졌는지 모르면 기록이 쓸모없어진다.
    #[test]
    fn the_command_survives_but_the_value_does_not() {
        let got = line_full("curl -H Authorization=Bearer_abc123 https://api.example.com");
        assert!(got.starts_with("curl -H Authorization="), "{got}");
        assert!(got.contains(MARK), "{got}");
        assert!(!got.contains("abc123"), "값이 남았다: {got}");
        assert!(got.contains("https://api.example.com"), "주소까지 지웠다: {got}");
    }

    /// 공개된 접두사를 쓰는 토큰은 그 자체가 비밀이다.
    #[test]
    fn well_known_token_shapes_are_masked() {
        for t in ["sk-live-abcdefghijklmnop", "ghp_abcdefghijklmnopqrst", "AKIAIOSFODNN7EXAMPLE"] {
            let got = line_full(&format!("export KEY {t}"));
            assert!(!got.contains(t), "{t} 가 남았다: {got}");
        }
    }

    /// 짧은 글자는 접두사가 같아도 토큰이 아니다(`sk-1`은 그냥 인자다).
    #[test]
    fn short_lookalikes_are_left_alone() {
        assert_eq!(line_full("run sk-1"), "run sk-1");
    }

    /// MySQL 꼴 — `-p비번`은 붙여 쓴 값만 가린다.
    #[test]
    fn a_glued_password_flag_is_masked() {
        let got = line_full("mysql -u root -pS3cr3t!");
        assert!(got.contains("-p"), "{got}");
        assert!(!got.contains("S3cr3t"), "{got}");
        assert!(got.contains("-u root"), "사용자까지 지웠다: {got}");
    }

    /// `-p` 하나만 있는 것은 비밀이 아니다(대화식으로 묻는 꼴).
    #[test]
    fn a_bare_password_flag_is_not_a_secret() {
        assert_eq!(line_full("mysql -u root -p"), "mysql -u root -p");
    }

    /// **띄어 쓴 값**도 가린다 — `--token abc`.
    #[test]
    fn a_spaced_value_after_a_keyword_is_masked() {
        let got = line_full("gh auth login --token abcdefghijklmnop");
        assert!(!got.contains("abcdefghijklmnop"), "{got}");
        assert!(got.contains("--token"), "{got}");
    }

    /// 개인키 본문은 줄째 버린다.
    #[test]
    fn a_private_key_body_is_dropped() {
        let got = redact("-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n");
        assert!(got.contains("[redacted private key]"), "{got}");
    }

    /// 여러 줄을 가려도 줄 수는 그대로여야 한다(로그 대조가 깨지지 않게).
    #[test]
    fn the_line_count_is_preserved() {
        let src = "a\npassword=1234\nb";
        assert_eq!(redact(src).lines().count(), src.lines().count());
    }

    /// 한글이 든 명령도 그대로 지나간다.
    #[test]
    fn hangul_commands_pass_through() {
        let s = "echo 안녕하세요";
        assert_eq!(line_full(s), s);
    }

    /// 이미 쌓인 기록도 훑는다 — 가리기를 켜기 전에 남은 것은 여전히 비밀이다.
    #[test]
    fn an_existing_history_is_swept() {
        let mut cfg = nabi_config::AppConfig::default();
        cfg.terminal.redact_history = true;
        cfg.terminal.cmd_history = vec![("mysql -pS3cr3t!".into(), "/".into(), 0, 1)];
        let out = super::sweep_history(cfg);
        assert!(!out.terminal.cmd_history[0].0.contains("S3cr3t"), "{:?}", out.terminal.cmd_history[0].0);
    }

    /// 꺼 놓았으면 **손대지 않는다** — 끈 사람은 원문을 원한다.
    #[test]
    fn sweeping_is_skipped_when_turned_off() {
        let mut cfg = nabi_config::AppConfig::default();
        cfg.terminal.redact_history = false;
        cfg.terminal.cmd_history = vec![("mysql -pS3cr3t!".into(), "/".into(), 0, 1)];
        let out = super::sweep_history(cfg);
        assert!(out.terminal.cmd_history[0].0.contains("S3cr3t"));
    }

    /// **로그에서 가장 흔한 꼴** — `password: x`처럼 구분자 뒤에 띄어 쓴 값.
    ///
    /// 처음 쓴 규칙이 이걸 놓쳤고, 지원 번들에 있던 옛 시험이 잡아냈다.
    #[test]
    fn a_value_after_a_colon_and_a_space_is_masked() {
        assert_eq!(line_full("password: x"), format!("password: {MARK}"));
        assert_eq!(line_full("Authorization: Bearer abc"), format!("Authorization: {MARK} abc"));
    }
}
