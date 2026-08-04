//! 세션 내보내기/가져오기(버전드, 비밀 없음).

use crate::model::SessionTree;
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

/// 내보내기 문서(스키마 버전 포함).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionExport {
    pub schema_version: u32,
    pub sessions: SessionTree,
}

impl SessionExport {
    pub fn new(tree: SessionTree) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            sessions: tree,
        }
    }
}

pub fn to_json(tree: &SessionTree) -> Result<String, String> {
    serde_json::to_string_pretty(&SessionExport::new(tree.clone())).map_err(|e| e.to_string())
}

pub fn from_json(s: &str) -> Result<SessionTree, String> {
    let doc: SessionExport = serde_json::from_str(s).map_err(|e| e.to_string())?;
    migrate(doc)
}

pub fn to_toml(tree: &SessionTree) -> Result<String, String> {
    toml::to_string_pretty(&SessionExport::new(tree.clone())).map_err(|e| e.to_string())
}

pub fn from_toml(s: &str) -> Result<SessionTree, String> {
    let doc: SessionExport = toml::from_str(s).map_err(|e| e.to_string())?;
    migrate(doc)
}

/// 스키마 버전에 따라 현재 형태로 변환한다(미지 버전은 오류).
fn migrate(doc: SessionExport) -> Result<SessionTree, String> {
    match doc.schema_version {
        1 => Ok(doc.sessions),
        v => Err(format!("지원하지 않는 세션 스키마 버전: {v}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SavedSession, SessionKind};

    #[test]
    fn json_round_trip_excludes_secrets() {
        let mut tree = SessionTree::default();
        tree.add(SavedSession {
            name: "prod".into(),
            folder: Some("work".into()),
            kind: SessionKind::Ssh {
                host: "example.com".into(),
                port: 22,
                user: "deploy".into(),
                credential_ref: Some("vault:1".into()),
                key_path: None,
                jump: None,
            },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
        });
        let json = to_json(&tree).unwrap();
        // 비밀 평문 키가 직렬화되지 않아야 한다(vault 참조만 허용).
        assert!(!json.contains("password"));
        assert!(!json.contains("passphrase"));
        assert_eq!(from_json(&json).unwrap(), tree);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let bad = r#"{"schema_version":99,"sessions":{"sessions":[]}}"#;
        assert!(from_json(bad).is_err());
    }
}
