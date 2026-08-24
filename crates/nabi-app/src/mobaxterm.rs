//! MobaXterm `MobaXterm.ini`(세션 북마크) → 저장 세션. SSH(type 109)만 가져온다.
//! 세션 값 형식: ` #109#<icon>%<host>%<port>%<user>%…`. `[Bookmarks*]` 섹션의 SubRep=폴더명.

use nabi_session::{SavedSession, SessionKind};

/// MobaXterm.ini에서 SSH 북마크를 추출한다(다른 세션 유형·메타 키는 건너뜀).
pub(crate) fn parse_mobaxterm(ini: &str) -> Vec<SavedSession> {
    let mut out = Vec::new();
    let mut folder: Option<String> = None;
    let mut in_bookmarks = false;
    for raw in ini.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_bookmarks = line[1..line.len() - 1].starts_with("Bookmarks");
            folder = None;
            continue;
        }
        if !in_bookmarks {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else { continue };
        let (key, val) = (key.trim(), val.trim());
        if key.eq_ignore_ascii_case("SubRep") {
            folder = (!val.is_empty()).then(|| val.replace('\\', "/"));
        } else if !key.eq_ignore_ascii_case("ImgNum") {
            if let Some(s) = parse_session(key, val, folder.as_deref()) {
                out.push(s);
            }
        }
    }
    out
}

/// 설치된 MobaXterm.ini 기본 경로(존재 시). 흔한 위치를 순서대로 탐색. 자동 가져오기용.
pub(crate) fn default_path() -> Option<std::path::PathBuf> {
    let home = crate::browser::home_dir();
    let mut cands = vec![
        home.join("Documents").join("MobaXterm").join("MobaXterm.ini"),
        home.join("Documents").join("MobaXterm.ini"),
    ];
    if let Some(ad) = std::env::var_os("APPDATA") {
        cands.push(std::path::Path::new(&ad).join("MobaXterm").join("MobaXterm.ini"));
    }
    cands.into_iter().find(|p| p.is_file())
}

/// SSH 세션을 MobaXterm.ini 문자열로 내보낸다(폴더별 [Bookmarks*] 섹션, type 109).
pub(crate) fn to_ini(sessions: &[SavedSession]) -> String {
    // host/port/user 뒤 공통 꼬리 필드(MobaXterm 기본값) — 실제 임포트되게 채운다.
    const TAIL: &str = "%-1%-1%%%22%%0%0%0%%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%0%0%0%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%0%-1";
    let mut groups: Vec<(String, Vec<&SavedSession>)> = Vec::new();
    for s in sessions {
        if s.is_ftp {
            continue;
        }
        let SessionKind::Ssh { .. } = &s.kind else { continue };
        let f = s.folder.clone().unwrap_or_default();
        match groups.iter_mut().find(|(g, _)| *g == f) {
            Some((_, v)) => v.push(s),
            None => groups.push((f, vec![s])),
        }
    }
    let mut out = String::new();
    for (i, (folder, list)) in groups.iter().enumerate() {
        out.push_str(&if i == 0 { "[Bookmarks]\n".into() } else { format!("[Bookmarks_{i}]\n") });
        out.push_str(&format!("SubRep={}\nImgNum=41\n", folder.replace('/', "\\")));
        for s in list {
            if let SessionKind::Ssh { host, port, user, .. } = &s.kind {
                out.push_str(&format!("{}= #109#0%{host}%{port}%{user}{TAIL}\n", s.name));
            }
        }
        out.push('\n');
    }
    out
}

/// 한 북마크 줄을 SSH 세션으로(타입 109만). 호스트가 없거나 다른 유형이면 None.
fn parse_session(name: &str, val: &str, folder: Option<&str>) -> Option<SavedSession> {
    let (typ, rest) = val.trim_start_matches('#').split_once('#')?;
    if typ != "109" {
        return None; // SSH 세션만.
    }
    let fields: Vec<&str> = rest.split('%').collect();
    let host = fields.get(1).map(|s| s.trim()).unwrap_or("");
    if host.is_empty() {
        return None;
    }
    let port = fields.get(2).and_then(|p| p.trim().parse().ok()).unwrap_or(22);
    let user = fields.get(3).map(|s| s.trim()).unwrap_or("").to_string();
    Some(SavedSession {
        name: name.to_string(),
        folder: folder.map(str::to_string).or_else(|| Some("mobaxterm".into())),
        kind: SessionKind::Ssh { host: host.to_string(), port, user, credential_ref: None, key_path: None, jump: None },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_mobaxterm;
    use nabi_session::SessionKind;

    #[test]
    fn parses_ssh_bookmarks_with_folder() {
        let ini = "[Bookmarks]\nSubRep=\nImgNum=42\n\
                   Web= #109#0%a.com%22%alice%%-1%-1%%%22%%0\n\
                   [Bookmarks_1]\nSubRep=Prod\nImgNum=41\n\
                   DB= #109#0%db.local%2222%root%%-1\n\
                   Win= #91#4%10.0.0.9%3389%admin%\n";
        let v = parse_mobaxterm(ini);
        assert_eq!(v.len(), 2); // RDP(91)은 제외.
        assert_eq!(v[0].name, "Web");
        assert_eq!(v[0].folder.as_deref(), Some("mobaxterm")); // SubRep 비면 기본.
        match &v[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("a.com", 22, "alice"));
            }
            _ => panic!(),
        }
        assert_eq!(v[1].name, "DB");
        assert_eq!(v[1].folder.as_deref(), Some("Prod"));
        match &v[1].kind {
            SessionKind::Ssh { host, port, .. } => assert_eq!((host.as_str(), *port), ("db.local", 2222)),
            _ => panic!(),
        }
    }

    #[test]
    fn export_roundtrips() {
        use nabi_session::SavedSession;
        let sess = SavedSession {
            name: "DB".into(),
            folder: Some("Prod".into()),
            kind: SessionKind::Ssh { host: "db.local".into(), port: 2222, user: "root".into(), credential_ref: None, key_path: None, jump: None },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        };
        let ini = super::to_ini(std::slice::from_ref(&sess));
        let back = parse_mobaxterm(&ini);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "DB");
        assert_eq!(back[0].folder.as_deref(), Some("Prod"));
        match &back[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("db.local", 2222, "root"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn ignores_outside_bookmarks() {
        let ini = "[Misc]\nFoo= #109#0%nope.com%22%x%\n";
        assert!(parse_mobaxterm(ini).is_empty());
    }
}
