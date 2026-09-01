//! 원격 SFTP 브라우저 패널 — 오케스트레이터 SFTP 액터의 UI(목록/폴더 네비).

use crate::app::NabiApp;
use crate::sftpxfer::Transfer;
use nabi_i18n::tr;
use nabi_proto::{Command, SftpEntry, SftpId, SshParams};

/// SFTP 탭의 재연결 출처(비밀 없음) — start_sftp → SftpPanel → workspace .stabs.
#[derive(Default)]
pub(crate) struct SftpOrigin {
    pub cred_ref: Option<String>,
    pub key_path: Option<String>,
    pub jump: Option<String>,
}

#[derive(Default)]
pub(crate) struct SftpPanel {
    pub open: bool,
    pub id: Option<SftpId>,
    /// 접속 호스트(패널 제목 표시용).
    pub host: String,
    /// 재연결·"여기서 터미널 열기"용 접속 정보(비밀 아님 — 비밀번호/키는 저장 안 함).
    pub conn_host: String,
    pub conn_user: String,
    pub conn_port: String,
    /// 워크스페이스 복원용 출처(비밀 아님): 볼트 키 참조·키 파일 경로·점프 호스트·FTP 여부.
    /// 비밀번호 자체는 절대 담지 않는다 — 볼트 키로만 참조한다.
    pub cred_ref: Option<String>,
    pub key_path: Option<String>,
    pub jump: Option<String>,
    pub is_ftp: bool,
    /// 재시작 복원 시 돌아갈 원격 경로(연결 완료 이벤트에서 1회 소비). None이면 서버 기본.
    pub restore_path: Option<String>,
    /// 원격 파일시스템 여유 공간(모르면 None — statvfs 미지원 서버).
    pub free_space: Option<u64>,
    pub path: String,
    pub entries: Vec<SftpEntry>,
    pub status: String,
    /// 전송 큐(다운로드/업로드 진행·완료). 액터가 FIFO로 처리.
    pub transfers: Vec<Transfer>,
    /// 업로드가 성공해 원격 목록이 낡았다는 표시. 큐가 다 빈 뒤 한 번만 새로 고친다.
    pub dir_stale: bool,
    /// 이름 변경 중인 항목(Some이면 인라인 편집기 표시).
    pub rename_from: Option<String>,
    /// 이름변경 시작 직후 1프레임 편집기 포커스.
    pub rename_focus: bool,
    /// 파일 목록 글꼴 크기(Ctrl+휠 — 이 SFTP 패널만). 0이면 기본값으로 지연 초기화.
    pub font_size: f32,
    /// 새 폴더/파일 입력 모드.
    pub mkdir_mode: bool,
    /// mkdir_mode 확정 시 폴더(false)/빈 파일(true).
    pub newfile_mode: bool,
    /// 삭제 확인 대기 중인 항목(Some이면 확인 행 표시).
    pub pending_delete: Option<String>,
    /// 이름 변경/새 폴더 입력 버퍼.
    pub input: String,
    /// 원격 항목 이름 필터(비면 전체 표시).
    pub filter: String,
    /// 디렉터리 비교용 로컬 맵(name→(is_dir,size)). Some이면 비교 색칠.
    pub compare: Option<std::collections::HashMap<String, (bool, u64)>>,
    /// 재귀 검색 결과 경로(비면 일반 목록 표시).
    pub search_results: Vec<String>,
    /// 숨김 파일(.으로 시작) 표시 여부(기본 false=숨김, ls 관례).
    pub show_hidden: bool,
    /// 권한(rwxr-xr-x) 열을 함께 보여 줄지. 설정(`browser_extra_col`)에서 흘러온다
    /// (`persist_view_prefs`) — 여기서 바꾸지 말 것. 로컬 탐색기의 속성 열과 같은 스위치다.
    pub extra_col: bool,
    /// 항목 보기 모드(자세히/목록/아이콘/타일).
    pub view_mode: crate::sftpview::ViewMode,
    /// 단일 클릭으로 선택된 항목(하이라이트·범위 기준점). 폴더 진입 시 해제.
    pub selected: Option<String>,
    /// 다중 선택 집합(Ctrl/Shift+클릭).
    pub multi: std::collections::HashSet<String>,
    /// 키보드 이동 시 선택 항목으로 스크롤 요청(1프레임).
    pub scroll: bool,
    /// 뒤로/앞으로 경로 히스토리(마우스 엄지 버튼).
    pub hist_back: Vec<String>,
    pub hist_fwd: Vec<String>,
    /// 일괄 이름변경 모드 + find/replace 버퍼.
    pub batch_mode: bool,
    pub batch_find: String,
    pub batch_replace: String,
    /// 편집 가능한 경로 주소창 버퍼(비편집 시 현재 경로 표시, Enter로 이동 — FileZilla식).
    pub addr: String,
    /// 동기화 미리보기: (업로드 여부, 전송할 파일명들). Some이면 적용 전 확인 행 표시.
    pub sync_pending: Option<(bool, Vec<String>)>,
}

