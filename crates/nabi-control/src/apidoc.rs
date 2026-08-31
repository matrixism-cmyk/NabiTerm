//! 제어 프로토콜 자기 문서(B3, `nabi cli api schema`) — 에이전트가 verb 어휘를 스스로 학습.
//!
//! schemars 의존을 들이는 대신 손으로 유지하되, **드리프트를 테스트로 막는다**: 각
//! ControlRequest 변형의 직렬화 op 이름이 이 문서에 전부 존재해야 테스트가 통과한다.
//! 변형을 추가하고 문서를 잊으면 게이트가 잡는다.

/// 프로토콜 문서(JSON). op 이름 → {params, doc}.
pub fn api_doc() -> serde_json::Value {
    serde_json::json!({
        "transport": "named pipe(NABI_CONTROL_PIPE), line-delimited JSON, Hello(token) 선행",
        "ops": {
            "hello": { "params": ["token", "from?"], "doc": "접속 인사+토큰 검증" },
            "list-panes": { "params": [], "doc": "pane 목록·상태" },
            "capture": { "params": ["pane", "lines", "start?", "end?", "escapes?", "view?"], "doc": "화면/스크롤백 캡처" },
            "spawn-terminal": { "params": ["shell", "cwd?", "dock?", "ssh?"], "doc": "새 pane" },
            "send-input": { "params": ["pane", "data", "raw?"], "doc": "입력 주입(bracketed 보호)" },
            "close-pane": { "params": ["pane"], "doc": "pane 닫기" },
            "resize": { "params": ["pane", "cols", "rows"], "doc": "그리드 크기" },
            "open-browser": { "params": ["path?"], "doc": "파일 브라우저 탭" },
            "open-editor": { "params": ["path"], "doc": "파일을 nabiPad 편집기로 연다. CLI 낱말은 `nabi cli open-file`" },
            "open-here": { "params": ["path"], "doc": "그 폴더에서 새 터미널을 열고 창을 앞으로" },
            "web-list": { "params": [], "doc": "열려 있는 웹 탭 목록(번호, 주소, 제목)" },
            "web-eval": { "params": ["pane?", "js"], "doc": "웹 탭에서 자바스크립트를 실행하고 결과를 JSON 으로 받는다" },
            "show-history": { "params": ["pane?"], "doc": "그 pane 의 전체 기록을 화면에 띄운다(휠을 올렸을 때와 같은 겹 화면). CLI 낱말은 `nabi cli history`" },
            "web-act": { "params": ["pane?", "act", "arg?"], "doc": "웹 탭 조종 — back|forward|reload|stop|goto|zoom|shot|pdf|text (CLI 는 web-back 등으로 부른다)" },
            "restart": { "params": [], "doc": "껐다 다시 켠다 — 묻지 않는다. 작업 공간이 저장되어 탭·분할이 돌아온다" },
            "quit": { "params": [], "doc": "나비텀을 끝낸다 — 묻지 않는다. 작업 공간은 저장한다" },
            "scroll": { "params": ["pane", "lines?", "to?"], "doc": "pane 스크롤백을 옮긴다(+과거/-최신, --top/--bottom). screenshot 과 함께 쓰면 사람이 보는 화면을 그대로 본다" },
            "self-update": { "params": ["check?"], "doc": "최신판으로 스스로 올린다(--check 면 확인만). 조용히 설치하고 다시 켠다. CLI 낱말은 `nabi cli update`" },
            "open-web": { "params": ["url?", "window?"], "doc": "내장 웹 브라우저를 탭으로 연다(--window 면 별도 창). 파일 브라우저는 open-browser. CLI 낱말은 `nabi cli web`" },
            "progress": { "params": ["pane", "pct?"], "doc": "진행률을 상태 표시줄에 띄운다(pct 없으면 지운다)" },
            "screenshot": { "params": ["pane?", "out?"], "doc": "화면을 PNG 로 뜬다(capture 는 글자, 이건 점)" },
            "pane-modes": { "params": ["pane"], "doc": "터미널 모드 진단(대체화면·마우스보고·1007 등)" },
            "open-sftp": { "params": ["session"], "doc": "저장 세션 SFTP 탭" },
            "wait": { "params": ["pane", "until", "timeout_ms", "match_text?", "match_regex?"], "doc": "조건 대기: exit|command-done|idle|output(+패턴)|agent:<state>" },
            "tail": { "params": ["pane"], "doc": "출력 스트림" },
            "subscribe": { "params": ["pane?", "kinds?"], "doc": "이벤트 스트림: spawned|exit|output|command-done|agent-status|cwd" },
            "focus": { "params": ["pane"], "doc": "탭 활성화" },
            "set-title": { "params": ["pane", "title"], "doc": "탭 제목" },
            "notify": { "params": ["title", "body"], "doc": "토스트" },
            "pane-status-set": { "params": ["key", "value?", "ttl_ms?"], "doc": "상태 발행(state 키=권위, label.<state>=라벨)" },
            "agent-explain": { "params": ["pane"], "doc": "상태 감지 근거" },
            "schedule-create": { "params": ["name", "spec", "kind", "payload", "pane_title"], "doc": "스케줄 등록(send|command|notify)" },
            "layout-export": { "params": [], "doc": "레이아웃 JSON(panes 목록+분할 tree) — layout apply가 panes를 소비" },
            "sftp-list": { "params": ["path"], "doc": "열린 SFTP 연결의 원격 목록(JSON 배열)" },
            "sftp-get": { "params": ["remote", "local"], "doc": "원격→로컬 단일 파일 다운로드(완료 대기)" },
            "sftp-put": { "params": ["local", "remote"], "doc": "로컬→원격 단일 파일 업로드(완료 대기)" },
        }
    })
}

