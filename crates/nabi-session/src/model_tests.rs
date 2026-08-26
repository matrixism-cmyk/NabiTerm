//! SessionTree 모델 단위 테스트(model.rs 라인 한도 유지를 위해 분리).

use crate::model::{session_matches, SavedSession, SessionKind, SessionTree};

fn ssh(name: &str, host: &str) -> SavedSession {
    SavedSession {
        name: name.into(),
        folder: None,
        kind: SessionKind::Ssh { host: host.into(), port: 22, user: "u".into(), credential_ref: None, key_path: None, jump: None, agent_forward: false },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    }
}

#[test]
fn sort_orders_by_folder_then_name() {
    let mut t = SessionTree::default();
    let mut b = ssh("beta", "h");
    b.folder = Some("z".into());
    let mut a = ssh("Alpha", "h");
    a.folder = Some("z".into());
    t.add(b);
    t.add(a);
    t.add(ssh("mid", "h")); // 폴더 없음 → 먼저.
    t.sort();
    assert_eq!(t.sessions.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["mid", "Alpha", "beta"]);
}

#[test]
fn rename_move_folders() {
    let mut t = SessionTree::default();
    t.add(ssh("a", "h1"));
    t.add(ssh("b", "h2"));
    assert!(t.rename("a", "a2"));
    assert!(!t.rename("a2", "b")); // 이름 충돌 거부.
    assert!(!t.rename("none", "x")); // 없는 세션.
    assert!(t.move_to_folder("a2", Some("work".into())));
    t.add({ let mut s = ssh("c", "h3"); s.folder = Some("work".into()); s });
    assert_eq!(t.folders(), vec!["work".to_string()]); // 중복 제거.
    assert!(t.move_to_folder("a2", Some("  ".into()))); // 공백 폴더 → None.
    assert_eq!(t.sessions.iter().find(|s| s.name == "a2").unwrap().folder, None);
}

#[test]
fn sort_group_rename_folder() {
    let mut t = SessionTree::default();
    let mut a = ssh("a", "zeta.com");
    a.folder = Some("work".into());
    let mut b = ssh("b", "alpha.com");
    b.folder = Some("work".into());
    t.add(a);
    t.add(b);
    t.add(ssh("c", "mid.com")); // 폴더 없음.
    t.sort_by_host();
    assert_eq!(t.sessions.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["b", "c", "a"]); // alpha<mid<zeta.
    let g = t.group_by_folder();
    assert_eq!(g[0].0, None); // 폴더 없는 그룹 먼저.
    assert_eq!(g[1].0.as_deref(), Some("work"));
    assert_eq!(t.rename_folder("work", "prod"), 2);
    assert_eq!(t.folders(), vec!["prod".to_string()]);
    assert_eq!(t.rename_folder("prod", "  "), 2); // 빈 새 이름 → 폴더 해제.
    assert!(t.folders().is_empty());
}

