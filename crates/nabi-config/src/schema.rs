//! serde 타입 설정 스키마.

use crate::aiprofile::AiProfileCfg;
use crate::telegram::TelegramCfg;
use serde::{Deserialize, Serialize};

/// 기본 글꼴 크기(px) — 설정 기본값과 Ctrl+0 줌 리셋이 공유하는 단일 진실원.
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// 앱 전체 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct AppConfig {
    pub appearance: Appearance,
    pub terminal: TerminalCfg,
    pub telegram: TelegramCfg,
}

/// 외형/언어 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub font_size: f32,
    pub font_family: String,
    /// "system" | "en" | "ko" | "ja"
    pub language: String,
    /// 색 스킴: "default" | "solarized" | "nord" | "gruvbox" | "light"
    pub theme: String,
    /// 커서 깜빡임.
    pub cursor_blink: bool,
    /// 커서 깜빡임 반주기(ms).
    pub blink_ms: u64,
    /// 비주얼 벨(BEL 시 화면 깜빡임).
    pub visual_bell: bool,
    /// 시작 시 창을 항상 위에 둘지.
    pub always_on_top: bool,
    /// 하단 상태바 표시.
    pub show_statusbar: bool,
    /// 전체 UI 배율(egui pixels_per_point).
    pub ui_scale: f32,
    /// 드래그 선택 종료 시 자동 복사.
    pub copy_on_select: bool,
    /// 커서 모양: "block" | "bar" | "underline"
    pub cursor_shape: String,
    /// 커서 색(#RRGGBB; 비우면 전경색).
    pub cursor_color: String,
    /// 선택 강조 배경색(#RRGGBB; 비우면 기본).
    pub selection_color: String,
    /// 검색 일치 강조색(#RRGGBB; 비우면 기본).
    pub match_color: String,
    /// 전경색 오버라이드(#RRGGBB; 비우면 테마).
    pub fg_color: String,
    /// 배경색 오버라이드(#RRGGBB; 비우면 테마).
    pub bg_color: String,
    /// Quake 모드 전역 핫키(global-hotkey 형식, 예: "Control+Backquote").
    pub quake_hotkey: String,
    /// 왼쪽 세션 사이드바 표시(MobaXterm식).
    #[serde(default)]
    pub show_sessions_panel: bool,
    /// 상단 퀵커넥트 바 표시(FileZilla식).
    #[serde(default = "default_true")]
    pub show_quickconnect_bar: bool,
    /// 탭 제목에 pane ID(#N) 배지 표시(제어 평면 `nabi cli` 타깃 참조용).
    #[serde(default = "default_true")]
    pub show_pane_ids: bool,
    /// 상태바에 시계(HH:MM) 표시. 기본 false.
    #[serde(default)]
    pub show_clock: bool,
    /// 마지막 창 크기(종료 시 저장, 시작 시 복원. 0=기본값 사용).
    #[serde(default)]
    pub window_w: f32,
    #[serde(default)]
    pub window_h: f32,
    /// 접힌 세션 그룹 이름(사이드바에서 접기 상태 영속).
    #[serde(default)]
    pub collapsed_groups: Vec<String>,
    /// 고정(즐겨찾기)한 세션 이름 — 사이드바 최상단 "📌 고정" 그룹에 모아 보여준다(MobaXterm식).
    #[serde(default)]
    pub pinned_sessions: Vec<String>,
    /// 세션별 메모/설명(이름→메모) — 사이드바 호버 툴팁에 표시(MobaXterm 세션 노트).
    #[serde(default)]
    pub session_notes: std::collections::BTreeMap<String, String>,
}

fn default_true() -> bool { true }
fn default_sftp_charset() -> String { "auto".into() }

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            font_family: "monospace".into(),
            language: "system".into(),
            theme: "default".into(),
            cursor_blink: true,
            blink_ms: 530,
            visual_bell: true,
            always_on_top: false,
            show_statusbar: true,
            ui_scale: 1.0,
            copy_on_select: true,
            cursor_shape: "block".into(),
            cursor_color: String::new(),
            selection_color: String::new(),
            match_color: String::new(),
            fg_color: String::new(),
            bg_color: String::new(),
            quake_hotkey: "Control+Backquote".into(),
            show_sessions_panel: false,
            show_quickconnect_bar: true,
            show_pane_ids: true,
            show_clock: false,
            window_w: 0.0,
            window_h: 0.0,
            collapsed_groups: Vec::new(),
            pinned_sessions: Vec::new(),
            session_notes: std::collections::BTreeMap::new(),
        }
    }
}

