//! UI → 오케스트레이터 요청 어휘.

use crate::sftp::SftpId;
use crate::shell::ShellKind;
use crate::ssh::SshParams;
use bytes::Bytes;
use nabi_types::{GridSize, PaneId};

/// UI(메뉴/팔레트/패널/키보드)가 방출하는 모든 요청.
#[derive(Debug, Clone)]
pub enum Command {
    /// 로컬 ConPTY 셸을 새 pane으로 스폰.
    SpawnLocalPane {
        shell: ShellKind,
        size: GridSize,
        scrollback: usize,
        encoding: String,
        /// 시작 작업 디렉터리(없으면 기본). 새 탭/분할이 현재 위치에서 열리게.
        cwd: Option<String>,
        /// 요청자 상관 ID — PaneSpawned에 에코돼 제어 평면이 자기 스폰의
        /// 정확한 pane ID를 식별한다(폴링 레이스 제거, CP-5 G1).
        reply_seq: Option<u64>,
    },
    /// SSH 원격 pane 연결.
    ConnectSsh {
        params: SshParams,
        size: GridSize,
        scrollback: usize,
        encoding: String,
        /// 서버 리소스 통계 폴링 주기(초). 0=비활성.
        stats_secs: u64,
        /// 스폰 상관 seq — PaneSpawned.seq로 에코되어 앱이 출처/명령/레이아웃을 정확히 매핑(복원 순서 무관).
        reply_seq: Option<u64>,
    },
    /// 포커스 pane(또는 지정 pane)에 입력 바이트 주입.
    WriteInput { pane: PaneId, data: Bytes },
    /// trzsz 전송 요청에 대한 사용자의 결정(수락/거절 + 경로).
    TrzszDecide(crate::trzsz::XferDecision),
    /// 진행 중인 trzsz 전송을 취소한다.
    TrzszCancel { pane: PaneId },
    /// pane 리사이즈(ConPTY resize / SSH window_change).
    Resize { pane: PaneId, size: GridSize },
    /// pane 닫기(세션 종료).
    ClosePane { pane: PaneId },
    /// 지정한 pane들에 같은 입력을 보낸다(synchronize-panes).
    ///
    /// 대상을 **명시 목록**으로 받는다 — 이전에는 `WindowId`를 실었지만 오케스트레이터가 이를
    /// 무시하고 프로세스의 모든 pane에 썼다(분리 창의 다른 SSH 세션까지). UI가 테두리로 표시하는
    /// 대상과 실제 수신자가 어긋나지 않도록, 보내는 쪽이 대상을 정한다.
    Broadcast { panes: Vec<PaneId>, data: Bytes },
    /// pane의 문자 인코딩 변경(예: "UTF-8", "EUC-KR").
    SetEncoding { pane: PaneId, label: String },
    /// 설정 핫리로드 적용.
    ReloadConfig,
    /// SFTP/FTP 연결 수립(원격 파일 브라우저용). ftp=true면 FTP.
    SftpConnect {
        id: SftpId,
        params: SshParams,
        ftp: bool,
        /// 전송 속도 제한(KB/s, 0=무제한).
        limit_kbps: u32,
        /// 동시 전송 개수(2 이상이면 전송마다 별도 연결의 워커를 쓴다).
        parallel: u32,
    },
    /// 원격 디렉터리 목록 요청.
    SftpList { id: SftpId, path: String },
    /// 동기화 계획용 원격 파일 트리 수집(S6-51). 회신=Event::SftpTree{seq}.
    SftpListTree { id: SftpId, root: String, seq: u64 },
    /// 원격 파일을 로컬 경로로 내려받기. resume>0이면 그 오프셋부터 이어받기.
    ///
    /// `xfer`는 UI 전송 큐 항목 식별자. 진행률·완료 이벤트가 이 값을 그대로 돌려주므로
    /// 큐 위치가 아니라 **항목 자체**에 귀속된다(재시도로 순서가 바뀌어도 안전).
    SftpDownload {
        id: SftpId,
        xfer: u64,
        remote: String,
        local: String,
        resume: u64,
    },
    /// 로컬 파일을 원격 경로로 올리기.
    SftpUpload {
        id: SftpId,
        xfer: u64,
        local: String,
        remote: String,
    },
    /// 원격 파일/디렉터리 삭제.
    SftpRemove { id: SftpId, path: String },
    /// 원격 파일/디렉터리 이름 변경.
    SftpRename {
        id: SftpId,
        from: String,
        to: String,
    },
    /// 원격 디렉터리 생성.
    SftpMkdir { id: SftpId, path: String },
    /// 원격 빈 파일 생성(touch).
    SftpTouch { id: SftpId, path: String },
    /// 진행 중인 전송 취소.
    SftpCancel { id: SftpId },
    /// 전송 하나만 취소(큐의 그 줄 ✕). 같은 연결의 다른 전송은 계속 간다.
    SftpCancelXfer { id: SftpId, xfer: u64 },
    /// 원격 파일 권한 변경(POSIX mode).
    SftpChmod {
        id: SftpId,
        path: String,
        mode: u32,
    },
    /// 원격 재귀 검색(root 아래 이름에 needle 포함).
    SftpSearch {
        id: SftpId,
        root: String,
        needle: String,
    },
    /// 원격 파일 **앞부분만** 읽어 온다(미리보기). 크기를 묻지 않고 max까지만 읽는다.
    SftpPreview {
        id: SftpId,
        path: String,
        max: usize,
    },
    /// 원격 디렉터리 총 크기 재귀 계산.
    SftpDirSize {
        id: SftpId,
        path: String,
    },
    /// 권한을 재귀 적용(디렉터리 하위 전부).
    SftpChmodRecursive {
        id: SftpId,
        path: String,
        mode: u32,
    },
    /// 원격 디렉터리를 로컬로 재귀 다운로드.
    SftpDownloadDir {
        id: SftpId,
        xfer: u64,
        remote: String,
        local: String,
    },
    /// 동기 재귀 다운로드(드래그-아웃 가상 폴더용) — 완료 시 done으로 결과 회신.
    SftpDownloadDirSync {
        id: SftpId,
        remote: String,
        local: String,
        done: std::sync::mpsc::Sender<bool>,
    },
    /// 로컬 디렉터리를 원격으로 재귀 업로드.
    SftpUploadDir {
        id: SftpId,
        xfer: u64,
        local: String,
        remote: String,
    },
    /// SFTP 연결 종료.
    SftpClose { id: SftpId },
    /// 로컬 포트 포워딩(-L): 임시 로컬 포트 → params로 SSH 후 remote_host:remote_port.
    StartLocalForward {
        id: u64,
        params: SshParams,
        remote_host: String,
        remote_port: u16,
    },
    /// 동적 포워딩(-D): 임시 로컬 SOCKS5 프록시 → params로 SSH direct-tcpip.
    StartDynamicForward { id: u64, params: SshParams },
    /// X11 포워딩(-X): 서버가 여는 x11 채널을 로컬 X 서버(127.0.0.1:6000)로 릴레이.
    StartX11Forward { id: u64, params: SshParams },
    /// 원격 포워딩(-R): 서버 remote_port → 로컬 127.0.0.1:remote_port.
    StartRemoteForward {
        id: u64,
        params: SshParams,
        remote_port: u16,
    },
    /// 실행 중인 포워딩 터널을 중지(태스크 abort).
    StopForward { id: u64 },
    /// 호스트키 신뢰 모달의 사용자 결정(accept=신뢰·known_hosts 학습, false=거부).
    HostKeyDecision { id: u64, accept: bool },
    /// 종료(비밀 zeroize 후).
    Shutdown,
}
