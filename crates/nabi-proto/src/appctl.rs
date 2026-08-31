//! 앱 레벨 제어 어휘 — 제어 평면(nabi-control)이 NabiApp에 보내는 UI 동작.
//!
//! 오케스트레이터 Command가 아니라 앱 소유 상태(브라우저/SFTP 탭, 도크)를 다룬다.

/// 제어 평면 → NabiApp 동작. `NabiApp::update()` 선두에서 drain.
#[derive(Debug, Clone)]
pub enum AppCtl {
    /// 로컬 파일 브라우저 탭 열기(경로 지정, 없으면 홈).
    OpenBrowser { path: Option<String> },
    /// 경로를 nabiPad 편집기 탭으로 연다(크기·이진 여부에 따라 알맞은 편집기가 골라진다).
    OpenEditor { path: String },
    /// 그 폴더에서 새 터미널을 열고 창을 앞으로 가져온다(탐색기 우클릭).
    OpenHere { path: String },
    /// 화면을 PNG 로 뜬다. pane 을 주면 그 자리만, 안 주면 창 전체.
    Screenshot { pane: Option<u64>, out: Option<String> },
    /// 진행률을 직접 알려 준다. `percent=None` 이면 지운다.
    ///
    /// 알려 준 pane 은 그때부터 화면을 읽지 않는다 — 말한 쪽이 우리 짐작보다 정확하다.
    Progress { pane: u64, percent: Option<u8> },
    /// 내장 **웹** 브라우저 창을 연다(없으면 우리 소개 문서).
    ///
    /// 위의 `OpenBrowser` 는 **파일** 탐색기다. 이름이 비슷해 헷갈리기 쉬워 나눠 적는다.
    OpenWeb { url: Option<String>, window: bool },
    /// 화면을 찍는다 — 앱이 `Event::ShotDone{seq, ..}` 로 **결과를 돌려준다.**
    ///
    /// 예전에는 요청을 넘기고 곧바로 "ok" 를 답했다. 그림은 다음 프레임에 그려지므로,
    /// 부른 쪽이 바로 그 파일을 읽으면 없다. 어디에 남았는지도 알 수 없었다 —
    /// 경로는 화면 토스트로만 갔다. 사람은 그걸 보지만 AI 에이전트는 못 본다.
    ShotSeq { seq: u64, pane: Option<u64>, out: Option<String> },
    /// 나비텀을 껐다 다시 켠다 — 묻지 않고, 작업 공간은 저장한다.
    Restart,
    /// 나비텀을 끝낸다 — 묻지 않고, 작업 공간은 저장한다.
    Quit,
    /// 스스로 최신판으로 올린다(check 면 확인만).
    SelfUpdate { check: bool },
    /// 저장된 SFTP 세션 이름으로 원격 탭 열기.
    OpenSftp { session: String },
    /// 다음 PaneSpawned의 도킹 위치(split-right|split-down|new-window — CP-7).
    DockNext { dock: String },
    /// 저장 세션 이름으로 SSH 터미널 연결(자격증명은 볼트 경유 — CP-7).
    ConnectSession { session: String },
    /// pane 탭 활성화(CP-7).
    Focus { pane: u64 },
    /// 탭 제목 변경(CP-7).
    SetTitle { pane: u64, title: String },
    /// 사용자 토스트 알림(발신 pane 표기 — CP-7).
    Notify {
        from: Option<u64>,
        title: String,
        body: String,
    },
    /// pane 커스텀 상태 키-값 설정/삭제(AI 도구가 모델/토큰 등 발행 → 상태바·탭). value=None=삭제, key="" =전체 삭제.
    PaneStatus {
        pane: u64,
        key: String,
        value: Option<String>,
        /// 만료(ms) — 지나면 자동 삭제(B7, herdr 메타데이터 TTL). None=영구.
        ttl_ms: Option<u64>,
    },
    /// 스케줄 잡 등록(C3): spec="*/5 * * * *"|"every 15m"|"at 09:30", kind=send|command|notify.
    ScheduleCreate { name: String, spec: String, kind: String, payload: String, pane_title: String },
    /// 웹 탭 목록 요청 — 앱이 Event::WebResult{seq,..}로 회신.
    WebList { seq: u64 },
    /// 웹 탭에서 자바스크립트 실행 — 앱이 Event::WebResult{seq,..}로 회신.
    /// 그 pane 의 전체 기록 겹 화면을 띄운다.
    ShowHistory { pane: Option<u64> },
    WebAct { seq: u64, pane: Option<u64>, act: String, arg: String },
    WebEval { seq: u64, pane: Option<u64>, js: String },
    /// 레이아웃 export 요청(B4) — 앱이 Event::LayoutJson{seq,json}으로 회신.
    LayoutExport { seq: u64 },
    /// 제어평면 SFTP 조작(S6-55): 현재 열린 SFTP 연결 대상. 앱이 Event::SftpCtlDone{seq,…}로 회신.
    SftpCtl { seq: u64, op: SftpCtlOp },
}

/// 제어평면 SFTP 조작 종류(S6-55) — 에이전트/스크립트의 원격 파일 왕복.
#[derive(Debug, Clone)]
pub enum SftpCtlOp {
    /// 저장된 세션 이름으로 SFTP 탭 열기.
    ///
    /// 예전에는 이것만 회신 없이 던지고 성공을 돌려줬다. 이름을 잘못 적으면 사람에게만
    /// 토스트가 뜨고 부른 쪽은 잘된 줄 알았다 — 에이전트는 그다음 단계로 넘어가 버린다.
    Open { session: String },
    /// 원격 디렉터리 목록(JSON 배열로 회신).
    List { path: String },
    /// 원격 → 로컬 단일 파일 다운로드(전송 큐 경유 — UI에도 보인다).
    Get { remote: String, local: String },
    /// 로컬 → 원격 단일 파일 업로드.
    Put { local: String, remote: String },
}
