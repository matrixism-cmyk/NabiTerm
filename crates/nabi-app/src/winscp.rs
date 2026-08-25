//! WinSCP 사이트 → 저장 세션. 한국에서 가장 널리 쓰이는 SFTP 클라이언트인데 임포터가 없었다.
//!
//! WinSCP는 설정을 **레지스트리 아니면 ini** 어느 한쪽에 둔다(설치 방식에 따라 다르다).
//! - 레지스트리: `HKCU\Software\Martin Prikryl\WinSCP 2\Sessions\<이름>`
//! - ini: 실행 파일 옆 `WinSCP.ini` 또는 `%APPDATA%\WinSCP.ini`, `[Sessions\<이름>]` 절
//!
//! 둘 다 **같은 필드 이름**을 쓰므로 파서는 하나면 된다 — `.reg` 텍스트든 ini든 결국
//! "구역 제목 + 키=값"이다. 다른 것은 값의 표기뿐이라(`dword:` 대 십진수) 거기만 갈라 본다.
//!
//! 사이트 이름은 URL 인코딩돼 있고 `/`가 폴더 구분자다 — `dev/web`은 `dev` 그룹의 `web`.
//! 이름 안에 진짜 `/`가 있으면 `%2F`로 인코딩돼 오므로 **가른 뒤에** 디코드해야 한다.
//!
//! 비밀번호는 가져오지 않는다. WinSCP의 저장 비밀번호는 가역 난독화라 옮겨 오는 순간
//! 우리 쪽에도 같은 약점이 생긴다 — 사용자가 볼트에 다시 넣게 하는 편이 낫다.

use nabi_session::{SavedSession, SessionKind};

/// FSProtocol 값(WinSCP `SessionData.h`). 0·1·2가 SSH 계열, 5가 FTP.
const FS_SCP: u32 = 0;
const FS_SFTP: u32 = 1;
const FS_SFTP_ONLY: u32 = 2;
const FS_FTP: u32 = 5;

/// 한 사이트를 읽는 중의 상태.
struct Cur {
    path: String,
    host: String,
    port: Option<u16>,
    user: String,
    key: Option<String>,
    proto: u32,
}

/// WinSCP 설정 텍스트(`.reg` export 또는 `WinSCP.ini`)에서 사이트를 뽑는다.
///
/// SSH 계열과 FTP만 가져온다. WebDAV·S3는 우리가 접속할 수 없으므로 조용히 건너뛴다 —
/// 열 수 없는 세션을 목록에 채워 넣으면 눌러 보고 나서야 알게 된다.
pub(crate) fn parse(text: &str) -> Vec<SavedSession> {
    let (mut out, mut cur) = (Vec::new(), None::<Cur>);
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            flush(&mut out, cur.take());
            cur = section_path(line).map(|path| Cur {
                path,
                host: String::new(),
                port: None,
                user: String::new(),
                key: None,
                proto: FS_SFTP,
            });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        let Some((k, v)) = split_kv(line) else { continue };
        match k.as_str() {
            "HostName" => c.host = v,
            "UserName" => c.user = v,
            "PublicKeyFile" => c.key = Some(v).filter(|s| !s.is_empty()),
            "PortNumber" => c.port = num(&v).and_then(|n| u16::try_from(n).ok()),
            "FSProtocol" => c.proto = num(&v).unwrap_or(FS_SFTP),
            _ => {}
        }
    }
    flush(&mut out, cur.take());
    out
}

/// 구역 제목에서 사이트 경로를 얻는다(`[...Sessions\\dev/web]` → `dev/web`).
///
/// `Default%20Settings`는 사이트가 아니라 기본값 틀이라 건너뛴다 — 가져오면 목록에
/// 접속할 수 없는 항목이 하나 끼어든다.
fn section_path(line: &str) -> Option<String> {
    let body = line.trim_start_matches('[').trim_end_matches(']');
    let idx = body.rfind("Sessions")?;
    let rest = body[idx + "Sessions".len()..].trim_start_matches(['\\', '/']);
    if rest.is_empty() || rest.eq_ignore_ascii_case("Default%20Settings") {
        return None;
    }
    Some(rest.to_string())
}

