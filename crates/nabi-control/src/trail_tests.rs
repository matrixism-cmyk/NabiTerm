//! `trail.rs` 시험 — 특히 **비밀이 기록에 닿지 않는지**를 지킨다.
//!
//! 본체에서 떼어냈다(라인 한도). 떼어 놓으니 읽는 사람이 "무엇을 지키기로 했는가"를
//! 한 파일에서 볼 수 있다 — 이 모듈의 약속은 코드보다 시험에 더 분명히 적혀 있다.

use crate::trail::*;

fn e(verb: &'static str, target: &str, outcome: Outcome) -> Entry {
    Entry {
        at_secs: 1,
        from: "pane 3".into(),
        verb,
        target: target.into(),
        outcome,
        bytes: 0,
    }
}

#[test]
fn entries_come_back_newest_last() {
    let mut t = Trail::new(10);
    t.push(e("spawn", "a", Outcome::Allowed));
    t.push(e("close", "b", Outcome::Denied));
    let all = t.entries();
    let got: Vec<&str> = all.iter().map(|x| x.target.as_str()).collect();
    assert_eq!(got, vec!["a", "b"], "최신이 뒤에 와야 한다");
}

#[test]
fn export_is_pasteable() {
    let list = vec![e("send-input", "pane 3", Outcome::Denied)];
    let text = export(&list);
    assert!(text.starts_with("time(s)\tfrom\t"), "머리글이 있어야 한다");
    assert!(text.contains("send-input\tpane 3\tdenied"), "{text}");
    assert!(text.ends_with('\n'));
}

#[test]
fn a_denied_request_is_recorded_too() {
    // 무엇을 시도했는지가 감사의 절반이다. 거부를 안 남기면 절반이 사라진다.
    let list = vec![e("send-input", "pane 1", Outcome::Denied)];
    assert!(export(&list).contains("denied"));
}

#[test]
fn the_ring_drops_the_oldest_not_the_newest() {
    // 넘치면 오래된 것이 밀려나야 한다. 최신이 사라지면 사고 직후를 못 본다.
    let mut t = Trail::new(3);
    for i in 0..5 {
        t.push(e("spawn", &format!("x{i}"), Outcome::Allowed));
    }
    let all = t.entries();
    let got: Vec<&str> = all.iter().map(|x| x.target.as_str()).collect();
    assert_eq!(got, vec!["x2", "x3", "x4"], "가장 최근 셋만 남아야 한다");
}

#[test]
fn a_zero_cap_still_keeps_one() {
    // 0으로 만들면 아무것도 안 남아 기록이 무의미해진다 — 최소 하나는 지킨다.
    let mut t = Trail::new(0);
    t.push(e("spawn", "only", Outcome::Allowed));
    assert_eq!(t.len(), 1);
    assert!(!t.is_empty());
}

#[test]
fn the_global_ring_accepts_records() {
    // 전역 경로도 한 번은 지나가 봐야 한다 — 순수 구조만 시험하면 배선이 빠져도 모른다.
    let before = crate::trail::len();
    record(e("spawn", "global-check", Outcome::Allowed));
    // 상한에 닿아 있으면 건수가 안 늘 수도 있다. 이 시험의 요점은 **들어갔는가**다.
    let _ = before;
    assert!(entries().iter().any(|x| x.target == "global-check"));
}

/// **감사 기록이 새 유출 경로가 되면 안 된다.**
///
/// `SendInput` 의 본문에는 비밀번호가 지나간다. 기록에 그것이 들어가면, 사용자를
/// 지키려고 만든 것이 오히려 비밀을 한곳에 모아 두는 자리가 된다. 가리기(redact)로
/// 막지 않고 **아예 담지 않는** 쪽을 골랐으므로, 그것을 시험이 지킨다.
#[test]
fn the_body_of_an_input_never_reaches_the_trail() {
    use crate::protocol::ControlRequest as R;
    let secret = "sudo -S mypassword123";
    let req = R::SendInput { pane: 3, data: secret.into(), raw: false };
    let (verb, target, bytes) = describe(&req);
    assert_eq!(verb, "send-input");
    assert_eq!(target, "pane 3");
    assert_eq!(bytes, secret.len(), "길이는 남는다 — 크기는 되짚을 때 쓴다");
    // 어느 칸에도 본문이 없어야 한다.
    for field in [verb, target.as_str()] {
        assert!(!field.contains("mypassword"), "본문이 샜다: {field}");
        assert!(!field.contains("sudo"), "본문이 샜다: {field}");
    }
    let text = export(&[Entry {
        at_secs: 0, from: "pane 3".into(), verb, target, outcome: Outcome::Allowed, bytes,
    }]);
    assert!(!text.contains("mypassword"), "내보내기에 본문이 샜다");
}

