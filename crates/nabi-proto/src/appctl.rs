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
    /// 내장 **웹** 브라우저 창을 연다(없으면 우리 소개 문서).
    ///
    /// 위의 `OpenBrowser` 는 **파일** 탐색기다. 이름이 비슷해 헷갈리기 쉬워 나눠 적는다.
    OpenWeb { url: Option<String> },
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
    /// 레이아웃 export 요청(B4) — 앱이 Event::LayoutJson{seq,json}으로 회신.
    LayoutExport { seq: u64 },
    /// 제어평면 SFTP 조작(S6-55): 현재 열린 SFTP 연결 대상. 앱이 Event::SftpCtlDone{seq,…}로 회신.
    SftpCtl { seq: u64, op: SftpCtlOp },
}

/// 제어평면 SFTP 조작 종류(S6-55) — 에이전트/스크립트의 원격 파일 왕복.
#[derive(Debug, Clone)]
pub enum SftpCtlOp {
    /// 원격 디렉터리 목록(JSON 배열로 회신).
    List { path: String },
    /// 원격 → 로컬 단일 파일 다운로드(전송 큐 경유 — UI에도 보인다).
    Get { remote: String, local: String },
    /// 로컬 → 원격 단일 파일 업로드.
    Put { local: String, remote: String },
}
