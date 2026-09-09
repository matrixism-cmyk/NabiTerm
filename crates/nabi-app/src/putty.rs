//! PuTTY 세션 ↔ 저장 세션. PuTTY는 세션을 레지스트리에 두므로 `.reg`(레지스트리 편집기 5.00)
//! 텍스트를 파싱/생성한다. 파서·생성기는 순수 함수(테스트 가능), 레지스트리 입출력은 menuact 글루.

use nabi_session::{SavedSession, SessionKind};

const KEY: &str = r"\PuTTY\Sessions\";

/// `.reg` 텍스트에서 PuTTY 세션을 추출한다. 폴더는 "putty"로 묶는다.
///
/// **SSH 와 직렬(serial)을 가져온다.** 예전에는 SSH 만 가져왔는데, 그때는 우리에게
/// 직렬 콘솔이 없어서 가져와도 열 수가 없었기 때문이다(2026-09-09에 생겼다).
/// PuTTY 를 쓰던 사람의 장비 콘솔 목록은 대개 SSH 목록만큼 길고, 그것을 손으로 다시
/// 만들게 하면 옮겨 올 이유가 사라진다.
///
/// 텔넷은 아직 가져오지 않는다 — 열 수 없는 세션을 목록에 채워 두면 눌렀을 때 아무 일도
/// 일어나지 않는다. 텔넷을 지원하는 날 함께 켠다.
pub(crate) fn parse_putty_reg(text: &str) -> Vec<SavedSession> {
    let mut out = Vec::new();
    let mut cur: Option<Cur> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut out, cur.take());
            if let Some(idx) = line.find(KEY) {
                let raw = line[idx + KEY.len()..].trim_end_matches(']');
                cur = Some(Cur { name: reg_decode(raw), ..Cur::default() });
            }
        } else if let Some(c) = cur.as_mut() {
            if let Some(v) = str_val(line, "HostName") {
                c.host = v;
            } else if let Some(v) = str_val(line, "UserName") {
                c.user = v;
            } else if let Some(v) = str_val(line, "Protocol") {
                c.proto = v.to_ascii_lowercase();
            } else if let Some(v) = str_val(line, "SerialLine") {
                c.line = v;
            } else if let Some(p) = dword_val(line, "PortNumber") {
                c.port = p;
            } else if let Some(v) = dword32(line, "SerialSpeed") {
                c.speed = v;
            } else if let Some(v) = dword32(line, "SerialDataBits") {
                c.data_bits = v as u8;
            } else if let Some(v) = dword32(line, "SerialParity") {
                c.parity = v as u8;
            } else if let Some(v) = dword32(line, "SerialStopHalfbits") {
                c.stop_halfbits = v as u8;
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
        // 직렬은 PuTTY 도 저장하는 종류다 — 가져올 수 있으면 돌려줄 수도 있어야 한다.
        if let SessionKind::Serial { port, baud, frame } = &sess.kind {
            if let Some(block) = serial_block(&sess.name, port, *baud, frame) {
                s.push_str(&block);
            }
            continue;
        }
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
    /// PuTTY 의 `Protocol` 값. 없으면 SSH 로 본다(옛 세션은 이 값을 안 적기도 한다).
    proto: String,
    /// 직렬: `SerialLine`(COM3)·`SerialSpeed`·데이터 비트·패리티·정지 비트.
    line: String,
    speed: u32,
    data_bits: u8,
    /// PuTTY 의 패리티 번호: 0=없음 1=홀수 2=짝수 3=마크 4=스페이스.
    parity: u8,
    /// PuTTY 는 **반 비트 수**로 적는다: 2=1비트, 4=2비트.
    stop_halfbits: u8,
}

impl Default for Cur {
    fn default() -> Self {
        // PuTTY 의 기본값과 같게 둔다 — 세션에 안 적힌 값은 PuTTY 도 이것을 쓴다.
        Self {
            name: String::new(), host: String::new(), port: 22, user: String::new(),
            proto: String::new(), line: String::new(), speed: 9600,
            data_bits: 8, parity: 0, stop_halfbits: 2,
        }
    }
}

/// PuTTY 의 패리티 번호를 우리 표기(N·E·O)로.
///
/// 마크·스페이스(3·4)는 우리가 지원하지 않는다. 조용히 N 으로 바꾸면 **글자가 깨지는데
/// 이유를 알 수 없으므로**, 그런 세션은 아예 가져오지 않는다(아래 `flush`).
fn parity_char(n: u8) -> Option<char> {
    match n {
        0 => Some('N'),
        1 => Some('O'),
        2 => Some('E'),
        _ => None,
    }
}

fn flush(out: &mut Vec<SavedSession>, cur: Option<Cur>) {
    let Some(c) = cur else { return };
    if c.name.is_empty() {
        return;
    }
    let Some(kind) = kind_of(&c) else { return };
    out.push(SavedSession {
        name: c.name,
        folder: Some("putty".into()),
        kind,
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    });
}

/// 이 세션을 우리 종류로 옮긴다. 열 수 없는 것이면 None(목록에 채우지 않는다).
fn kind_of(c: &Cur) -> Option<SessionKind> {
    if c.proto == "serial" {
        // 포트 이름이 없으면 열 수가 없다.
        let parity = parity_char(c.parity)?;
        if c.line.is_empty() || !(5..=8).contains(&c.data_bits) {
            return None;
        }
        // PuTTY 는 반 비트로 적는다: 2=1비트, 4=2비트. 1.5비트(3)는 우리가 못 낸다.
        let stop = match c.stop_halfbits {
            2 => 1u8,
            4 => 2,
            _ => return None,
        };
        return Some(SessionKind::Serial {
            port: c.line.clone(),
            baud: c.speed,
            frame: format!("{}{parity}{stop}", c.data_bits),
        });
    }
    // 프로토콜을 안 적은 옛 세션은 SSH 로 본다(PuTTY 도 그렇게 읽는다).
    let ssh = c.proto.is_empty() || c.proto == "ssh";
    if !ssh || c.host.is_empty() {
        return None;
    }
    Some(SessionKind::Ssh {
        host: c.host.clone(),
        port: c.port,
        user: c.user.clone(),
        credential_ref: None,
        key_path: None,
        jump: None,
        agent_forward: false,
    })
}

/// `"Key"="value"` 한 줄에서 value를 뽑는다(키 불일치면 None).
fn str_val(line: &str, key: &str) -> Option<String> {
    let pfx = format!("\"{key}\"=\"");
    line.strip_prefix(&pfx)?.strip_suffix('"').map(str::to_string)
}

/// 직렬 세션 하나를 PuTTY `.reg` 블록으로. 표기를 못 읽으면 None(엉뚱한 값을 쓰지 않는다).
fn serial_block(name: &str, port: &str, baud: u32, frame: &str) -> Option<String> {
    let (data_bits, parity, stop) = nabi_serial::SerialCfg::parse_frame(frame)?;
    let par = match parity {
        'O' => 1u32,
        'E' => 2,
        _ => 0,
    };
    let halfbits = u32::from(stop) * 2; // 우리 1·2 → PuTTY 2·4.
    Some(format!(
        "[HKEY_CURRENT_USER\\Software\\SimonTatham{KEY}{}]\n\
         \"Protocol\"=\"serial\"\n\"SerialLine\"=\"{port}\"\n\
         \"SerialSpeed\"=dword:{baud:08x}\n\"SerialDataBits\"=dword:{:08x}\n\
         \"SerialParity\"=dword:{par:08x}\n\"SerialStopHalfbits\"=dword:{halfbits:08x}\n\n",
        reg_encode(name),
        u32::from(data_bits),
    ))
}

/// `"Key"=dword:0000XXXX` 에서 32비트 값을 뽑는다(속도는 921600 처럼 16비트를 넘는다).
fn dword32(line: &str, key: &str) -> Option<u32> {
    let pfx = format!("\"{key}\"=dword:");
    u32::from_str_radix(line.strip_prefix(&pfx)?, 16).ok()
}

/// `"Key"=dword:0000XXXX` 한 줄에서 16진 값을 뽑는다.
fn dword_val(line: &str, key: &str) -> Option<u16> {
    let pfx = format!("\"{key}\"=dword:");
    u32::from_str_radix(line.strip_prefix(&pfx)?, 16).ok().map(|v| v as u16)
}

/// 레지스트리 키 이름의 `%XX` 이스케이프를 디코드한다.
/// **바이트로 모았다가 마지막에 UTF-8 로 읽는다.**
///
/// 예전에는 바이트마다 `as char` 로 밀어 넣었다. 그것은 Latin-1 해석이라, 한 글자가 여러
/// 바이트인 이름(한글·일본어·이모지)이 통째로 깨졌다 — `장비` 가 `ì¥ë¹` 가 되는 식이다.
/// ASCII 이름만 시험하고 있어서 **아무도 몰랐다**(2026-09-09에 직렬 왕복 시험을 한글
/// 이름으로 쓰다가 드러났다). SSH 세션을 가져올 때도 같은 결함이었다.
fn reg_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    // 깨진 바이트가 섞여 있어도 나머지는 살린다 — 이름 하나 때문에 목록 전체를 잃지 않는다.
    String::from_utf8_lossy(&out).into_owned()
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
        // 포트 이름(`SerialLine`)이 없는 직렬 세션은 **열 수가 없으므로** 가져오지 않는다.
        // 예전에는 "직렬은 무조건 제외"였다 — 그때는 우리에게 직렬 콘솔이 없었다.
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "My Server"); // %20 디코드.
        match &v[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("example.com", 22, "bob")); // dword 0x16=22.
            }
            _ => panic!(),
        }
    }

    /// PuTTY 의 직렬 세션을 가져온다 — 그쪽 표기를 우리 표기로 옮긴다.
    #[test]
    fn 직렬_세션을_가져온다() {
        let reg = "Windows Registry Editor Version 5.00\n\n\
            [HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Switch]\n\
            \"Protocol\"=\"serial\"\n\"SerialLine\"=\"COM3\"\n\
            \"SerialSpeed\"=dword:0001c200\n\"SerialDataBits\"=dword:00000007\n\
            \"SerialParity\"=dword:00000002\n\"SerialStopHalfbits\"=dword:00000004\n";
        let v = parse_putty_reg(reg);
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].kind,
            // 0x1c200 = 115200. 패리티 2=짝수(E), 반비트 4=정지 2비트.
            SessionKind::Serial { port: "COM3".into(), baud: 115200, frame: "7E2".into() }
        );
    }

    /// 값을 안 적은 직렬 세션은 **PuTTY 의 기본값**으로 본다(PuTTY 도 그렇게 읽는다).
    #[test]
    fn 안_적은_값은_기본값이다() {
        let reg = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\C]\n\
            \"Protocol\"=\"serial\"\n\"SerialLine\"=\"COM1\"\n";
        let v = parse_putty_reg(reg);
        assert_eq!(
            v[0].kind,
            SessionKind::Serial { port: "COM1".into(), baud: 9600, frame: "8N1".into() }
        );
    }

    /// **못 여는 것은 가져오지 않는다.** 목록에 채워 두면 눌렀을 때 아무 일도 없다.
    #[test]
    fn 못_여는_직렬은_거른다() {
        let head = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\X]\n\
            \"Protocol\"=\"serial\"\n\"SerialLine\"=\"COM9\"\n";
        // 마크·스페이스 패리티(3·4)와 1.5 정지비트(3)는 우리가 못 낸다.
        for bad in [
            "\"SerialParity\"=dword:00000003\n",
            "\"SerialParity\"=dword:00000004\n",
            "\"SerialStopHalfbits\"=dword:00000003\n",
            "\"SerialDataBits\"=dword:00000004\n",
        ] {
            assert!(parse_putty_reg(&format!("{head}{bad}")).is_empty(), "{bad}");
        }
        // 포트 이름이 없으면 열 수가 없다.
        let noline = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Y]\n\
            \"Protocol\"=\"serial\"\n";
        assert!(parse_putty_reg(noline).is_empty());
    }

    /// 프로토콜을 안 적은 옛 세션은 SSH 로 본다(PuTTY 와 같은 해석).
    #[test]
    fn 프로토콜이_없으면_ssh() {
        let reg = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Old]\n\
            \"HostName\"=\"h.example\"\n";
        let v = parse_putty_reg(reg);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0].kind, SessionKind::Ssh { .. }));
    }

    /// 가져올 수 있으면 돌려줄 수도 있어야 한다 — 직렬도 왕복한다.
    #[test]
    fn 직렬도_왕복한다() {
        for (baud, frame) in [(9600u32, "8N1"), (115200, "7E2"), (921600, "8O1")] {
            let sess = SavedSession {
                name: "장비 [운영]".into(),
                folder: None,
                kind: SessionKind::Serial { port: "COM12".into(), baud, frame: frame.into() },
                on_connect: None,
                cwd: None,
                is_ftp: false,
                open_sftp: false,
                tag: Default::default(),
            };
            let back = parse_putty_reg(&to_putty_reg(std::slice::from_ref(&sess)));
            assert_eq!(back.len(), 1, "{baud} {frame}");
            assert_eq!(back[0].name, "장비 [운영]");
            assert_eq!(back[0].kind, sess.kind, "{baud} {frame}");
        }
    }

    #[test]
    fn export_roundtrips() {
        let sess = SavedSession {
            name: "Web [prod]".into(),
            folder: None,
            kind: SessionKind::Ssh { host: "a.com".into(), port: 2222, user: "al".into(), credential_ref: None, key_path: None, jump: None, agent_forward: false },
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
