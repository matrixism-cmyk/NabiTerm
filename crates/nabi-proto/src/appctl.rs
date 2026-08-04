//! 앱 레벨 제어 어휘 — 제어 평면(nabi-control)이 NabiApp에 보내는 UI 동작.
//!
//! 오케스트레이터 Command가 아니라 앱 소유 상태(브라우저/SFTP 탭, 도크)를 다룬다.

/// 제어 평면 → NabiApp 동작. `NabiApp::update()` 선두에서 drain.
#[derive(Debug, Clone)]
pub enum AppCtl {
    /// 로컬 파일 브라우저 탭 열기(경로 지정, 없으면 홈).
    OpenBrowser { path: Option<String> },
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
    },
}
