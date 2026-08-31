//! 제어 프로토콜 어휘(serde JSON, line-delimited). I/O 없음 — 어휘만.

use serde::{Deserialize, Serialize};

/// 클라이언트 → 서버. 접속 후 첫 줄은 반드시 Hello(토큰 검증).
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum ControlRequest {
    /// 접속 인사 + 토큰. 요청자 pane(NABI_PANE_ID)은 감사 로그용.
    Hello { token: String, from: Option<u64> },
    /// U5: pane 목록·상태 스냅샷.
    ListPanes,
    /// U4: 화면+스크롤백 마지막 lines줄 텍스트 캡처.
    /// CP-8: start/end=절대 줄 범위(음수=끝에서 거꾸로), escapes=SGR 보존(tmux -e).
    Capture {
        pane: u64,
        lines: u32,
        #[serde(default)]
        start: Option<i64>,
        #[serde(default)]
        end: Option<i64>,
        #[serde(default)]
        escapes: bool,
    },
    /// U1: 새 터미널(CP-2). dock=tab|split-right|split-down|new-window(CP-7),
    /// ssh=저장 세션 이름(자격증명은 볼트 경유 — 평문 금지).
    SpawnTerminal {
        shell: String,
        cwd: Option<String>,
        #[serde(default)]
        dock: Option<String>,
        #[serde(default)]
        ssh: Option<String>,
    },
    /// U6: 입력 주입(CP-2). 기본은 타깃이 bracketed paste 모드면 200~/201~ 래핑
    /// (주입 방지 — WezTerm send-text 의미론, CP-5 G6). raw=true면 바이트 직주입.
    SendInput {
        pane: u64,
        data: String,
        #[serde(default)]
        raw: bool,
    },
    /// U8(CP-2).
    ClosePane { pane: u64 },
    Resize { pane: u64, cols: u16, rows: u16 },
    /// U2: 로컬 파일 브라우저 탭(CP-3).
    OpenBrowser { path: Option<String> },
    /// 파일을 nabiPad로 연다 — 에이전트가 "이 로그 좀 열어 줘"를 할 수 있게.
    OpenEditor { path: String },
    /// pane의 터미널 모드를 읽는다(읽기 전용 진단).
    PaneModes { pane: u64 },
    /// 그 폴더에서 새 터미널을 열고 **창을 앞으로** 가져온다(탐색기 우클릭).
    OpenHere { path: String },
    /// 화면을 그림으로 뜬다 — pane 을 주면 그 자리만, 안 주면 창 전체.
    ///
    /// `capture` 는 **글자**를 준다. 이건 **점**을 준다. 그림이 제대로 그려졌는지,
    /// 색이 맞는지처럼 글자로는 알 수 없는 것을 보려면 이쪽이 필요하다.
    Screenshot { pane: Option<u64>, out: Option<String> },
    /// 진행률을 직접 알려 준다(0~100, 없으면 지운다).
    ///
    /// 우리가 화면을 읽어 넘겨짚는 것보다 정확하다. 알려 준 pane 은 그때부터 화면을
    /// 읽지 않는다 — 말한 쪽이 권위다.
    Progress { pane: u64, percent: Option<u8> },
    /// 내장 **웹** 브라우저 창을 연다 — 에이전트가 "이 화면 좀 띄워 줘"를 할 수 있게.
    ///
    /// 포트 포워딩으로 끌어온 원격 웹 화면을 여는 것이 가장 잦은 쓰임이다.
    OpenWeb {
        url: Option<String>,
        /// 탭이 아니라 **별도 창**으로 띄운다. 탭 안의 웹은 메인 창에 붙어 있어서
        /// 분리 창으로 뗄 수 없다 — 떼어 놓고 보고 싶을 때 이 길을 쓴다.
        #[serde(default)]
        window: bool,
    },
    /// 나비텀을 끝낸다 — **묻지 않는다.**
    ///
    /// 창을 그냥 닫으면 탭이 여럿일 때 "정말 닫겠습니까"를 묻는다. 사람에게는 맞는
    /// 동작이지만, 자동으로 다루는 쪽에서는 그 창 하나 때문에 멈춘다(2026-08-31 요청).
    ///
    /// 작업 공간은 **저장하고** 나간다 — 강제 종료가 아니다. 다음에 켜면 그대로 돌아온다.
    Quit,
    /// pane 의 스크롤백을 옮긴다 — 사람이 휠을 굴리는 것과 같은 일이다.
    ///
    /// 읽는 것만으로는 못 보는 것이 있다. `capture` 는 저장된 글을 주지만, **화면에 무엇이
    /// 그려지는지**는 그리는 자리까지 가 봐야 안다. 스크롤해 놓고 `screenshot` 을 찍으면
    /// 사람이 보는 것을 그대로 볼 수 있다(2026-08-30 스크롤 결함을 재현하려고 만들었다).
    Scroll {
        pane: u64,
        /// 옮길 줄 수. +면 과거로, -면 최신으로.
        #[serde(default)]
        lines: i32,
        /// `"top"` 이면 맨 위, `"bottom"` 이면 맨 아래. 비면 `lines` 를 쓴다.
        #[serde(default)]
        to: String,
    },
    /// 스스로 최신판으로 올린다 — 사람이 단추를 누르지 않아도 된다.
    ///
    /// 설치는 조용히(`/SILENT`) 진행되고, 끝나면 인스톨러가 나비텀을 다시 켠다.
    /// 내려받은 파일은 릴리스가 공지한 SHA-256 과 대조한 뒤에만 실행한다 — 검증
    /// 없이 실행하는 길은 만들지 않는다.
    SelfUpdate {
        /// 확인만 하고 설치하지는 않는다.
        #[serde(default)]
        check: bool,
    },
    /// 열려 있는 웹 탭 목록(번호·주소·제목).
    ///
    /// `web` 이 만든 탭을 나중에 지목하려면 번호를 알아야 한다. 웹 탭은 오케스트레이터
    /// pane 이 아니라 `list` 에 나오지 않는다 — 그래서 따로 묻는 길이 필요하다.
    WebList,
    /// 웹 탭 안에서 자바스크립트를 실행하고 결과(JSON)를 받는다.
    ///
    /// pane 안의 AI 가 웹을 읽고 만지는 길이다. 페이지에 코드를 넣는 일이라 **주입 등급**이다.
    /// 그 pane 의 **전체 기록**을 화면에 띄운다(휠을 올렸을 때와 같은 겹 화면).
    ///
    /// 화면을 덮어 그리는 프로그램은 스크롤백에 조각만 남긴다. 기록에는 다 있으므로,
    /// 사람에게 지나간 것을 보여 줘야 할 때 이 길로 띄운다.
    ShowHistory { pane: Option<u64> },
    /// 웹 탭을 조종한다 — 자바스크립트로는 안 되는 것들.
    ///
    /// 동사를 아홉 개로 쪼개는 대신 `act` 한 칸에 담았다. 프로토콜이 늘어날수록 양쪽을
    /// 맞춰야 할 자리가 늘어나는데, 이것들은 전부 "이 탭에 이렇게 하라"는 같은 모양이다.
    /// CLI 에서는 `web-back` 처럼 낱말로 갈라 보인다 — 부르는 사람에게는 그편이 낫다.
    WebAct {
        pane: Option<u64>,
        /// back|forward|reload|stop|goto|zoom|shot|pdf|text
        act: String,
        /// goto 면 주소, zoom 이면 배율, shot·pdf 면 저장할 자리. 나머지는 빈 글.
        #[serde(default)]
        arg: String,
    },
    WebEval {
        /// 어느 웹 탭인가. 없으면 열려 있는 것이 하나일 때만 그것을 쓴다.
        pane: Option<u64>,
        js: String,
    },
    /// U3: 저장 SFTP 세션을 새 탭으로 연결(CP-3). 자격증명은 볼트에서.
    OpenSftp { session: String },
    /// U7: 조건 충족까지 블록(스트림 — CP-4). until: exit|command-done|idle|output.
    Wait {
        pane: u64,
        until: String,
        timeout_ms: u64,
        /// until=output일 때: 이 부분 문자열이 화면에 나타날 때까지(B2).
        #[serde(default)]
        match_text: Option<String>,
        /// until=output일 때: 이 정규식이 화면에 매치될 때까지(B2).
        #[serde(default)]
        match_regex: Option<String>,
    },
    /// U7 변형: pane 출력 실시간 스트림(CP-4).
    Tail { pane: u64 },
    /// B3: 이벤트 구독 스트림. kinds 비면 전부, pane 없으면 전 pane.
    /// kind 어휘: spawned|exit|output|command-done|agent-status|cwd.
    Subscribe {
        #[serde(default)]
        pane: Option<u64>,
        #[serde(default)]
        kinds: Vec<String>,
    },
    /// pane 탭 활성화(에이전트가 사용자 주의 유도 — CP-7).
    Focus { pane: u64 },
    /// 탭 제목 변경(CP-7).
    SetTitle { pane: u64, title: String },
    /// 사용자 토스트 알림(발신 pane 표기 — CP-7).
    Notify { title: String, body: String },
    /// 호출 pane의 커스텀 상태 키-값 설정/삭제(AI 도구 → 상태바·탭). value=None=삭제, key="" =전체 삭제.
    PaneStatusSet {
        key: String,
        value: Option<String>,
        /// 만료(ms) — 지나면 자동 삭제(B7). None=영구.
        #[serde(default)]
        ttl_ms: Option<u64>,
    },
    /// A4: pane 화면을 내장 감지 규칙 전부로 평가해 판정 근거를 회신(디버깅·규칙 작성용).
    AgentExplain { pane: u64 },
    /// C3: 스케줄 잡 등록(사양 검증은 앱에서 — 실패 시 토스트).
    ScheduleCreate { name: String, spec: String, kind: String, payload: String, pane_title: String },
    /// B4: 현재 탭·분할 레이아웃을 JSON으로 회신(apply가 소비하는 panes 목록 + 정확한 tree).
    LayoutExport,
    /// S6-55: 열린 SFTP 연결의 원격 디렉터리 목록(JSON 배열 회신).
    SftpList { path: String },
    /// S6-55: 원격 → 로컬 단일 파일 다운로드(전송 큐 경유, 완료까지 대기).
    SftpGet { remote: String, local: String },
    /// S6-55: 로컬 → 원격 단일 파일 업로드(완료까지 대기).
    SftpPut { local: String, remote: String },
}

