//! 제어 평면 **동사 전수 스모크** — 문서에 적힌 동사를 실제 앱에 하나씩 던져 본다.
//!
//! ## 왜 필요한가
//!
//! 제어 평면은 우리가 다른 터미널과 갈라서는 자리다. AI 에이전트가 이 동사들로 나비텀을
//! 부린다. 그런데 지금까지 **끝까지 돌려 본 것은 다섯 개뿐**이었다(기동·spawn·send·
//! capture·close). 나머지 서른 남짓은 아무도 실제로 불러 본 적이 없다.
//!
//! 이름이 맞는지는 이미 검사한다(`agentguide` 의 대조 시험 둘). 하지만 이름이 맞아도
//! 동작이 죽어 있을 수 있다. 실제로 그런 일이 있었다 — 만들어 놓고 아무도 안 부르는
//! 함수가 아홉 개였고 그중 셋은 결함이었다(`xtask unused`, 배치 BD·BF).
//!
//! 여기서는 **부르면 오류 없이 답하는가**만 본다. 그것만으로도 "그 동사는 죽었다"를
//! 잡을 수 있고, 그게 에이전트에게는 가장 나쁜 상태다 — 에이전트는 오류를 받으면
//! 우리 프로그램이 고장 났다고 판단한다.
//!
//! ## 무엇을 부르지 않는가
//!
//! 되돌릴 수 없거나 바깥을 건드리는 것은 뺀다. 스모크가 사용자의 컴퓨터를 바꾸면 안 된다.
//!
//! * `update` — 프로그램을 갈아 끼우고 다시 켠다.
//! * `integration install` — 사용자의 `~/.claude/settings.json` 을 고친다.
//! * `schedule create` — 일정이 파일에 남는다.
//! * `open-sftp`·`sftp-*` — 원격 연결이 있어야 한다(실서버 시험이 따로 있다).
//! * `web` — 엣지 런타임이 필요하고, 없는 PC 에서는 실패가 정상이다.
//! * `events`·`tail` — 끝나지 않는 흐름이라 왕복 하나로 볼 수 없다.

use std::io::BufRead;

/// 한 동사 시험: (이름, 보낼 JSON, 성공으로 칠 조건).
struct Probe {
    name: &'static str,
    req: String,
}

/// 안전하게 부를 수 있는 동사들을 만든다. `pane` 은 미리 띄워 둔 pane 번호다.
fn probes(pane: u64) -> Vec<Probe> {
    let p = |name: &'static str, req: String| Probe { name, req };
    vec![
        p("list-panes", r#"{"op":"list-panes"}"#.into()),
        p("capture", format!(r#"{{"op":"capture","pane":{pane},"lines":5}}"#)),
        p("pane-modes", format!(r#"{{"op":"pane-modes","pane":{pane}}}"#)),
        p("agent-explain", format!(r#"{{"op":"agent-explain","pane":{pane}}}"#)),
        p("layout-export", r#"{"op":"layout-export"}"#.into()),
        p("web-list", r#"{"op":"web-list"}"#.into()),
        p("set-title", format!(r#"{{"op":"set-title","pane":{pane},"title":"e2e"}}"#)),
        p("focus", format!(r#"{{"op":"focus","pane":{pane}}}"#)),
        p("resize", format!(r#"{{"op":"resize","pane":{pane},"cols":90,"rows":30}}"#)),
        p("notify", r#"{"op":"notify","title":"e2e","body":""}"#.into()),
        p("progress-set", format!(r#"{{"op":"progress","pane":{pane},"percent":42}}"#)),
        p("progress-clear", format!(r#"{{"op":"progress","pane":{pane},"percent":null}}"#)),
        // pane 은 `hello` 에서 밝힌 from 으로 정해진다 — 요청에 pane 을 담지 않는다.
        p("pane-status-set", r#"{"op":"pane-status-set","key":"model","value":"e2e"}"#.into()),
        p("pane-status-clear", r#"{"op":"pane-status-set","key":"model","value":null}"#.into()),
        // 화면 캡처는 **파일이 생겼는지까지** 본다 — 오류 없이 끝나고 아무것도 안 남는
        // 것이 가장 나쁘다(부른 쪽은 찍힌 줄 안다). 경로를 우리가 정해 확인한다.
        p(
            "screenshot",
            format!(r#"{{"op":"screenshot","pane":null,"out":"{}"}}"#, json_path(&shot_path())),
        ),
        p("show-history", format!(r#"{{"op":"show-history","pane":{pane}}}"#)),
        // 파일 브라우저 탭을 연다 — 프로세스를 만들지 않아 스모크에 안전하다.
        p("open-browser", r#"{"op":"open-browser","path":null}"#.into()),
    ]
}

/// 화면 캡처를 받아 볼 임시 파일 경로.
fn shot_path() -> String {
    std::env::temp_dir().join("nabi-e2e-shot.png").display().to_string()
}

/// 윈도우 경로를 JSON 문자열에 넣을 수 있게 고친다 — 역슬래시가 이스케이프로 읽힌다.
fn json_path(p: &str) -> String {
    p.replace('\\', "/")
}

/// 동사들을 하나씩 던지고 실패한 것만 모아 돌려준다.
///
/// 하나가 실패해도 멈추지 않는다 — 몇 개가 죽었는지 한 번에 알아야 고칠 계획을 세운다.
pub(crate) fn sweep(
    pipe: &mut std::fs::File,
    rd: &mut impl BufRead,
    pane: u64,
) -> Result<(), String> {
    let mut bad: Vec<String> = Vec::new();
    let all = probes(pane);
    let total = all.len();
    for pr in all {
        match crate::e2e::roundtrip(pipe, rd, &pr.req) {
            Ok(r) if is_ok(&r) => {}
            Ok(r) => bad.push(format!("  {} → {}", pr.name, first_line(&r))),
            Err(e) => bad.push(format!("  {} → 왕복 실패: {e}", pr.name)),
        }
    }
    // 찍었다고 답했으면 파일이 있어야 한다.
    let shot = shot_path();
    match std::fs::metadata(&shot).map(|m| m.len()) {
        Ok(n) if n > 0 => {
            let _ = std::fs::remove_file(&shot);
        }
        Ok(_) => bad.push(format!("  screenshot → 파일은 생겼는데 비어 있다: {shot}")),
        Err(e) => bad.push(format!("  screenshot → 찍었다고 했는데 파일이 없다: {e}")),
    }
    println!("제어 동사 {} 개 시험 · 실패 {}", total, bad.len());
    match bad.is_empty() {
        true => Ok(()),
        false => Err(format!("죽은 제어 동사 {}개:\n{}", bad.len(), bad.join("\n"))),
    }
}

/// 응답이 성공인가. 우리 서버는 성공을 `"res":"ok"` 나 자료가 담긴 응답으로 답한다.
///
/// 오류만 확실히 가려낸다 — 응답 모양은 동사마다 달라서 "무엇이 와야 한다"를 여기 적어
/// 두면 동사가 늘 때마다 이 표가 실제와 달라진다.
fn is_ok(resp: &str) -> bool {
    !resp.contains(r#""res":"err""#) && !resp.contains(r#""error""#)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(160).collect()
}
