//! 오케스트레이터 → UI 통지 어휘.

use crate::pane_msg::CommandBlock;
use crate::sftp::{SftpEntry, SftpId};
use nabi_types::PaneId;

/// 오케스트레이터가 UI로 보내는 모든 통지.
#[derive(Debug, Clone)]
pub enum Event {
    /// pane이 생성됨. seq는 SpawnLocalPane.reply_seq 에코(제어 평면 상관용).
    PaneSpawned { pane: PaneId, seq: Option<u64> },
    /// pane 출력 발생(damage) — UI는 해당 뷰포트만 repaint.
    PaneOutput { pane: PaneId },
    /// pane 종료(자식 프로세스 종료 코드).
    PaneExited { pane: PaneId, code: Option<i32> },
    /// SSH 연결이 오류로 끊어짐(pane은 유지 — UI가 재연결 제안).
    SshDisconnected { pane: PaneId, message: String },
    /// 원격 SSH 서버 리소스 통계(주기 폴링) — 상태바 CPU/MEM/부하/가동시간 표시.
    ServerStats { pane: PaneId, stats: Box<crate::stats::ServerStats> }, // Box: enum 변형 크기 축소(채널 효율).
    /// 미지 SSH 호스트키 — 사용자 신뢰 확인 요청(UI 모달). id로 결정을 회신.
    HostKeyPrompt {
        id: u64,
        host: String,
        port: u16,
        algorithm: String,
        fingerprint: String,
    },
    /// 명령 실행 시작(OSC 133;C) — 타이밍 측정 시작.
    CommandStarted { pane: PaneId },
    /// OSC 133/sentinel로 검출한 명령 블록.
    CommandBlock { pane: PaneId, block: CommandBlock },
    /// 셸 작업 디렉터리 변경(OSC 7).
    CwdChanged { pane: PaneId, path: String },
    /// 사용자가 방금 실행한 명령줄(OSC 633;E, 셸 통합 v2). 워크스페이스 복원 재실행용.
    CommandLine { pane: PaneId, cmd: String },
    /// 터미널이 클립보드 복사를 요청함(OSC 52).
    ClipboardCopy { text: String },
    /// 터미널 데스크톱 알림(OSC 9).
    Notify { pane: PaneId, text: String },
    /// 작업 진행률(OSC 9;4). percent=None은 제거/불확정.
    Progress { pane: PaneId, percent: Option<u8> },
    /// OSC 7771 제어 동작(in-band). 앱이 opt-in·로컬 pane 한정으로 처리.
    ControlOsc { pane: PaneId, verb: String, json: String },
    /// 에이전트 상태 변화(idle|working|blocked|done) — 앱이 합성 발행(훅 권위 또는 화면
    /// 감지 확정 시). 제어 평면 `agent wait`/이벤트 구독이 이걸 본다(B1/B3).
    AgentStatus { pane: PaneId, state: &'static str },
    /// 레이아웃 export 회신(B4) — 제어 평면 요청 seq에 JSON을 되돌려 준다.
    LayoutJson { seq: u64, json: String },
    /// 제어평면 SFTP 조작 회신(S6-55): ok=성공, data=목록 JSON 또는 사유.
    SftpCtlDone { seq: u64, ok: bool, data: String },
    /// 설정 리로드 완료.
    ConfigReloaded,
    /// SFTP 연결 성공.
    SftpConnected { id: SftpId },
    /// 원격 디렉터리 목록 도착.
    SftpListing {
        id: SftpId,
        path: String,
        entries: Vec<SftpEntry>,
    },
    /// SFTP 오류.
    SftpError { id: SftpId, message: String },
    /// 원격 재귀 검색 결과(매칭 경로들).
    SftpSearchResults { id: SftpId, results: Vec<String> },
    /// 원격 디렉터리 집계 결과(경로, 파일 수, 폴더 수, 총 바이트).
    SftpDirSize { id: SftpId, path: String, files: u64, dirs: u64, bytes: u64 },
    /// 미리보기 결과. `more`는 뒤에 내용이 더 있는지. 실패는 `err`에 담는다 —
    /// 조용히 빈 화면을 보여 주면 "빈 파일"과 구분이 안 된다.
    SftpPreview { id: SftpId, path: String, data: Vec<u8>, more: bool, err: Option<String> },
    /// 동기화 계획용 원격 파일 트리(상대경로, 크기, mtime) — SftpListTree(seq) 회신.
    SftpTree { id: SftpId, seq: u64, files: Vec<(String, u64, u64)> },
    /// 전송 진행(누적 바이트). `xfer`는 명령에 실어 보낸 큐 항목 식별자.
    SftpProgress { id: SftpId, xfer: u64, bytes: u64 },
    /// 파일 전송(다운로드/업로드) 완료. `xfer`로 큐 항목을 정확히 지목한다.
    SftpTransferDone {
        id: SftpId,
        xfer: u64,
        name: String,
        ok: bool,
        message: String,
    },
    /// 파일 작업(삭제·이름변경·폴더생성·권한 등) 완료 — **전송이 아니다**.
    ///
    /// 예전에는 이것도 `SftpTransferDone`으로 보내서, 진행 중인 전송 큐 항목이 엉뚱하게
    /// 완료 처리됐다(2GB 다운로드 중 파일 하나 삭제하면 다운로드가 "끝난 것"이 됨).
    SftpOpDone {
        id: SftpId,
        name: String,
        ok: bool,
        message: String,
    },
    /// 포트 포워딩 시작됨(id + 사람이 읽는 설명 메시지).
    ForwardStarted { id: u64, message: String },
    /// 원격이 파일 전송(trzsz)을 요청했다 — 사용자에게 물어야 한다.
    TrzszAsk { pane: PaneId, mode: crate::trzsz::XferMode },
    /// 전송 진행.
    TrzszProgress { pane: PaneId, progress: crate::trzsz::XferProgress },
    /// 전송 종료(성공이든 실패든). `names`는 실제 저장된 이름들.
    TrzszDone { pane: PaneId, ok: bool, message: String, names: Vec<String> },
    /// 일반 오류 통지.
    Error { message: String },
    /// 로컬 셸 스폰 실패(seq는 SpawnLocalPane.reply_seq 에코 — 제어 평면이 빠른 에러 회신).
    SpawnFailed { seq: Option<u64>, message: String },
}
