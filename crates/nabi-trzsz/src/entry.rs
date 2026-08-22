//! 전송 항목 하나 — 디렉터리 모드에서는 `#NAME`이 이름이 아니라 **JSON**으로 온다.
//!
//! ```json
//! {"path_id":0,"path_name":["docs","img","a.png"],"is_dir":false,"size":1234,"perm":null}
//! ```
//!
//! `path_name`의 첫 조각이 최상위 이름이고 나머지가 그 아래 경로다. `path_id`는 같은
//! 최상위 폴더에 속한 항목을 묶는다 — 최상위 이름이 겹쳐 바뀌면 **그 새 이름을 계속 써야**
//! 폴더가 둘로 갈라지지 않는다.

use serde::{Deserialize, Serialize};

/// 보내거나 받을 항목 하나.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 같은 최상위 경로에 속한 항목을 묶는 번호.
    #[serde(default)]
    pub path_id: i64,
    /// 상대 경로 조각. 단일 파일 전송이면 조각 하나.
    #[serde(rename = "path_name")]
    pub rel: Vec<String>,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub size: u64,
    /// 원격이 알려준 권한(우리는 쓰지 않는다 — 받은 파일에 실행 권한을 붙이지 않는다).
    #[serde(default)]
    pub perm: Option<u32>,
}

impl Entry {
    /// 평범한 파일 하나(디렉터리 모드가 아닐 때).
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Self { path_id: 0, rel: vec![name.into()], is_dir: false, size, perm: None }
    }

    /// 화면에 보여줄 이름 — 경로의 마지막 조각.
    pub fn name(&self) -> &str {
        self.rel.last().map_or("", String::as_str)
    }

    /// 최상위 이름(폴더 전송이면 폴더 이름).
    pub fn root(&self) -> &str {
        self.rel.first().map_or("", String::as_str)
    }

    /// `#NAME`에 실을 페이로드. 디렉터리 모드면 JSON, 아니면 이름 그대로.
    pub fn wire_name(&self, directory: bool) -> String {
        if directory {
            serde_json::to_string(self).unwrap_or_else(|_| self.name().to_owned())
        } else {
            self.name().to_owned()
        }
    }

    /// 원격이 보낸 `#NAME`을 읽는다.
    pub fn parse(payload: &str, directory: bool) -> Result<Self, String> {
        if !directory {
            return Ok(Self::file(payload, 0));
        }
        let mut e: Self =
            serde_json::from_str(payload).map_err(|err| format!("bad NAME json: {err}"))?;
        if e.rel.is_empty() {
            return Err("NAME has no path".into());
        }
        // 조각 하나하나가 이름이어야 한다. 여기서 걸러도 저장소가 다시 검사한다(이중 방어).
        e.rel.retain(|p| !p.is_empty());
        if e.rel.is_empty() {
            return Err("NAME path is all empty parts".into());
        }
        Ok(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_mode_uses_the_name_as_is() {
        let e = Entry::parse("report.txt", false).unwrap();
        assert_eq!(e.name(), "report.txt");
        assert_eq!(e.wire_name(false), "report.txt");
        assert!(!e.is_dir);
    }

    #[test]
    fn directory_mode_reads_the_json_shape() {
        let j = r#"{"path_id":2,"path_name":["docs","img","a.png"],"is_dir":false,"size":9}"#;
        let e = Entry::parse(j, true).unwrap();
        assert_eq!(e.path_id, 2);
        assert_eq!(e.rel, ["docs", "img", "a.png"]);
        assert_eq!(e.root(), "docs");
        assert_eq!(e.name(), "a.png");
        assert_eq!(e.size, 9);
    }

    #[test]
    fn a_directory_entry_carries_no_data() {
        let j = r#"{"path_id":0,"path_name":["docs"],"is_dir":true,"size":0}"#;
        let e = Entry::parse(j, true).unwrap();
        assert!(e.is_dir);
        assert_eq!(e.name(), "docs");
    }

    #[test]
    fn round_trips_through_the_wire() {
        let e = Entry {
            path_id: 1,
            rel: vec!["a".into(), "b.txt".into()],
            is_dir: false,
            size: 5,
            perm: None,
        };
        assert_eq!(Entry::parse(&e.wire_name(true), true).unwrap(), e);
    }

    #[test]
    fn refuses_a_nameless_entry() {
        assert!(Entry::parse(r#"{"path_name":[]}"#, true).is_err());
        assert!(Entry::parse(r#"{"path_name":["",""]}"#, true).is_err());
        assert!(Entry::parse("not json", true).is_err());
    }

    /// 우리가 보내는 JSON은 원격이 읽을 수 있는 키 이름을 써야 한다.
    #[test]
    fn wire_json_uses_the_protocol_keys() {
        let e = Entry { path_id: 3, rel: vec!["d".into()], is_dir: true, size: 0, perm: None };
        let s = e.wire_name(true);
        assert!(s.contains("\"path_name\":[\"d\"]"), "{s}");
        assert!(s.contains("\"path_id\":3"), "{s}");
        assert!(s.contains("\"is_dir\":true"), "{s}");
    }
}
