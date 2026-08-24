//! PuTTY 세션 ↔ 저장 세션. PuTTY는 세션을 레지스트리에 두므로 `.reg`(레지스트리 편집기 5.00)
//! 텍스트를 파싱/생성한다. 파서·생성기는 순수 함수(테스트 가능), 레지스트리 입출력은 menuact 글루.

use nabi_session::{SavedSession, SessionKind};

const KEY: &str = r"\PuTTY\Sessions\";

/// `.reg` 텍스트에서 PuTTY 세션(HostName/PortNumber/UserName/Protocol)을 추출한다.
/// SSH 프로토콜만 가져온다(telnet/serial 등 제외). 폴더는 "putty"로 묶는다.
pub(crate) fn parse_putty_reg(text: &str) -> Vec<SavedSession> {
    let mut out = Vec::new();
    let mut cur: Option<Cur> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut out, cur.take());
            if let Some(idx) = line.find(KEY) {
                let raw = line[idx + KEY.len()..].trim_end_matches(']');
                cur = Some(Cur { name: reg_decode(raw), host: String::new(), port: 22, user: String::new(), ssh: true });
            }
        } else if let Some(c) = cur.as_mut() {
            if let Some(v) = str_val(line, "HostName") {
                c.host = v;
            } else if let Some(v) = str_val(line, "UserName") {
                c.user = v;
            } else if let Some(v) = str_val(line, "Protocol") {
                c.ssh = v.eq_ignore_ascii_case("ssh");
            } else if let Some(p) = dword_val(line, "PortNumber") {
                c.port = p;
            }
        }
    }
    flush(&mut out, cur.take());
    out
}

/// 세션 목록을 PuTTY `.reg` 텍스트로 내보낸다(SSH 터미널 세션만).
pub(crate) fn to_putty_reg(sessions: &[SavedSession]) -> String {
    let mut s = String::from("Windows Registry Editor Version 5.00\n\n");
    for sess in sessions {
        let SessionKind::Ssh { host, port, user, .. } = &sess.kind else { continue };
        if sess.is_ftp {
            continue;
        }
        s.push_str(&format!("[HKEY_CURRENT_USER\\Software\\SimonTatham{KEY}{}]\n", reg_encode(&sess.name)));
        s.push_str(&format!("\"HostName\"=\"{host}\"\n\"PortNumber\"=dword:{port:08x}\n"));
        if !user.is_empty() {
            s.push_str(&format!("\"UserName\"=\"{user}\"\n"));
        }
        s.push_str("\"Protocol\"=\"ssh\"\n\n");
    }
    s
}

/// PuTTY 세션 레지스트리를 임시 `.reg`로 export해 디코드한 텍스트를 돌려준다(없거나 실패 시 None).
/// 런타임 글루(reg.exe 의존) — 파싱은 [`parse_putty_reg`]가 담당한다.
pub(crate) fn export_registry_text() -> Option<String> {
    let tmp = std::env::temp_dir().join("nabi_putty_export.reg");
    let out = std::process::Command::new("reg")
        .args(["export", r"HKCU\Software\SimonTatham\PuTTY\Sessions"])
        .arg(&tmp)
        .arg("/y")
        .output()
        .ok()?;
    if !out.status.success() {
        return None; // PuTTY 미설치/세션 없음.
    }
    let bytes = std::fs::read(&tmp).ok()?;
    let _ = std::fs::remove_file(&tmp);
    Some(crate::editload::decode(&bytes).0) // .reg는 UTF-16LE — 인코딩 자동 감지.
}

struct Cur {
    name: String,
    host: String,
    port: u16,
    user: String,
    ssh: bool,
}

fn flush(out: &mut Vec<SavedSession>, cur: Option<Cur>) {
    let Some(c) = cur else { return };
    if c.host.is_empty() || !c.ssh || c.name.is_empty() {
        return;
    }
    out.push(SavedSession {
        name: c.name,
        folder: Some("putty".into()),
        kind: SessionKind::Ssh { host: c.host, port: c.port, user: c.user, credential_ref: None, key_path: None, jump: None },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    });
}

/// `"Key"="value"` 한 줄에서 value를 뽑는다(키 불일치면 None).
fn str_val(line: &str, key: &str) -> Option<String> {
    let pfx = format!("\"{key}\"=\"");
    line.strip_prefix(&pfx)?.strip_suffix('"').map(str::to_string)
}

/// `"Key"=dword:0000XXXX` 한 줄에서 16진 값을 뽑는다.
fn dword_val(line: &str, key: &str) -> Option<u16> {
    let pfx = format!("\"{key}\"=dword:");
    u32::from_str_radix(line.strip_prefix(&pfx)?, 16).ok().map(|v| v as u16)
}

/// 레지스트리 키 이름의 `%XX` 이스케이프를 디코드한다.
fn reg_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

/// 키 이름에서 위험 문자(`\` `%` `[` `]`·제어)를 `%XX`로 인코딩한다(나머지는 그대로).
fn reg_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c, '\\' | '%' | '[' | ']') {
                format!("%{:02X}", c as u32).chars().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_putty_sessions() {
        let reg = "Windows Registry Editor Version 5.00\n\n\
            [HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\My%20Server]\n\
            \"HostName\"=\"example.com\"\n\"PortNumber\"=dword:00000016\n\"UserName\"=\"bob\"\n\"Protocol\"=\"ssh\"\n\n\
            [HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Serial1]\n\"Protocol\"=\"serial\"\n";
        let v = parse_putty_reg(reg);
        assert_eq!(v.len(), 1); // serial 세션은 제외.
        assert_eq!(v[0].name, "My Server"); // %20 디코드.
        match &v[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("example.com", 22, "bob")); // dword 0x16=22.
            }
            _ => panic!(),
        }
    }

    #[test]
    fn export_roundtrips() {
        let sess = SavedSession {
            name: "Web [prod]".into(),
            folder: None,
            kind: SessionKind::Ssh { host: "a.com".into(), port: 2222, user: "al".into(), credential_ref: None, key_path: None, jump: None },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        };
        let back = parse_putty_reg(&to_putty_reg(std::slice::from_ref(&sess)));
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "Web [prod]"); // `[ ]` %XX 왕복.
        match &back[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("a.com", 2222, "al"));
            }
            _ => panic!(),
        }
    }
}