#[test]
fn remove_folder_drops_all() {
    let mut t = SessionTree::default();
    let mut a = ssh("a", "h");
    a.folder = Some("tmp".into());
    t.add(a);
    t.add(ssh("b", "h2")); // 폴더 없음.
    assert_eq!(t.remove_folder("tmp"), 1);
    assert_eq!(t.sessions.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(t.remove_folder("none"), 0);
}

#[test]
fn merge_findhost_moveall() {
    let mut t = SessionTree::default();
    t.add(ssh("a", "h1"));
    let mut other = SessionTree::default();
    other.add(ssh("b", "h2"));
    other.add(ssh("c", "h2")); // b/c는 같은 host h2 → 병합 후 dedup으로 하나 제거.
    let added = t.merge(other);
    assert_eq!(added, 1); // b만 순증(c는 중복 제거).
    assert_eq!(t.find_by_host("H1").len(), 1); // 대소문자 무시.
    let mut t2 = SessionTree::default();
    let mut s = ssh("x", "h");
    s.folder = Some("old".into());
    t2.add(s);
    assert_eq!(t2.move_all_to_folder(Some("old"), Some("new".into())), 1);
    assert_eq!(t2.folders(), vec!["new".to_string()]);
}

#[test]
fn find_and_filter() {
    let mut t = SessionTree::default();
    t.add(ssh("web", "example.com")); // user "u".
    t.add(ssh("db", "db.local"));
    assert!(t.find("web").is_some());
    assert!(t.find("none").is_none());
    assert_eq!(t.filter("example").len(), 1); // 호스트 매치.
    assert_eq!(t.filter("DB").iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["db"]); // 대소문자 무시.
    assert_eq!(t.filter("").len(), 2); // 빈 질의=전체.
    assert_eq!(t.filter("zzz").len(), 0);
}

#[test]
fn target_string_and_kind_label() {
    let mut s = ssh("x", "h"); // user "u", port 22.
    assert_eq!(s.target_string(), "u@h"); // 22 생략.
    assert_eq!(s.kind_label(), "SSH");
    if let SessionKind::Ssh { port, .. } = &mut s.kind {
        *port = 2222;
    }
    assert_eq!(s.target_string(), "u@h:2222");
    s.is_ftp = true;
    assert_eq!(s.kind_label(), "FTP");
    let local = SavedSession {
        name: "L".into(),
        folder: None,
        kind: SessionKind::Local { shell: "pwsh".into() },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    };
    assert_eq!(local.target_string(), "pwsh");
    assert_eq!(local.kind_label(), "Local");
}

#[test]
fn copy_name_and_counts() {
    let mut t = SessionTree::default();
    t.add(ssh("web", "h"));
    assert_eq!(t.unique_copy_name("web"), "web copy");
    t.add(ssh("web copy", "h2"));
    assert_eq!(t.unique_copy_name("web"), "web copy 2"); // 충돌 회피.
    t.add(SavedSession {
        name: "L".into(),
        folder: None,
        kind: SessionKind::Local { shell: "pwsh".into() },
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    });
    assert_eq!(t.ssh_count(), (2, 1)); // SSH 2 + 로컬 1.
}

#[test]
fn session_matches_and_hosts() {
    let mut s = ssh("Web", "example.com"); // user "u".
    s.folder = Some("Prod".into());
    assert!(session_matches(&s, "")); // 빈 질의=전체.
    assert!(session_matches(&s, "example")); // 호스트.
    assert!(session_matches(&s, "PROD")); // 폴더, 대소문자 무시.
    assert!(session_matches(&s, "web")); // 이름.
    assert!(session_matches(&s, "web prod")); // 다중 단어 AND(순서 무관, 이름+폴더 교차).
    assert!(!session_matches(&s, "web zzz")); // 한 토큰이라도 불일치면 제외.
    assert!(!session_matches(&s, "zzz"));
    let mut t = SessionTree::default();
    t.add(ssh("a", "h2"));
    t.add(ssh("b", "h1"));
    t.add(ssh("c", "h1"));
    assert_eq!(t.hosts(), vec!["h1".to_string(), "h2".to_string()]); // 중복 제거·정렬.
}

#[test]
fn dedup_keeps_first_per_target() {
    let mut t = SessionTree::default();
    t.add(ssh("a", "h1"));
    t.add(ssh("b", "h1")); // 같은 host/port/user → 중복.
    t.add(ssh("c", "h2"));
    assert_eq!(t.dedup(), 1);
    assert_eq!(t.sessions.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
}

#[cfg(test)]
mod tag_tests {
    use crate::{SavedSession, SessionKind, SessionTag};

    fn s(tag: SessionTag) -> SavedSession {
        SavedSession {
            name: "x".into(), folder: None,
            kind: SessionKind::Local { shell: "powershell".into() },
            on_connect: None, cwd: None, is_ftp: false, open_sftp: false, tag,
        }
    }

    /// 표식 없는 기존 세션 파일을 읽어도 그대로 열려야 한다(뒤로 호환).
    #[test]
    fn a_session_saved_before_tags_existed_still_loads() {
        let old = r#"{"name":"old","folder":null,"kind":{"Local":{"shell":"cmd"}}}"#;
        let got: SavedSession = serde_json::from_str(old).expect("옛 형식을 읽어야 한다");
        assert_eq!(got.tag, SessionTag::None);
    }

    /// 표식은 저장되고 그대로 돌아와야 한다 — 껐다 켜도 남는 것이 요점이다.
    #[test]
    fn a_tag_survives_a_save_and_load() {
        let json = serde_json::to_string(&s(SessionTag::Prod)).unwrap();
        assert!(json.contains("\"prod\""), "{json}");
        let back: SavedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tag, SessionTag::Prod);
    }

    /// 되돌릴 수 없는 곳은 운영뿐이다 — 여기가 넓어지면 확인이 잦아져 아무도 안 읽는다.
    #[test]
    fn only_production_asks_for_confirmation() {
        let risky: Vec<_> = SessionTag::ALL.iter().filter(|t| t.is_risky()).collect();
        assert_eq!(risky, vec![&SessionTag::Prod]);
    }

    /// 모든 표식에 라벨과 색이 있어야 한다 — 색만으로 구분하게 두지 않는다.
    #[test]
    fn every_tag_has_a_label_and_a_colour() {
        for t in SessionTag::ALL {
            assert!(t.key().starts_with("tag."), "{:?}", t);
            assert_ne!(t.rgb(), (0, 0, 0));
        }
        let keys: std::collections::HashSet<_> = SessionTag::ALL.iter().map(|t| t.key()).collect();
        assert_eq!(keys.len(), SessionTag::ALL.len(), "i18n 키가 겹친다");
    }
}

/// **표식으로도 걸러진다** — 세션이 늘면 "운영만 보기"가 이름 검색만큼 자주 필요하다.
#[test]
fn the_filter_sees_tags_too() {
    use crate::{session_matches, SessionTag};
    let mut prod = ssh("Web", "example.com");
    prod.tag = SessionTag::Prod;
    let mut dev = ssh("Web2", "example.com");
    dev.tag = SessionTag::Dev;
    assert!(session_matches(&prod, "prod"));
    assert!(!session_matches(&dev, "prod"), "개발 세션이 운영으로 걸렸다");
    assert!(session_matches(&dev, "dev"));
}

/// 표식 낱말과 이름을 **함께** 쓸 수 있어야 한다(둘 다 맞아야 걸린다).
#[test]
fn a_tag_word_combines_with_the_name() {
    use crate::{session_matches, SessionTag};
    let mut a = ssh("web-01", "a.com");
    a.tag = SessionTag::Prod;
    let mut b = ssh("db-01", "b.com");
    b.tag = SessionTag::Prod;
    assert!(session_matches(&a, "prod web"));
    assert!(!session_matches(&b, "prod web"), "이름이 안 맞는데 걸렸다");
}

/// 표식이 없는 세션이 아무 낱말에나 걸리면 안 된다(빈 낱말이 섞여 들어가는 실수).
#[test]
fn an_untagged_session_does_not_match_a_tag_word() {
    use crate::session_matches;
    let plain = ssh("Web", "example.com");
    assert!(!session_matches(&plain, "prod"));
    assert!(!session_matches(&plain, "dev"));
}