/// 터미널 동작 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalCfg {
    pub scrollback: usize,
    /// 기본 셸: "pwsh" | "powershell" | "cmd" | "wsl" | "gitbash"
    pub default_shell: String,
    /// 새 로컬 터미널 기본 시작 디렉터리(비우면 포커스 셸 cwd 상속→없으면 시스템 기본).
    #[serde(default)]
    pub default_cwd: String,
    /// 기본 인코딩: "UTF-8" | "EUC-KR" | "Shift_JIS" ...
    pub encoding: String,
    /// 시작 시 마지막 워크스페이스(열린 탭) 자동 복원.
    pub restore_workspace: bool,
    /// 복원 시 종료 직전 실행 중이던 명령(claude 등)을 마지막 cwd에서 재실행.
    #[serde(default = "default_true")]
    pub restore_running_command: bool,
    /// SSH 복원 시 허용 목록 AI CLI만 고정 resume 명령으로 재실행(opt-in).
    #[serde(default)]
    pub restore_ssh_ai_command: bool,
    /// 여러 탭이 열려 있을 때 닫기 전 확인 대화상자.
    pub confirm_close: bool,
    /// 최근 Quick Connect 호스트/사용자/포트(프리필용, 비밀 아님).
    pub last_host: String,
    pub last_user: String,
    pub last_port: String,
    /// Find가 스크롤백을 거슬러 검색할 최대 줄 수.
    pub search_limit: usize,
    /// 여러 줄 클립보드 붙여넣기 전 확인 대화상자(붙여넣기 안전).
    pub warn_paste_newline: bool,
    /// 자주 쓰는 명령 스니펫(메뉴에서 클릭하면 포커스 pane에 전송+실행).
    pub snippets: Vec<String>,
    /// SFTP 전송 속도 제한(KB/s, 0=무제한).
    #[serde(default)] pub speed_limit_kbps: u32,
    /// 전송 후 SHA-256 해시 검증(rclone식). 원격에 해시 명령이 없으면 크기 비교로 폴백.
    #[serde(default)] pub sftp_verify_hash: bool,
    /// SFTP 파일명 인코딩(auto/utf8/euc-kr/shift_jis/gbk). v3 서버는 파일명을 서버 로컬
    /// 인코딩 raw 바이트로 보내므로(한국 서버 CP949 등) auto가 무손실 감지·역인코딩한다.
    #[serde(default = "default_sftp_charset")] pub sftp_name_charset: String,
    /// AI 터미널 프로필 목록(세션▸새 AI 터미널). 설정▸AI 터미널에서 편집.
    #[serde(default)] pub ai_profiles: Vec<AiProfileCfg>,
    /// AI 명령 바: AI CLI 실행 pane 상단에 슬래시 명령 버튼 줄 표시(aicmdbar.rs).
    #[serde(default = "default_true")] pub ai_cmd_bar: bool,
    /// 명령 바에서 마지막으로 고른 모델·노력 수준(재시작 후에도 버튼에 그대로 보이게).
    /// CLI가 상태줄로 실제 값을 알려주면 그쪽이 우선한다.
    #[serde(default)] pub ai_last_model: String,
    #[serde(default)] pub ai_last_effort: String,
    /// 한 원격 연결에서 동시에 진행할 전송 수(1~4). 나머지는 큐에서 대기한다.
    pub max_parallel_transfers: u32,
    /// SFTP 다운로드 기본 폴더(비우면 로컬 창/홈). 설정 시 목적지 대화상자의 시작 위치.
    #[serde(default)] pub download_dir: String,
    /// 다운로드 시 목적지를 매번 물어볼지(기본 true). false + download_dir 설정 시 묻지 않고 그 폴더로.
    #[serde(default = "default_true")]
    pub download_ask: bool,
    /// 빠른 연결 최근 호스트("user@host:port", 최신 우선).
    #[serde(default)]
    pub recent_hosts: Vec<String>,
    /// 파일 정렬 기준(0=이름,1=크기,2=날짜).
    #[serde(default)]
    pub browser_sort: u8,
    /// SFTP 보기 모드(0=자세히,1=목록,2=큰,3=작은,4=타일).
    #[serde(default)]
    pub sftp_view: u8,
    /// 원격이 로컬 클립보드에 쓰는 것(OSC 52) 허용 범위.
    /// 0=차단, 1=허용하되 알림(기본), 2=조용히 허용.
    #[serde(default = "default_osc52")]
    pub osc52_mode: u8,
    /// 숨김 파일(.) 표시.
    #[serde(default)]
    pub browser_show_hidden: bool,
    /// 출력에서 강조할 키워드 목록(로그 모니터링용 하이라이트 규칙).
    #[serde(default)]
    pub highlight_keywords: Vec<String>,
    /// 출력 트리거: 이 패턴(부분 문자열, 대소문자 무시)이 새 출력에 나타나면 알림(빌드 완료·에러 감시).
    #[serde(default)]
    pub alert_patterns: Vec<String>,
    /// SSH keepalive 간격(초). 0=끄기. 방화벽/유휴 타임아웃 대응(ServerAliveInterval). 기본 30.
    #[serde(default = "default_keepalive")]
    pub ssh_keepalive_secs: u64,
    /// 로컬 브라우저 정렬 내림차순 여부(컬럼 헤더 재클릭으로 토글).
    #[serde(default)]
    pub browser_sort_desc: bool,
    /// 로컬 브라우저 보기 모드(0=자세히,1=목록,2=큰,3=작은,4=타일).
    #[serde(default)]
    pub browser_view: u8,
    /// 에이전트 제어 평면 모드: "off" | "ask"(기본) | "on". (docs/agent-control.md)
    #[serde(default = "default_control_mode")]
    pub control_mode: String,
    /// OSC 7771 in-band 제어 허용(기본 false — 원격 출력 위장 방지). 로컬 pane만 처리.
    #[serde(default)]
    pub control_allow_osc: bool,
    /// 시작 시 자동 업데이트 확인(GitHub 릴리스). 기본 true.
    #[serde(default = "default_true")]
    pub auto_check_update: bool,
    /// "일주일 후에" 스누즈 — 이 unix초 이전에는 업데이트 확인을 건너뜀(0=없음).
    #[serde(default)]
    pub update_remind_after: i64,
    /// 셸 통합 설치 권장을 "다시 보지 않기"로 끈 경우 true.
    #[serde(default)]
    pub shellinteg_dismissed: bool,
    /// 파일 편집을 내장 에디터로(기본 true). false면 OS 기본/외부 편집기로 연다.
    #[serde(default = "default_true")]
    pub editor_builtin: bool,
    /// 볼트 마스터 비밀번호를 OS 자격증명(Windows)에 저장해 자동 잠금 해제(F1, 기본 false).
    /// 편의↑·보안↓ 절충 — 같은 OS 세션 사용자가 볼트를 자동으로 열 수 있다.
    #[serde(default)]
    pub vault_remember: bool,
    /// SSH 서버 리소스 통계 폴링 주기(초, MobaXterm식 상태바). 0=비활성. 기본 3.
    /// 리눅스(/proc)만 표시 — 별도 exec 채널로 주기적 명령을 실행한다.
    #[serde(default = "default_stats_secs")]
    pub ssh_stats_secs: u64,
    /// SSH 끊김 시 자동 재접속(안정적으로 연결됐다 끊긴 경우만; 즉시 실패는 모달). 기본 false.
    #[serde(default)]
    pub auto_reconnect: bool,
    /// SSH 리소스 경보 임계(%) — CPU/MEM/디스크가 이 값 이상이면 빨강·토스트. 기본 90.
    #[serde(default = "default_alert_pct")]
    pub ssh_stats_alert_pct: u32,
    /// 세션별 마지막 접속 시각(이름→unix초). 세션 목록에 상대시간 표시(D4).
    #[serde(default)]
    pub last_connected: std::collections::BTreeMap<String, i64>,
    /// 디렉터리 방문 기록(경로→(횟수,마지막unix초)) — zoxide식 점프(E1).
    #[serde(default)]
    pub dir_visits: std::collections::BTreeMap<String, (u32, i64)>,
    /// 명령 히스토리((명령,cwd,종료코드,unix초)) — Atuin식 재실행(E5).
    #[serde(default)]
    pub cmd_history: Vec<(String, String, i32, i64)>,
    /// SFTP 원격 경로 북마크(FileZilla식 즐겨찾기).
    #[serde(default)]
    pub sftp_bookmarks: Vec<String>,
    /// AI CLI(Claude Code·Codex)를 시작 시 자동으로 최신으로 올린다. 기본 false —
    /// 남의 프로그램을 말없이 갈아 끼우는 일이라 사용자가 켠 경우에만 한다.
    #[serde(default)]
    pub ai_cli_auto_update: bool,
    /// AI CLI 최신 버전을 마지막으로 확인한 unix초(하루 1회로 제한).
    #[serde(default)]
    pub ai_cli_checked_at: i64,
    /// 에이전트가 입력 대기(blocked)로 전이할 때 시스템 알림음(A7). 기본 false.
    #[serde(default)]
    pub agent_sound: bool,
}