#[cfg(test)]
mod tests {
    /// **진짜 목록은 소스에 있다** — 여기서 그것을 읽어 문서와 맞대 본다.
    ///
    /// 예전에는 손으로 적은 표본 스물두 개를 돌렸다. 그러면 **잊은 것은 애초에 안 본다** —
    /// 잊었기 때문에 표본에도 없기 때문이다. 실제로 넷이 그렇게 어긋난 채 통과했다
    /// (`open-editor` 를 `open-file` 로 적는 식으로, wire 이름 자리에 CLI 낱말이 들어갔다).
    /// 문서를 믿고 부른 AI 는 "알 수 없는 동작"만 돌려받았고, 시험은 계속 초록이었다.
    fn real_ops() -> Vec<String> {
        let src = include_str!("protocol.rs");
        let mut out = Vec::new();
        let mut inside = false;
        for line in src.lines() {
            if line.contains("pub enum ControlRequest") {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if line == "}" {
                break; // 다음 enum(ControlResponse)까지 넘어가지 않는다.
            }
            // 변형은 들여쓰기 네 칸에 대문자로 시작한다. 속성·주석·필드는 걸러진다.
            let Some(rest) = line.strip_prefix("    ") else { continue };
            if !rest.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            let name: String = rest.chars().take_while(char::is_ascii_alphanumeric).collect();
            if !name.is_empty() {
                out.push(kebab(&name));
            }
        }
        out
    }

    /// `OpenEditor` → `open-editor`. serde 의 kebab-case 와 같은 규칙이다.
    fn kebab(name: &str) -> String {
        let mut s = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                s.push('-');
            }
            s.push(c.to_ascii_lowercase());
        }
        s
    }

    /// 소스에 있는 동작은 전부 문서에 있어야 한다 — **그리고 그 반대도.**
    ///
    /// 반대쪽이 없으면 있지도 않은 동작이 문서에 남는다. 그것이 더 나쁘다:
    /// 빠진 것은 AI 가 모르고 지나가지만, **틀린 것은 AI 가 믿고 부른다.**
    #[test]
    fn the_document_and_the_protocol_say_the_same_thing() {
        let doc = super::api_doc();
        let ops = doc["ops"].as_object().expect("ops");
        let real = real_ops();
        assert!(real.len() > 30, "소스에서 동작을 못 읽었다({}개) — 파싱이 깨졌다", real.len());
        for op in &real {
            assert!(ops.contains_key(op), "문서에 '{op}' 가 빠졌다 — apidoc.rs 를 갱신하라");
        }
        for name in ops.keys() {
            assert!(
                real.contains(name),
                "문서의 '{name}' 은 프로토콜에 없다 — CLI 낱말을 wire 이름 자리에 적지 않았는지 보라"
            );
        }
    }

    /// 하나라도 실제로 직렬화해 봐서 kebab 규칙 추정이 맞는지 확인한다.
    /// 이것이 없으면 위 시험은 자기가 만든 규칙끼리만 맞춰 보는 셈이 된다.
    #[test]
    fn the_kebab_guess_matches_what_serde_actually_writes() {
        let v = serde_json::to_value(crate::protocol::ControlRequest::OpenEditor {
            path: String::new(),
        })
        .unwrap();
        assert_eq!(v["op"].as_str(), Some("open-editor"));
        assert_eq!(kebab("OpenEditor"), "open-editor");
    }
}
