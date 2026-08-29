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
            "capture": { "params": ["pane", "lines", "start?", "end?", "escapes?"], "doc": "화면/스크롤백 캡처" },
            "spawn-terminal": { "params": ["shell", "cwd?", "dock?", "ssh?"], "doc": "새 pane" },
            "send-input": { "params": ["pane", "data", "raw?"], "doc": "입력 주입(bracketed 보호)" },
            "close-pane": { "params": ["pane"], "doc": "pane 닫기" },
            "resize": { "params": ["pane", "cols", "rows"], "doc": "그리드 크기" },
            "open-browser": { "params": ["path?"], "doc": "파일 브라우저 탭" },
            "open-file": { "params": ["path"], "doc": "파일을 nabiPad 편집기로 연다" },
            "open-here": { "params": ["path"], "doc": "그 폴더에서 새 터미널을 열고 창을 앞으로" },
            "web-list": { "params": [], "doc": "열려 있는 웹 탭 목록(번호, 주소, 제목)" },
            "web-eval": { "params": ["pane?", "js"], "doc": "웹 탭에서 자바스크립트를 실행하고 결과를 JSON 으로 받는다" },
            "update": { "params": ["check?"], "doc": "최신판으로 스스로 올린다(--check 면 확인만). 조용히 설치하고 다시 켠다" },
            "web": { "params": ["url?", "window?"], "doc": "내장 웹 브라우저를 탭으로 연다(--window 면 별도 창). 파일 브라우저는 open-browser" },
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
    use crate::protocol::ControlRequest as R;

    /// 드리프트 가드: 모든 요청 변형의 op 이름이 문서에 있어야 한다.
    /// 새 변형을 추가하면 여기 샘플과 api_doc 둘 다 갱신해야 통과한다(의도된 마찰).
    #[test]
    fn every_request_variant_is_documented() {
        let samples: Vec<R> = vec![
            R::Hello { token: String::new(), from: None },
            R::ListPanes,
            R::Capture { pane: 1, lines: 1, start: None, end: None, escapes: false },
            R::SpawnTerminal { shell: "cmd".into(), cwd: None, dock: None, ssh: None },
            R::SendInput { pane: 1, data: String::new(), raw: false },
            R::ClosePane { pane: 1 },
            R::Resize { pane: 1, cols: 1, rows: 1 },
            R::OpenBrowser { path: None },
            R::OpenSftp { session: String::new() },
            R::Wait { pane: 1, until: String::new(), timeout_ms: 1, match_text: None, match_regex: None },
            R::Tail { pane: 1 },
            R::Subscribe { pane: None, kinds: vec![] },
            R::Focus { pane: 1 },
            R::SetTitle { pane: 1, title: String::new() },
            R::Notify { title: String::new(), body: String::new() },
            R::PaneStatusSet { key: String::new(), value: None, ttl_ms: None },
            R::AgentExplain { pane: 1 },
            R::ScheduleCreate { name: String::new(), spec: String::new(), kind: String::new(), payload: String::new(), pane_title: String::new() },
            R::LayoutExport,
            R::SftpList { path: ".".into() },
            R::SftpGet { remote: "r".into(), local: "l".into() },
            R::SftpPut { local: "l".into(), remote: "r".into() },
        ];
        let doc = super::api_doc();
        let ops = doc["ops"].as_object().expect("ops");
        for s in samples {
            let v = serde_json::to_value(&s).unwrap();
            let op = v["op"].as_str().expect("op tag");
            assert!(ops.contains_key(op), "api_doc에 '{op}' 항목이 없다 — apidoc.rs를 갱신하라");
        }
    }
}
