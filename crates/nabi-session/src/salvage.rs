//! 깨진 세션 파일에서 **살릴 수 있는 것을 살린다**(배치 AA).
//!
//! `load_tree`는 파싱이 실패하면 원본을 `.toml.bak`으로 보존하고 **빈 트리**를 돌려줬다.
//! 파일을 안 덮어쓴다는 점은 옳지만, 사용자가 보기에는 **저장한 서버가 전부 사라진** 것이다.
//! 50개를 쌓아 둔 사람이 항목 하나가 깨졌다고 49개를 잃을 이유는 없다.
//!
//! ## 어떻게 살리는가
//!
//! 세션 목록은 `Vec<SavedSession>`이다. 그래서 전체 파싱이 실패하면 **항목을 하나씩** 읽어
//! 성공한 것만 모은다. 깨진 항목만 빠지고 나머지는 그대로 돌아온다.
//!
//! 백업은 그대로 남긴다 — 살린 것이 전부라는 보장이 없으니, 원본은 사용자가 볼 수 있어야 한다.

use crate::model::{SavedSession, SessionTree};

/// 전체 파싱이 실패한 TOML에서 **읽히는 세션만** 건져 낸다.
///
/// 돌려주는 것은 `(살린 트리, 버린 항목 수)`. 버린 수를 함께 주는 이유는 조용히 넘어가지
/// 않기 위해서다 — 사용자는 몇 개를 잃었는지 알아야 원본을 뒤져 볼지 정할 수 있다.
pub fn salvage_toml(text: &str) -> (SessionTree, usize) {
    let Ok(root) = text.parse::<toml::Value>() else {
        // TOML 문법 자체가 깨졌으면 항목 경계를 알 수 없다. 건질 것이 없다.
        return (SessionTree::default(), 0);
    };
    let Some(items) = root
        .get("sessions")
        .and_then(|s| s.get("sessions"))
        .or_else(|| root.get("sessions"))
        .and_then(|v| v.as_array())
    else {
        return (SessionTree::default(), 0);
    };
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for item in items {
        match item.clone().try_into::<SavedSession>() {
            Ok(s) => kept.push(s),
            Err(_) => dropped += 1,
        }
    }
    (SessionTree { sessions: kept }, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::to_toml;
    use crate::model::{SavedSession, SessionKind};

    fn tree(names: &[&str]) -> SessionTree {
        SessionTree {
            sessions: names
                .iter()
                .map(|n| SavedSession {
                    name: (*n).to_string(),
                    folder: None,
                    kind: SessionKind::Local { shell: "pwsh".into() },
                    on_connect: None,
                    cwd: None,
                    is_ftp: false,
                    open_sftp: false,
                    tag: Default::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_healthy_file_yields_everything() {
        let text = to_toml(&tree(&["a", "b", "c"])).unwrap();
        let (got, dropped) = salvage_toml(&text);
        assert_eq!(got.sessions.len(), 3);
        assert_eq!(dropped, 0);
    }

    /// **이 시험이 이 파일의 이유다.** 항목 하나가 깨져도 나머지는 돌아온다.
    #[test]
    fn one_broken_entry_does_not_take_the_others() {
        let text = to_toml(&tree(&["a", "b", "c"])).unwrap();
        // 가운데 항목의 이름을 숫자로 바꿔 타입을 어긋뜨린다(손으로 고치다 흔히 나는 꼴).
        let broken = text.replacen("name = \"b\"", "name = 42", 1);
        assert_ne!(broken, text, "시험이 실제로 무언가를 망가뜨려야 한다");
        let (got, dropped) = salvage_toml(&broken);
        assert_eq!(dropped, 1, "깨진 항목 하나만 버린다");
        let names: Vec<&str> = got.sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c"], "나머지는 그대로 살아야 한다");
    }

    #[test]
    fn broken_toml_syntax_yields_nothing() {
        // 문법이 깨지면 항목 경계를 알 수 없다 — 억지로 건지려 들면 엉뚱한 것이 나온다.
        let (got, dropped) = salvage_toml("이건 [[ TOML 이 아니다");
        assert!(got.sessions.is_empty());
        assert_eq!(dropped, 0);
    }

    #[test]
    fn a_file_without_sessions_is_empty_not_a_panic() {
        let (got, dropped) = salvage_toml("schema_version = 1\n");
        assert!(got.sessions.is_empty());
        assert_eq!(dropped, 0);
    }

    #[test]
    fn every_entry_broken_means_nothing_kept_but_counted() {
        let text = to_toml(&tree(&["a", "b"])).unwrap();
        let broken = text.replace("name = ", "name = 1 # ");
        let (got, dropped) = salvage_toml(&broken);
        assert!(got.sessions.is_empty());
        assert_eq!(dropped, 2, "몇 개를 잃었는지는 알려 준다");
    }
}
