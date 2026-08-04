//! FileZilla `sitemanager.xml` → 저장 세션. SFTP(Protocol 1)=SSH 터미널, FTP/FTPS=FTP 브라우저.
//! XML 크레이트 없이 `<Server>…</Server>` 블록을 훑어 추출한다(폴더 계층은 "filezilla"로 묶음).

use nabi_session::{SavedSession, SessionKind};

/// sitemanager.xml에서 서버 항목(호스트/포트/사용자/프로토콜/이름)을 추출한다.
/// `<Folder>` 계층을 추적해 세션 폴더로 보존한다(최상위는 "filezilla").
pub(crate) fn parse_filezilla(xml: &str) -> Vec<SavedSession> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut rest = xml;
    while let Some((pos, tok)) = ["<Folder>", "</Folder>", "<Server>"]
        .iter()
        .filter_map(|t| rest.find(t).map(|p| (p, *t)))
        .min_by_key(|(p, _)| *p)
    {
        rest = &rest[pos + tok.len()..];
        match tok {
            "<Folder>" => {
                let end = rest.find('<').unwrap_or(0);
                let name = crate::editorconvert::html_decode(rest[..end].trim());
                stack.push(if name.is_empty() { "filezilla".into() } else { name });
            }
            "</Folder>" => {
                stack.pop();
            }
            _ => {
                let Some(e) = rest.find("</Server>") else { break };
                let block = &rest[..e];
                rest = &rest[e + 9..];
                let host = tag(block, "Host");
                if host.is_empty() {
                    continue;
                }
                let port = tag(block, "Port").parse::<u16>().unwrap_or(22);
                let user = tag(block, "User");
                let proto = tag(block, "Protocol").parse::<i32>().unwrap_or(0);
                let name = match tag(block, "Name") {
                    n if n.is_empty() => host.clone(),
                    n => n,
                };
                out.push(SavedSession {
                    name,
                    folder: Some(stack.last().cloned().unwrap_or_else(|| "filezilla".into())),
                    kind: SessionKind::Ssh { host, port, user, credential_ref: None, key_path: None, jump: None },
                    on_connect: None,
                    cwd: None,
                    is_ftp: proto != 1, // SFTP(1)=SSH 터미널, 그 외(FTP/FTPS)=FTP 브라우저.
                    open_sftp: false,
                });
            }
        }
    }
    out
}

/// 설치된 FileZilla의 sitemanager.xml 기본 경로(존재 시). 자동 가져오기에 사용.
pub(crate) fn default_path() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(&std::env::var_os("APPDATA")?)
        .join("FileZilla")
        .join("sitemanager.xml");
    p.is_file().then_some(p)
}

/// 세션 목록을 FileZilla sitemanager.xml 문자열로 내보낸다(SSH/SFTP 세션만).
pub(crate) fn to_sitemanager(sessions: &[SavedSession]) -> String {
    let esc = crate::editorconvert::html_encode;
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<FileZilla3>\n\t<Servers>\n");
    for sess in sessions {
        let SessionKind::Ssh { host, port, user, .. } = &sess.kind else { continue };
        let proto = if sess.is_ftp { 0 } else { 1 }; // 0=FTP, 1=SFTP.
        s.push_str(&format!(
            "\t\t<Server>\n\t\t\t<Host>{}</Host>\n\t\t\t<Port>{port}</Port>\n\t\t\t<Protocol>{proto}</Protocol>\n\t\t\t<Type>0</Type>\n\t\t\t<User>{}</User>\n\t\t\t<Logontype>1</Logontype>\n\t\t\t<Name>{}</Name>\n\t\t</Server>\n",
            esc(host), esc(user), esc(&sess.name)
        ));
    }
    s.push_str("\t</Servers>\n</FileZilla3>\n");
    s
}

/// `<Tag>내용</Tag>`의 첫 내용을 XML 엔티티 디코드해 돌려준다(없으면 빈 문자열).
fn tag(block: &str, name: &str) -> String {
    let open = format!("<{name}>");
    let Some(st) = block.find(&open).map(|i| i + open.len()) else {
        return String::new();
    };
    let Some(len) = block[st..].find(&format!("</{name}>")) else {
        return String::new();
    };
    crate::editorconvert::html_decode(block[st..st + len].trim())
}

#[cfg(test)]
mod tests {
    use super::parse_filezilla;
    use nabi_session::SessionKind;

    #[test]
    fn parses_servers_and_protocol() {
        let xml = "<FileZilla3><Servers>\
        <Server><Host>a.com</Host><Port>22</Port><Protocol>1</Protocol><User>alice</User><Name>Web</Name></Server>\
        <Folder>grp<Server><Host>ftp.b.com</Host><Port>21</Port><Protocol>0</Protocol><User>bob</User></Server></Folder>\
        </Servers></FileZilla3>";
        let v = parse_filezilla(xml);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "Web");
        assert!(!v[0].is_ftp); // SFTP=1 → SSH.
        match &v[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("a.com", 22, "alice"));
            }
            _ => panic!(),
        }
        assert_eq!(v[1].name, "ftp.b.com"); // Name 없으면 host.
        assert!(v[1].is_ftp); // FTP=0 → FTP 브라우저.
        assert_eq!(v[0].folder.as_deref(), Some("filezilla")); // 폴더 밖 → 기본.
        assert_eq!(v[1].folder.as_deref(), Some("grp")); // <Folder>grp 보존.
    }

    #[test]
    fn export_roundtrips() {
        use nabi_session::SavedSession;
        let sess = SavedSession {
            name: "My <Web>".into(),
            folder: None,
            kind: SessionKind::Ssh { host: "a.com".into(), port: 2222, user: "al&ice".into(), credential_ref: None, key_path: None, jump: None },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
        };
        let xml = super::to_sitemanager(std::slice::from_ref(&sess));
        let back = parse_filezilla(&xml);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "My <Web>"); // XML 이스케이프 왕복.
        match &back[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("a.com", 2222, "al&ice"));
            }
            _ => panic!(),
        }
        assert!(!back[0].is_ftp);
    }

    #[test]
    fn decodes_entities_and_skips_hostless() {
        let xml = "<Server><Host>x&amp;y.com</Host><Protocol>1</Protocol></Server><Server><Port>22</Port></Server>";
        let v = parse_filezilla(xml);
        assert_eq!(v.len(), 1); // 호스트 없는 두 번째는 제외.
        match &v[0].kind {
            SessionKind::Ssh { host, .. } => assert_eq!(host, "x&y.com"),
            _ => panic!(),
        }
    }
}