/// `"키"="값"`(reg) 또는 `키=값`(ini) 한 줄을 가른다.
fn split_kv(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once('=')?;
    let key = k.trim().trim_matches('"').to_string();
    let val = v.trim();
    // reg 문자열은 따옴표로 감싸여 있고 역슬래시가 이스케이프돼 있다.
    let val = if val.starts_with('"') {
        unescape(val.trim_matches('"'))
    } else {
        val.to_string()
    };
    (!key.is_empty()).then_some((key, val))
}

/// reg 문자열의 이스케이프를 되돌린다(`\` → `\`, `\"` → `"`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            if let Some(n) = it.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `dword:0000001a`(reg) 또는 `26`(ini)을 수로.
fn num(v: &str) -> Option<u32> {
    match v.strip_prefix("dword:") {
        Some(h) => u32::from_str_radix(h.trim(), 16).ok(),
        None => v.trim().parse().ok(),
    }
}

/// URL 인코딩을 되돌린다(`%20` → 공백). WinSCP가 사이트 이름에 쓰는 표기다.
///
/// **바이트로 모았다가 마지막에 한 번 문자열로 만든다.** 한글 한 글자는 UTF-8에서 세
/// 바이트(`%ED%85%8C`)라, 바이트마다 글자로 밀어 넣으면 이름이 깨진다.
fn url_decode(s: &str) -> String {
    let (b, mut out) = (s.as_bytes(), Vec::with_capacity(s.len()));
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(n) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 모은 사이트 하나를 저장 세션으로 옮긴다. 접속할 수 없는 종류면 버린다.
fn flush(out: &mut Vec<SavedSession>, cur: Option<Cur>) {
    let Some(c) = cur else { return };
    if c.host.is_empty() {
        return;
    }
    let is_ftp = c.proto == FS_FTP;
    if !is_ftp && !matches!(c.proto, FS_SCP | FS_SFTP | FS_SFTP_ONLY) {
        return; // WebDAV·S3 등 — 우리가 열 수 없다.
    }
    // `/`가 폴더 구분자다. 마지막 조각이 이름, 앞은 그룹.
    let decoded: Vec<String> = c.path.split('/').map(url_decode).collect();
    let (name, folder) = match decoded.split_last() {
        Some((last, [])) => (last.clone(), None),
        Some((last, head)) => (last.clone(), Some(head.join("/"))),
        None => return,
    };
    let port = c.port.unwrap_or(if is_ftp { 21 } else { 22 });
    out.push(SavedSession {
        name,
        folder,
        kind: SessionKind::Ssh {
            host: c.host,
            port,
            user: c.user,
            credential_ref: None, // 비밀번호는 옮기지 않는다(위 모듈 주석 참고).
            key_path: c.key,
            jump: None,
            agent_forward: false,
        },
        on_connect: None,
        cwd: None,
        is_ftp,
        open_sftp: false,
        tag: Default::default(),
    });
}

/// WinSCP 설정을 찾아 텍스트로 돌려준다 — ini가 있으면 ini, 없으면 레지스트리.
///
/// 런타임 글루(파일·reg.exe 의존). 파싱은 [`parse`]가 맡는다.
pub(crate) fn find_config() -> Option<String> {
    for p in ini_candidates() {
        if let Ok(t) = std::fs::read_to_string(&p) {
            if t.contains("Sessions") {
                return Some(t);
            }
        }
    }
    registry_text()
}

/// ini가 놓일 만한 자리들(WinSCP 문서 순서).
fn ini_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Some(app) = std::env::var_os("APPDATA") {
        v.push(std::path::PathBuf::from(app).join("WinSCP.ini"));
    }
    for env in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(env) {
            v.push(std::path::PathBuf::from(root).join("WinSCP").join("WinSCP.ini"));
        }
    }
    v
}

/// 레지스트리를 임시 `.reg`로 내보내 UTF-16을 디코드한다.
fn registry_text() -> Option<String> {
    let tmp = std::env::temp_dir().join("nabi_winscp_export.reg");
    let out = std::process::Command::new("reg")
        .args(["export", r"HKCU\Software\Martin Prikryl\WinSCP 2\Sessions"])
        .arg(&tmp)
        .arg("/y")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let bytes = std::fs::read(&tmp).ok()?;
    let _ = std::fs::remove_file(&tmp);
    Some(nabi_editor::editload::decode(&bytes).0) // .reg는 UTF-16LE — 인코딩 자동 감지.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 레지스트리 export 텍스트(UTF-16을 디코드한 뒤 모습).
    const REG: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\Martin Prikryl\WinSCP 2\Sessions\Default%20Settings]