impl NabiApp {
    /// Quick Connect 입력값으로 원격 연결을 연다(ftp=true면 FTP, 아니면 SFTP).
    pub(crate) fn open_remote_from_qc(&mut self, ftp: bool) {
        self.normalize_qc(); // host 칸의 "user@host:port" 분리.
        let host = self.quick_connect.host.trim().to_string();
        if host.is_empty() {
            return;
        }
        let port = self.quick_connect.port.trim().parse().unwrap_or(22);
        let user = self.quick_connect.user.trim().to_string();
        let pw = self.quick_connect.password.clone();
        let key = self.quick_connect.key_path.trim().to_string();
        let params = crate::sshauth::params_for(host, port, user, pw, &key, ftp);
        // 키 파일로 붙었으면 그 경로만 출처로 남긴다(비밀번호는 저장하지 않으므로 복원 시 재입력).
        let org = SftpOrigin { key_path: (!key.is_empty()).then(|| key.clone()), ..Default::default() };
        self.start_sftp(params, ftp, org);
    }

    /// SFTP/FTP 연결을 시작하고 새 도킹 탭으로 연다(매번 고유 PaneId — 멀티 탭).
    pub(crate) fn start_sftp(&mut self, params: SshParams, ftp: bool, org: SftpOrigin) {
        // 현재 활성 원격 패널이 열려 있으면 배경으로 보관(다른 탭으로 유지).
        if self.sftp.open {
            if let Some(old) = self.sftp_pane {
                self.sftp_bg.insert(old, std::mem::take(&mut self.sftp));
            }
        }
        self.sftp_seq += 1;
        let id = self.sftp_seq;
        let pane = nabi_types::PaneId::new(u64::MAX - id); // 실제 pane과 안 겹치는 고유 id.
        let proto = if ftp { "FTP" } else { "SFTP" };
        let host = if params.user.is_empty() {
            format!("{proto} {}", params.host)
        } else {
            format!("{proto} {}@{}", params.user, params.host)
        };
        self.sftp = SftpPanel {
            open: true,
            id: Some(id),
            host,
            conn_host: params.host.clone(),
            conn_user: params.user.clone(),
            conn_port: params.port.to_string(),
            cred_ref: org.cred_ref,
            key_path: org.key_path,
            jump: org.jump,
            is_ftp: ftp,
            status: tr(self.lang, "sftp.connecting").to_string(),
            view_mode: crate::sftpview::ViewMode::from_u8(self.config.terminal.sftp_view),
            ..Default::default()
        };
        self.orch.send(Command::SftpConnect {
            id,
            params,
            ftp,
            limit_kbps: self.config.terminal.speed_limit_kbps,
            parallel: self.config.terminal.max_parallel_transfers,
        });
        self.add_pane(pane);
        if let Some(loc) = self.dock.find_tab(&pane) {
            let _ = self.dock.set_active_tab(loc);
        }
        self.sftp_pane = Some(pane);
    }
}