#[test]
fn a_pane_status_value_is_not_recorded() {
    // 에이전트가 아무 글이나 넣을 수 있는 자리다 — 열쇠만 남긴다.
    use crate::protocol::ControlRequest as R;
    let req = R::PaneStatusSet {
        key: "phase".into(),
        value: Some("고객 이름 홍길동".into()),
        ttl_ms: None,
    };
    let (_, target, _) = describe(&req);
    assert_eq!(target, "phase");
    assert!(!target.contains("홍길동"));
}

#[test]
fn a_notify_body_is_not_recorded() {
    use crate::protocol::ControlRequest as R;
    let req = R::Notify { title: "빌드 실패".into(), body: "토큰 ghp_abc123".into() };
    let (verb, target, _) = describe(&req);
    assert_eq!(verb, "notify");
    assert!(target.is_empty(), "제목·본문 모두 담지 않는다");
}

#[test]
fn paths_are_recorded_because_that_is_the_point() {
    // 어떤 파일을 가져갔는지는 감사의 핵심이다 — 경로는 남긴다.
    use crate::protocol::ControlRequest as R;
    let req = R::SftpGet { pane: None, remote: "/etc/shadow".into(), local: "C:/tmp/x".into() };
    let (verb, target, _) = describe(&req);
    assert_eq!((verb, target.as_str()), ("sftp-get", "/etc/shadow"));
}

/// **동사 이름이 겹치면 기록이 모호해진다.**
///
/// 새 요청을 더하면서 옆 갈래를 복사해 오는 것이 가장 흔한 실수다. 그러면 서로 다른
/// 두 동작이 같은 이름으로 남고, 기록을 읽는 사람은 무엇이 일어났는지 가릴 수 없다.
/// 컴파일러는 갈래가 **빠진 것**은 잡아 주지만 **같은 이름**은 못 잡는다.
#[test]
fn every_verb_name_is_unique_and_nonempty() {
    use crate::protocol::ControlRequest as R;
    // 모든 변형을 하나씩 만든다. 새 변형을 더하면 `describe` 가 컴파일 오류로 알려 주고,
    // 여기에도 더해야 이 시험이 그것을 본다.
    let all = vec![
        R::Hello { token: String::new(), from: None },
        R::ListPanes,
        R::PaneModes { pane: 1 },
        R::Capture { pane: 1, lines: 10, start: None, end: None, escapes: false, view: false },
        R::AgentExplain { pane: 1 },
        R::SendInput { pane: 1, data: "x".into(), raw: false },
        R::ClosePane { pane: 1 },
        R::Resize { pane: 1, cols: 80, rows: 24 },
        R::Focus { pane: 1 },
        R::SetTitle { pane: 1, title: "t".into() },
        R::PaneStatusSet { key: "k".into(), value: None, ttl_ms: None },
        R::OpenBrowser { path: None },
        R::OpenHere { path: "p".into() },
        R::OpenEditor { path: "p".into() },
        R::OpenSftp { session: "s".into() },
        R::SftpList { pane: None, path: "p".into() },
        R::Notify { title: "t".into(), body: "b".into() },
        R::LayoutExport,
        R::Tail { pane: 1 },
    ];
    let mut seen = std::collections::HashSet::new();
    for req in &all {
        let (verb, _, _) = describe(req);
        assert!(!verb.is_empty(), "{req:?} 의 동사가 비어 있다");
        assert!(seen.insert(verb), "동사 이름이 겹친다: {verb}");
    }
    assert!(seen.len() >= 19, "훑기가 망가졌다({}개)", seen.len());
}