fn default_alert_pct() -> u32 { 90 }
fn default_control_mode() -> String { "ask".into() }
fn default_stats_secs() -> u64 { 3 }
fn default_keepalive() -> u64 { 30 }

impl Default for TerminalCfg {
    fn default() -> Self {
        Self {
            scrollback: 5000,
            default_shell: "powershell".into(),
            default_cwd: String::new(),
            encoding: "UTF-8".into(),
            restore_workspace: true, // 재시작 시 작업 복구가 기본(스크롤백 백로그 포함).
            restore_running_command: true, // 종료 직전 실행 중이던 명령도 복원 재실행.
            restore_ssh_ai_command: false, // 원격 자동 실행은 명시적으로 켠 경우만.
            shellinteg_dismissed: false,
            editor_builtin: true,
            vault_remember: false,
            ssh_stats_secs: 3,
            auto_reconnect: false,
            ssh_stats_alert_pct: 90,
            last_connected: std::collections::BTreeMap::new(),
            dir_visits: std::collections::BTreeMap::new(),
            cmd_history: Vec::new(),
            sftp_bookmarks: Vec::new(),
            ai_cli_auto_update: false,
            ai_cli_checked_at: 0,
            agent_sound: false,
            confirm_close: true,
            last_host: String::new(),
            last_user: String::new(),
            last_port: "22".into(),
            search_limit: 5000,
            warn_paste_newline: false,
            snippets: Vec::new(),
            speed_limit_kbps: 0,
            sftp_verify_hash: false,
            sftp_name_charset: default_sftp_charset(),
            ai_profiles: Vec::new(),
            ai_cmd_bar: true,
            ai_last_model: String::new(),
            ai_last_effort: String::new(),
            max_parallel_transfers: 2,
            download_dir: String::new(),
            download_ask: true,
            recent_hosts: Vec::new(),
            browser_sort: 0,
            sftp_view: 0,
            osc52_mode: default_osc52(),
            browser_show_hidden: false,
            highlight_keywords: Vec::new(),
            alert_patterns: Vec::new(),
            ssh_keepalive_secs: default_keepalive(),
            browser_sort_desc: false,
            browser_view: 0,
            control_mode: "ask".into(),
            control_allow_osc: false,
            auto_check_update: true,
            update_remind_after: 0,
        }
    }
}

/// OSC 52 기본값 = 1(허용하되 알림). 통째로 막으면 SSH 너머 nvim/tmux yank가 죽고,
/// 조용히 허용하면 원격이 클립보드를 바꿔치기해도 사용자가 알 길이 없다.
fn default_osc52() -> u8 {
    1
}