/// 서버 → 클라이언트 응답(요청당 한 줄).
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "res", rename_all = "kebab-case")]
pub enum ControlResponse {
    Ok,
    Panes { panes: Vec<PaneInfo> },
    /// 스크롤 뒤의 자리 — 부른 쪽이 끝에 닿았는지 알아야 멈출 때를 안다.
    Scrolled { offset: u64, history: u64 },
    Captured {
        pane: u64,
        text: String,
        row: u16,
        col: u16,
        /// 대체 화면(vim 등 전체화면 앱) 여부 — 소비자가 줄 스트림 의미를 판별.
        #[serde(default)]
        alt_screen: bool,
    },
    /// 새 pane ID 회신(spawn 후 후속 send/capture 타깃).
    Spawned { pane: u64 },
    /// Wait/Tail 스트림 항목.
    Event { pane: u64, kind: String, data: String },
    /// pane의 터미널 모드 스냅샷(진단) — 휠·붙여넣기·키 동작을 좌우하는 보이지 않는 상태.
    Modes {
        pane: u64,
        alt_screen: bool,
        mouse_on: bool,
        alt_scroll: bool,
        bracketed_paste: bool,
        app_cursor: bool,
        kitty_keys: u8,
        scrollback_lines: usize,
        scroll_offset: usize,
    },
    Err { message: String },
}

/// pane 스냅샷 항목. CP-6 확장 필드는 serde 기본값으로 하위호환.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PaneInfo {
    pub id: u64,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
    /// "local" | "ssh".
    #[serde(default)]
    pub kind: String,
    /// 최신 작업 디렉터리(OSC 7).
    #[serde(default)]
    pub cwd: Option<String>,
    /// 활동 상태: working | blocked | idle (휴리스틱 — state_ms로 재판정 가능).
    #[serde(default)]
    pub state: String,
    /// 현 상태 추정 경과(ms).
    #[serde(default)]
    pub state_ms: u64,
    /// 마지막 명령 종료 코드(OSC 133;D).
    #[serde(default)]
    pub last_exit: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrips_json() {
        let r = ControlRequest::Capture {
            pane: 3,
            lines: 50,
            start: None,
            end: None,
            escapes: false,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"op\":\"capture\""));
        assert!(matches!(
            serde_json::from_str::<ControlRequest>(&s).unwrap(),
            ControlRequest::Capture { pane: 3, lines: 50, .. }
        ));
        let h: ControlRequest =
            serde_json::from_str(r#"{"op":"hello","token":"t","from":null}"#).unwrap();
        assert!(matches!(h, ControlRequest::Hello { .. }));
    }
}
