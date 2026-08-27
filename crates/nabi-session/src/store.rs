//! 세션 트리 파일 저장/로드(원자적).

use crate::export::from_toml;
use crate::model::SessionTree;
use std::path::Path;

/// sessions.toml에서 트리를 로드한다(없으면 빈 트리).
///
/// 파싱 실패는 "파일 없음"과 구분한다 — 그냥 빈 트리를 돌려주면 다음 저장이 깨진 파일을
/// 덮어써 저장된 세션이 통째로 사라진다. 손상 시 원본을 `.toml.bak`으로 보존한다.
pub fn load_tree(path: &Path) -> SessionTree {
    load_tree_reporting(path).0
}

/// 로드 + 손상 시 보존된 백업 경로(호출측이 사용자에게 알릴 수 있게).
pub fn load_tree_reporting(path: &Path) -> (SessionTree, Option<std::path::PathBuf>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (SessionTree::default(), None); // 파일 없음 = 정상 초기 상태.
    };
    match from_toml(&text) {
        Ok(t) => (t, None),
        Err(_) => {
            // 통째로 못 읽어도 **읽히는 항목은 살린다**. 50개를 쌓아 둔 사람이 하나가
            // 깨졌다고 49개를 잃을 이유는 없다. 원본은 그대로 백업해 둔다 — 살린 것이
            // 전부라는 보장이 없으니 사용자가 직접 볼 수 있어야 한다.
            let (salvaged, _dropped) = crate::salvage::salvage_toml(&text);
            (salvaged, backup_corrupt(path))
        }
    }
}

/// 깨진 파일을 기존 백업과 겹치지 않는 이름으로 옮긴다. 성공 시 백업 경로.
fn backup_corrupt(path: &Path) -> Option<std::path::PathBuf> {
    (0..100).find_map(|n| {
        let ext = if n == 0 { "toml.bak".to_string() } else { format!("toml.bak{n}") };
        let bak = path.with_extension(ext);
        (!bak.exists())
            .then(|| std::fs::rename(path, &bak).ok().map(|_| bak))
            .flatten()
    })
}

/// 트리를 원자적으로 저장한다.
///
/// 쓰기 자체는 `nabi_config::persist::save`에 맡긴다. 예전에는 여기에 같은 코드가 한 벌 더
/// 있었는데, 그쪽 주석에는 "단일 진실원"이라고 적혀 있으면서 실제로는 둘이었다.
///
/// 그리고 둘이 **이미 어긋나 있었다.** 설정 쪽은 임시 파일 이름에 프로세스 번호를 붙이는데
/// 여기는 안 붙여서, nabiTerm을 두 개 띄우면 같은 임시 파일을 놓고 다퉜다 — 한쪽이 반쯤 쓴
/// 파일을 다른 쪽이 제자리로 옮기면 저장된 세션이 깨진다.
pub fn save_tree(path: &Path, tree: &SessionTree) -> std::io::Result<()> {
    nabi_config::persist::save(path, &crate::export::SessionExport::new(tree.clone()))
}


#[cfg(test)]
mod tests {
    use super::{load_tree_reporting, save_tree};
    use crate::model::{SavedSession, SessionKind, SessionTree};

    fn ssh(name: &str, host: &str) -> SavedSession {
        SavedSession {
            name: name.into(),
            folder: None,
            kind: SessionKind::Ssh {
                host: host.into(),
                port: 22,
                user: "root".into(),
                credential_ref: None,
                key_path: None,
                jump: None,
                agent_forward: false,
            },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        }
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("nabi-store-{}-{name}.toml", std::process::id()))
    }

    #[test]
    fn missing_file_is_not_corruption() {
        let p = tmp("missing");
        let _ = std::fs::remove_file(&p);
        let (tree, bak) = load_tree_reporting(&p);
        assert!(tree.sessions.is_empty());
        assert!(bak.is_none(), "파일 없음은 백업 대상이 아니다");
    }

    #[test]
    fn corrupt_file_is_preserved_not_lost() {
        let p = tmp("corrupt");
        let _ = std::fs::remove_file(p.with_extension("toml.bak"));
        std::fs::write(&p, b"this is not valid toml {{{").unwrap();
        let (tree, bak) = load_tree_reporting(&p);
        assert!(tree.sessions.is_empty());
        let bak = bak.expect("손상 파일은 백업되어야 한다");
        assert!(bak.exists(), "백업이 실제로 존재해야 한다");
        assert!(!p.exists(), "원본은 백업으로 옮겨진다");
        let _ = std::fs::remove_file(&bak);
    }

    #[test]
    fn roundtrip_preserves_sessions() {
        let p = tmp("roundtrip");
        let mut t = SessionTree::default();
        t.add(ssh("prod", "example.com"));
        save_tree(&p, &t).unwrap();
        let (back, bak) = load_tree_reporting(&p);
        assert!(bak.is_none(), "정상 파일은 백업하지 않는다");
        assert_eq!(back.sessions.len(), 1);
        assert_eq!(back.sessions[0].name, "prod");
        let _ = std::fs::remove_file(&p);
    }

    /// **깨진 파일에서도 읽히는 세션은 돌아온다** — 백업은 그대로 남긴다.
    ///
    /// 예전에는 빈 트리를 돌려줬다. 파일을 안 덮어쓴다는 점은 옳았지만, 사용자가 보기에는
    /// 저장한 서버가 전부 사라진 것이었다.
    #[test]
    fn a_corrupt_file_still_gives_back_what_it_can() {
        let path = tmp("salv");
        let mut tree = SessionTree::default();
        tree.add(ssh("first", "h1"));
        tree.add(ssh("second", "h2"));
        tree.add(ssh("third", "h3"));
        save_tree(&path, &tree).unwrap();

        // 가운데 항목의 포트를 문자열로 바꾼다(손으로 고치다 흔히 나는 꼴).
        let text = std::fs::read_to_string(&path).unwrap();
        let broken = text.replacen("host = \"h2\"", "host = 22", 1);
        assert_ne!(broken, text, "시험이 실제로 무언가를 망가뜨려야 한다");
        std::fs::write(&path, &broken).unwrap();

        let (got, bak) = load_tree_reporting(&path);
        let names: Vec<&str> = got.sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["first", "third"], "멀쩡한 둘은 살아야 한다");
        let bak = bak.expect("원본은 백업으로 남아야 한다");
        assert!(bak.exists(), "백업 파일이 실제로 있어야 한다");
        let _ = std::fs::remove_file(&bak);
        let _ = std::fs::remove_file(&path);
    }
}
