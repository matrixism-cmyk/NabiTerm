//! Xshell 세션(.xsh) 가져오기(T7-5 한국 1급) — Xshell 이탈 사용자의 세션을 흡수한다.
//!
//! `.xsh`는 INI 형식: `[CONNECTION]` Host/Port/Protocol, `[CONNECTION:AUTHENTICATION]`
//! UserName. 기본 위치는 `문서\NetSarang Computer\<버전>\Xshell\Sessions`(하위 폴더 포함,
//! 폴더 구조는 세션 그룹으로 보존). 비밀번호는 Xshell 자체 암호화라 가져오지 않는다
//! (연결 시 볼트/프롬프트 경로 사용 — 평문 금지 원칙과도 일치).

use nabi_session::{SavedSession, SessionKind};
use std::path::{Path, PathBuf};

/// `.xsh` INI 텍스트 → 저장 세션(SSH만). `folder`는 세션 그룹(하위 폴더명 등).
pub(crate) fn parse_xsh(name: &str, folder: Option<String>, text: &str) -> Option<SavedSession> {
    let (mut host, mut port, mut user, mut proto) = (String::new(), 22u16, String::new(), String::from("ssh"));
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.to_ascii_uppercase();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_uppercase(), v.trim());
        match (section.as_str(), k.as_str()) {
            ("CONNECTION", "HOST") => host = v.to_string(),
            ("CONNECTION", "PORT") => port = v.parse().unwrap_or(22),
            ("CONNECTION", "PROTOCOL") => proto = v.to_ascii_lowercase(),
            ("CONNECTION:AUTHENTICATION", "USERNAME") => user = v.to_string(),
            _ => {}
        }
    }
    if host.is_empty() || !proto.contains("ssh") {
        return None; // telnet/serial/rlogin 등 제외.
    }
    Some(SavedSession {
        name: name.to_string(),
        folder,
        kind: SessionKind::Ssh { host, port, user, credential_ref: None, key_path: None, jump: None },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    })
}

/// 폴더를 재귀로 훑어 `.xsh` 세션을 모은다. 최상위 폴더명 대신 하위 폴더 경로를 그룹으로.
pub(crate) fn scan_dir(root: &Path) -> Vec<SavedSession> {
    let mut out = Vec::new();
    walk(root, root, &mut out, 0);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<SavedSession>, depth: usize) {
    if depth > 6 {
        return; // 순환/비정상 깊이 방어.
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out, depth + 1);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("xsh")) {
            let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            // 그룹 = Sessions 기준 상대 하위 폴더("xshell/서브폴더"), 루트 직속은 "xshell".
            let rel = p.parent().and_then(|d| d.strip_prefix(root).ok()).map(|r| r.to_string_lossy().replace('\\', "/"));
            let folder = match rel.as_deref() {
                Some("") | None => Some("xshell".to_string()),
                Some(r) => Some(format!("xshell/{r}")),
            };
            // .xsh는 UTF-16LE(BOM)인 경우가 많다 — 자동 인코딩 감지로 읽는다.
            if let Ok(bytes) = std::fs::read(&p) {
                let text = nabi_editor::editload::decode(&bytes).0;
                if let Some(s) = parse_xsh(&name, folder, &text) {
                    out.push(s);
                }
            }
        }
    }
}

/// Xshell 기본 세션 폴더 자동 탐색(버전 폴더 여러 개면 가장 최근 수정).
pub(crate) fn default_sessions_dir() -> Option<PathBuf> {
    let docs = std::env::var_os("USERPROFILE").map(PathBuf::from)?.join("Documents").join("NetSarang Computer");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for ver in std::fs::read_dir(&docs).ok()?.flatten() {
        let cand = ver.path().join("Xshell").join("Sessions");
        if cand.is_dir() {
            let t = cand.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().map(|(bt, _)| t > *bt).unwrap_or(true) {
                best = Some((t, cand));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_session() {
        let ini = "[CONNECTION]\r\nHost=dev.example.co.kr\r\nPort=2022\r\nProtocol=SSH\r\n\
                   [CONNECTION:AUTHENTICATION]\r\nUserName=kim\r\nPassword=ENCRYPTED\r\n";
        let s = parse_xsh("개발서버", Some("xshell".into()), ini).expect("ssh 세션");
        assert_eq!(s.name, "개발서버");
        match &s.kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("dev.example.co.kr", 2022, "kim"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn rejects_non_ssh() {
        let ini = "[CONNECTION]\nHost=x\nProtocol=TELNET\n";
        assert!(parse_xsh("t", None, ini).is_none(), "telnet 제외");
        assert!(parse_xsh("h", None, "[CONNECTION]\nProtocol=SSH\n").is_none(), "호스트 없음 제외");
    }

    #[test]
    fn scan_preserves_subfolder_groups() {
        let dir = std::env::temp_dir().join(format!("nabi-xsh-{}", std::process::id()));
        let sub = dir.join("운영");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.xsh"), "[CONNECTION]\nHost=a\nProtocol=SSH\n").unwrap();
        std::fs::write(sub.join("b.xsh"), "[CONNECTION]\nHost=b\nProtocol=SSH\n").unwrap();
        let v = scan_dir(&dir);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].folder.as_deref(), Some("xshell"));
        assert_eq!(v[1].folder.as_deref(), Some("xshell/운영"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
