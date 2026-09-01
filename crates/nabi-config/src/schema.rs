//! serde 타입 설정 스키마.

use crate::aiprofile::AiProfileCfg;
use crate::telegram::TelegramCfg;
use serde::{Deserialize, Serialize};

/// 기본 글꼴 크기(px) — 설정 기본값과 Ctrl+0 줌 리셋이 공유하는 단일 진실원.
///
/// 터미널 본문과 새 편집기 문서에만 쓰인다(메뉴·버튼 같은 UI 크롬은 egui 기본값).
/// 16px은 **Windows Terminal 기본값(Cascadia Mono 12pt = 96dpi에서 16px)에 맞춘 값**이다.
/// 예전 기본 14px은 그보다 한 단계 작아 "처음 켰을 때 글씨가 작다"는 인상을 줬다
/// (사용자 보고 2026-08-21). 이미 설정 파일이 있는 사용자는 영향받지 않는다 — 새 설치만 바뀐다.
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

/// 글꼴 크기가 쓸 수 있는 범위.
///
/// 파일을 손으로 고치거나 설정이 깨져 0 이 들어오면 **글자가 아예 안 보인다.** 그러면
/// 설정 화면을 열어 되돌릴 수도 없다 — 화면을 못 읽으니까. 그래서 읽을 때 다듬는다.
pub const FONT_MIN: f32 = 6.0;
pub const FONT_MAX: f32 = 40.0;
/// UI 배율이 쓸 수 있는 범위. 0 이면 창이 사라지고, 크면 단추 하나가 화면을 덮는다.
pub const SCALE_MIN: f32 = 0.5;
pub const SCALE_MAX: f32 = 3.0;

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
    /// ANSI 16색 팔레트 이름(`standard`·`deuteranopia`·`highcontrast`).
    ///
    /// 기본은 `standard` — 지금 쓰는 사람의 화면을 바꾸지 않는다.
    #[serde(default)]
    pub palette: String,
    /// 색 말고 **기호로도** 알린다(전송 성공/실패 등).
    #[serde(default)]
    pub symbol_cues: bool,
    pub cursor_blink: bool,
    /// 커서 깜빡임 반주기(ms).
    pub blink_ms: u64,
    /// 비주얼 벨(BEL 시 화면 깜빡임).
    pub visual_bell: bool,
    /// 시작 시 창을 항상 위에 둘지.
    pub always_on_top: bool,
    /// 하단 상태바 표시.
    pub show_statusbar: bool,
    /// 시작할 때 스플래시 화면을 잠깐 보여 줄지(아무 키·클릭으로 즉시 넘어간다).
    #[serde(default = "default_true")]
    pub splash: bool,
    /// 마지막으로 실행한 프로그램 판. 지금 판과 다르면 '새로워진 점'을 보여 준다.
    #[serde(default)]
    pub last_run_version: String,
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
    /// 마지막 창 위치(화면 좌표). 저장된 자리가 지금 화면 밖이면 무시한다(winpos).
    #[serde(default)]
    pub window_x: f32,
    #[serde(default)]
    pub window_y: f32,
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
            palette: String::new(),
            symbol_cues: false,
            cursor_blink: true,
            blink_ms: 530,
            visual_bell: true,
            always_on_top: false,
            show_statusbar: true,
            splash: true,
            last_run_version: String::new(),
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
            window_x: 0.0,
            window_y: 0.0,
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
    /// 붙여넣기에 유니코드 속임(방향 재정의·제로폭·호모글리프)이 섞이면 확인 대화상자.
    /// 개행 경고와 별개 스위치 — 위험의 성격이 다르고, 오탐이 거의 없어 기본 켜짐.
    #[serde(default = "default_true")] pub warn_paste_unicode: bool,
    /// 업로드한 파일의 Unix 권한 정규화(빈 값=끄기, "auto"=일반 644·스크립트 755, "755" 같은 8진수).
    /// Windows 로컬 파일에는 Unix 모드가 없어 '보존'이 불가능하다 — 그래서 정규화로 푼다.
    #[serde(default)] pub sftp_upload_mode: String,
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
    /// 영문 팁 한글 오버레이(사전 기반) — 터미널의 `Tip:`/`Note:` 줄 위에 번역을 덧그린다.
    #[serde(default = "default_true")] pub tip_overlay: bool,
    /// 사전에 없는 팁을 AI(claude -p)로 번역(기본 꺼짐 — 요금·프라이버시·폐쇄망 고려).
    #[serde(default)] pub tip_translate_ai: bool,
    /// 팁 번역 캐시 파일 경로(비우면 설정 폴더). 공유 폴더·개발 서버 경로를 지정하면
    /// 여러 PC의 번역이 한 파일에 누적된다(저장 시 병합).
    #[serde(default)] pub tip_cache_path: String,
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
    /// **자동 응답을 켤 것인가.** 기본은 꺼짐 — 원격에 우리가 대신 글자를 보내는 일이라
    /// 규칙이 있다고 저절로 동작하게 두지 않는다(autoreply 문서 참고).
    #[serde(default)]
    pub auto_reply: bool,
    /// 운영 표식 세션에서 되돌릴 수 없는 명령을 보내기 전에 확인한다.
    ///
    /// 기본 켬이지만 **표식이 붙은 세션에서만** 동작하므로, 표식을 쓰지 않는 사람에게는
    /// 아무 변화가 없다. 확인창이 거슬리면 여기서 끈다.
    #[serde(default = "yes")]
    pub guard_dangerous: bool,
    /// **오프라인 모드** — 사용자가 시키지 않은 바깥 호출을 하지 않는다.
    ///
    /// 폐쇄망(정부·금융·공장)에서는 그런 호출이 보안 경보를 띄우고 프록시 로그에 남는다.
    /// 사용자가 단추를 눌러 시작한 일(글꼴 내려받기 등)은 막지 않는다 — 눌렀는데 아무
    /// 일도 없으면 그건 보호가 아니라 고장이다.
    #[serde(default)]
    pub offline_mode: bool,
    /// 상태바에 공인 IP를 보여 준다(제삼자 서비스에 조회). **기본 켬**(지금 동작 유지).
    #[serde(default = "yes")]
    pub public_ip_lookup: bool,
    /// 명령 히스토리에 남길 때 비밀로 보이는 값을 가린다. **기본 켬.**
    ///
    /// 끄면 명령이 있는 그대로 설정 파일에 쌓인다 — 그 파일은 밖으로 나가기 쉽다.
    #[serde(default = "yes")]
    pub redact_history: bool,
    /// 세션 로그에 쓸 때도 가린다. **기본 켬.**
    #[serde(default = "yes")]
    pub redact_logs: bool,
    /// 새 셸·SSH 창의 출력을 **저절로** 파일에 기록한다(설정 폴더의 `logs/`).
    ///
    /// 기본은 꺼짐 — 터미널에는 비밀번호도 남의 데이터도 지나간다. 묻지 않고 디스크에
    /// 남길 일이 아니다.
    #[serde(default)]
    pub session_log_auto: bool,
    /// 세션 로그를 **되감을 수 있는** asciinema `.cast` 형식으로 남긴다.
    ///
    /// 기본은 꺼짐 — 지금까지 남기던 평문 로그가 사람이 바로 읽기에는 낫다. 켜면 시각이
    /// 함께 들어가 나중에 실제 속도로 재생할 수 있다(장애 재현·인수인계).
    #[serde(default)]
    pub session_log_cast: bool,
    /// 다녀온 **로컬** 폴더(최신이 앞). 원격과 같은 규칙(`recentpaths`)을 쓴다.
    #[serde(default)]
    pub local_recent: Vec<String>,
    /// 다녀온 원격 경로(`호스트:경로`, 최신이 앞). 북마크와 달리 **스스로** 쌓인다.
    #[serde(default)]
    pub sftp_recent: Vec<String>,
    /// SSH 접속 시간 제한(초). 0=기본(15). 폐쇄망·위성 회선처럼 느린 곳에서 늘린다.
    ///
    /// 호스트키 확인창이 뜰 수 있는 첫 접속에는 이 값과 무관하게 넉넉히 준다
    /// (지문 읽는 시간이 여기 들어가기 때문 — `nabi_ssh::conntimeout`).
    #[serde(default)]
    pub ssh_connect_timeout_secs: u64,
    /// SSH keepalive 간격(초). 0=끄기. 방화벽/유휴 타임아웃 대응(ServerAliveInterval). 기본 30.
    #[serde(default = "default_keepalive")]
    pub ssh_keepalive_secs: u64,
    /// 로컬 브라우저 정렬 내림차순 여부(컬럼 헤더 재클릭으로 토글).
    #[serde(default)]
    pub browser_sort_desc: bool,
    /// 로컬 브라우저 보기 모드(0=자세히,1=목록,2=큰,3=작은,4=타일).
    #[serde(default)]
    pub browser_view: u8,
    /// **열을 하나 더 보여 준다** — 로컬은 속성(RHSA), 원격은 권한(rwxr-xr-x).
    ///
    /// 기본은 꺼짐이다. 대부분의 사람에게 이름·유형·크기·날짜면 충분하고, 열이 늘면
    /// 이름 칸이 그만큼 좁아진다. 필요한 사람(서버를 다루는 쪽)이 켠다.
    #[serde(default)]
    pub browser_extra_col: bool,
    /// 양자내성 연결 정책: "auto"(기본) | "warn" | "require".
    ///
    /// 기본이 "auto" 인 까닭은, 막는 것을 기본으로 두면 어제까지 되던 접속이 오늘
    /// 안 되고 사용자는 우리가 고장 났다고 판단하기 때문이다. 지키고 싶은 사람이 켠다.
    #[serde(default = "default_kex_policy")]
    pub ssh_kex_policy: String,
    /// 에이전트 제어 평면 모드: "off" | "ask"(기본) | "on". (docs/agent-control.md)
    #[serde(default = "default_control_mode")]
    pub control_mode: String,
    /// OSC 7771 in-band 제어 허용(기본 false — 원격 출력 위장 방지). 로컬 pane만 처리.
    #[serde(default)]
    pub control_allow_osc: bool,
    /// 앱이 스크롤백을 지우지 못하게 한다(`CSI 3 J`). 기본 **켬**.
    ///
    /// 화면을 덮어 그리는 TUI 가 다시 그리기 전에 이 시퀀스를 보내는 일이 잦은데, 그러면
    /// 사람이 위로 올려 보려던 것이 그 순간 없어진다. 지운 것은 되돌릴 수 없고, 안 지운
    /// 것은 언제든 지울 수 있다(메뉴의 "스크롤백 비우기") — 그래서 막는 쪽이 기본이다.
    #[serde(default = "default_true")]
    pub protect_scrollback: bool,
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
    /// 이 초 이상 걸린 명령이 **보이지 않는 자리에서** 끝나면 알린다. 0=시간 알림 끔
    /// (실패 알림은 남는다). 예전에는 10초가 코드에 박혀 있었다.
    #[serde(default = "default_slow_command_secs")]
    pub slow_command_secs: u64,
    /// 접속 이력을 파일로 남길 것인가(호스트·사용자·시각·지속시간만).
    #[serde(default = "default_true")]
    pub keep_conn_history: bool,
    /// 세션 이름 → 접속 시 보낼 환경변수(`KEY=VALUE` 여러 줄).
    ///
    /// `SavedSession`이 아니라 여기 두는 것은 `auto_forwards`·`last_connected`와 같은 이유다 —
    /// 세션 구조체를 만드는 자리가 서른 곳이 넘어 필드를 더하면 그만큼을 함께 고쳐야 한다.
    #[serde(default)]
    pub session_env: std::collections::HashMap<String, String>,
    /// 세션별 마지막 접속 시각(이름→unix초). 세션 목록에 상대시간 표시(D4).
    #[serde(default)]
    pub last_connected: std::collections::BTreeMap<String, i64>,
    /// 디렉터리 방문 기록(경로→(횟수,마지막unix초)) — zoxide식 점프(E1).
    #[serde(default)]
    pub dir_visits: std::collections::BTreeMap<String, (u32, i64)>,
    /// 명령 히스토리((명령,cwd,종료코드,unix초)) — Atuin식 재실행(E5).
    #[serde(default)]
    pub cmd_history: Vec<(String, String, i32, i64)>,
    /// 팔레트에서 최근에 고른 명령 **이름**(최신이 앞). 동작이 아니라 이름을 기억하는
    /// 이유는 paletteorder 문서 참고 — 동작에는 pane 번호 같은 그때뿐인 값이 들어 있다.
    #[serde(default)]
    pub palette_recent: Vec<String>,
    /// 외부 편집기 실행 파일(비우면 OS 기본 앱). 원격 파일을 밖에서 열 때 쓴다.
    #[serde(default)]
    pub external_editor: String,
    /// 진단 로그 보관 일수. 0이면 정리하지 않는다(끄는 길).
    #[serde(default = "default_log_keep_days")]
    pub log_keep_days: u32,
    /// 사용자 정의 링크 규칙(`정규식 -> 주소틀`). 로그의 낱말을 클릭 가능한 주소로.
    #[serde(default)]
    pub link_rules: Vec<String>,
    /// 세션 이름 → 접속 시 자동으로 열 터널(`로컬:원격호스트:원격포트`).
    /// 세션 파일이 아니라 여기 두는 이유는 autofwd 문서 참고(last_connected와 같은 선례).
    #[serde(default)]
    pub auto_forwards: std::collections::BTreeMap<String, Vec<String>>,
    /// 명령 소요 시간((끝난 unix초, 걸린 초)) — cmd_history와 **따로** 둔다.
    /// 기존 튜플을 늘리면 옛 설정 파일이 파싱에 실패하고, 그러면 설정 전체가 초기화된다
    /// (load는 extract().unwrap_or_default()다). 새 필드는 없으면 기본값이라 안전하다.
    #[serde(default)]
    pub cmd_secs: Vec<(i64, u32)>,
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
/// 30초. 사람이 "자리를 뜰까" 망설이기 시작하는 지점이라 알림이 쓸모 있어지는 첫 구간이다.
fn default_slow_command_secs() -> u64 { 30 }
fn default_control_mode() -> String { "ask".into() }
fn default_kex_policy() -> String { "auto".into() }
fn default_stats_secs() -> u64 { 3 }
fn yes() -> bool { true }
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
            ssh_kex_policy: default_kex_policy(),
            auto_reconnect: false,
            ssh_stats_alert_pct: 90,
            slow_command_secs: default_slow_command_secs(),
            keep_conn_history: true,
            session_env: Default::default(),
            last_connected: std::collections::BTreeMap::new(),
            dir_visits: std::collections::BTreeMap::new(),
            cmd_history: Vec::new(),
            palette_recent: Vec::new(),
            cmd_secs: Vec::new(),
            auto_forwards: std::collections::BTreeMap::new(),
            link_rules: Vec::new(),
            log_keep_days: default_log_keep_days(),
            external_editor: String::new(),
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
            warn_paste_unicode: true,
            sftp_upload_mode: String::new(),
            snippets: Vec::new(),
            speed_limit_kbps: 0,
            sftp_verify_hash: false,
            sftp_name_charset: default_sftp_charset(),
            ai_profiles: Vec::new(),
            ai_cmd_bar: true,
            ai_last_model: String::new(),
            ai_last_effort: String::new(),
            tip_overlay: true,
            tip_translate_ai: false,
            tip_cache_path: String::new(),
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
            auto_reply: false,
            guard_dangerous: true,
            offline_mode: false,
            public_ip_lookup: true,
            redact_history: true,
            redact_logs: true,
            session_log_auto: false,
            session_log_cast: false,
            local_recent: Vec::new(),
            sftp_recent: Vec::new(),
            ssh_connect_timeout_secs: 0,
            ssh_keepalive_secs: default_keepalive(),
            browser_sort_desc: false,
            browser_view: 0,
            browser_extra_col: false,
            control_mode: "ask".into(),
            control_allow_osc: false,
            protect_scrollback: true,
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

/// 로그 기본 보관 일수 — 문제를 되짚기에 넉넉하되 폴더가 부풀지 않을 만큼.
fn default_log_keep_days() -> u32 {
    30
}