"HostName"="ignored.example.com"

[HKEY_CURRENT_USER\Software\Martin Prikryl\WinSCP 2\Sessions\web01]
"HostName"="web01.example.com"
"UserName"="deploy"
"PortNumber"=dword:0000082a
"FSProtocol"=dword:00000001

[HKEY_CURRENT_USER\Software\Martin Prikryl\WinSCP 2\Sessions\dev/%ED%85%8C%EC%8A%A4%ED%8A%B8]
"HostName"="dev.example.com"
"UserName"="me"
"FSProtocol"=dword:00000005

[HKEY_CURRENT_USER\Software\Martin Prikryl\WinSCP 2\Sessions\cloud]
"HostName"="bucket.example.com"
"FSProtocol"=dword:00000006
"#;

    #[test]
    fn it_reads_sites_from_a_registry_export() {
        let got = parse(REG);
        assert_eq!(got.len(), 2, "기본값 틀과 WebDAV는 빠져야 한다: {got:?}");
        assert_eq!(got[0].name, "web01");
        assert!(!got[0].is_ftp);
        match &got[0].kind {
            nabi_session::SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("web01.example.com", 2090, "deploy"));
            }
            k => panic!("SSH 이어야 한다: {k:?}"),
        }
    }

    /// 사이트 이름의 `/`는 그룹 구분자다 — WinSCP 폴더가 우리 그룹이 되어야 한다.
    #[test]
    fn a_slash_in_the_name_becomes_a_group() {
        let got = parse(REG);
        let ftp = got.iter().find(|s| s.is_ftp).expect("FTP 사이트");
        assert_eq!(ftp.folder.as_deref(), Some("dev"));
        assert_eq!(ftp.name, "테스트", "URL 인코딩된 한글 이름이 풀려야 한다");
    }

    /// 포트를 안 적어 뒀으면 프로토콜에 맞는 기본값이어야 한다.
    #[test]
    fn a_missing_port_falls_back_per_protocol() {
        let got = parse(REG);
        let ftp = got.iter().find(|s| s.is_ftp).unwrap();
        match &ftp.kind {
            nabi_session::SessionKind::Ssh { port, .. } => assert_eq!(*port, 21),
            k => panic!("{k:?}"),
        }
    }

    /// ini 형식도 같은 파서로 읽힌다 — 값 표기만 다르다.
    #[test]
    fn it_reads_the_same_fields_from_an_ini_file() {
        let ini = "[Configuration\\Interface]\nkey=1\n\n[Sessions\\prod]\nHostName=prod.example.com\nUserName=root\nPortNumber=2222\nFSProtocol=2\n";
        let got = parse(ini);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "prod");
        match &got[0].kind {
            nabi_session::SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("prod.example.com", 2222, "root"));
            }
            k => panic!("{k:?}"),
        }
    }

    /// 개인키 경로는 가져오고, 비밀번호는 **가져오지 않는다**.
    #[test]
    fn it_takes_the_key_file_but_never_the_password() {
        let ini = "[Sessions\\k]\nHostName=h\nPublicKeyFile=C:\\keys\\id.ppk\nPassword=A35C2F9E\n";
        let got = parse(ini);
        match &got[0].kind {
            nabi_session::SessionKind::Ssh { key_path, credential_ref, .. } => {
                assert_eq!(key_path.as_deref(), Some("C:\\keys\\id.ppk"));
                assert!(credential_ref.is_none(), "비밀번호를 옮겨 오면 안 된다");
            }
            k => panic!("{k:?}"),
        }
    }

    /// 호스트가 없는 항목은 접속할 수 없으니 버린다.
    #[test]
    fn an_entry_without_a_host_is_dropped() {
        assert!(parse("[Sessions\\empty]\nUserName=x\n").is_empty());
    }

    /// 이름 안의 진짜 `/`는 그룹이 아니다 — `%2F`로 와서 이름 그대로 남아야 한다.
    #[test]
    fn an_encoded_slash_stays_inside_the_name() {
        let got = parse("[Sessions/a%2Fb]
HostName=h
");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "a/b");
        assert!(got[0].folder.is_none());
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(parse("").is_empty());
        assert!(parse("just some text\nwith no sections").is_empty());
    }
}
