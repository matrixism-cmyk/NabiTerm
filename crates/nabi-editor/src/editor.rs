//! 내장 텍스트 에디터 — 문서 모델 + 열기/저장/로드 + 디스패치. 렌더는 [`crate::editortab`].
//! 각 문서는 도크 탭(PaneId) 하나로, 다른 패널처럼 탭/분리 창에 배치된다.

use std::path::{Path, PathBuf};

/// 한 편집 문서(도크 탭/분리 창 하나).
pub struct EditorDoc {
    pub title: String,
    /// 로컬 파일 경로 또는 원격 임시 파일 경로.
    pub path: PathBuf,
    /// Some((id, 원격경로))면 저장 시 그 경로로 업로드.
    pub remote: Option<(nabi_proto::SftpId, String)>,
    pub text: String,
    pub dirty: bool,
    /// 내용 적재 완료(원격은 다운로드 후 true).
    pub loaded: bool,
    /// 이 에디터 본문 글꼴 크기(Ctrl+휠 줌 — E2).
    pub font_size: f32,
    /// 감지된 인코딩 라벨(E2). 저장은 현재 UTF-8.
    pub encoding: String,
    /// 줄 끝(저장 시 보존 — E2).
    pub eol: &'static str,
    /// 코드 하이라이트 켜기(E3 — 토글, 대용량은 자동 평문).
    pub highlight: bool,
    /// 자동 줄바꿈(끄면 가로 스크롤 — E5).
    pub wrap: bool,
    /// 공백/탭을 점·화살표로 표시(E5).
    pub show_ws: bool,
    /// 읽기 전용 토글(E5 — 편집 잠금).
    pub readonly: bool,
    /// 대용량이면 가상화 뷰어(읽기 전용 — E4). 일반 파일은 None(편집 가능).
    pub big: Option<crate::editbig::BigFile>,
    /// 대용량 편집 버퍼(E6 — rope). Some면 가상화 편집기로 렌더(big/text 미사용).
    pub edit: Option<crate::editbuf::EditBuf>,
    /// 찾기/바꾸기/줄이동 상태(E5).
    pub find: crate::editorfind::FindState,
    /// 메뉴바 표시(nabiPad N1). 열 때 EditorConfig.show_menu_bar로 초기화.
    pub show_menu: bool,
    /// HEX(이진) 편집 버퍼(nabiPad N3). Some면 HEX 편집기로 렌더(text/edit/big 미사용).
    pub hex: Option<crate::edithex::HexBuf>,
    /// 텍스트 통계 캐시(D 성능): (len_key, chars, lines). 길이 변화 시에만 재계산.
    pub stats_cache: (usize, usize, usize),
    /// 미니맵(우측 축소 개요) 표시. 일반 텍스트 편집기에서만.
    pub minimap: bool,
    /// 개요(좌측 아웃라인) 패널 표시 — 헤더/정의 줄 목록 클릭 점프.
    pub outline: bool,
    /// 좌측 줄 번호 거터 표시(기본 true, 보기 메뉴에서 토글).
    pub show_lineno: bool,
    /// 북마크한 줄(0기반) — 거터에 점 표시, 다음/이전 점프(VS Code식).
    pub bookmarks: Vec<usize>,
    /// 현재 커서 줄(0기반, 렌더가 매 프레임 갱신) — 북마크 토글/점프 기준.
    pub cur_line: usize,
    /// 구문 강조 언어 강제(확장자 문자열, 예 "rs"). None이면 파일 확장자 자동(무제목 문서 언어 모드용).
    pub syntax_ext: Option<String>,
    /// LSP 진단(T6-4): (0기반 줄, 심각도 1=오류 2=경고…, 메시지). 앱 LSP 허브가 매 프레임 채운다.
    pub diags: Vec<(usize, u8, String)>,
    /// 현재 커서 문자 오프셋(렌더가 매 프레임 갱신) — 정의 이동 등 위치 기반 요청 기준.
    pub cur_off: usize,
}

impl EditorDoc {
    /// 구문 강조·주석·표시에 쓰는 실효 언어 확장자: 구문 언어 모드(syntax_ext) 우선, 없으면 파일 확장자(없으면 "txt").
    pub fn lang_ext(&self) -> String {
        self.syntax_ext.clone().unwrap_or_else(|| self.path.extension().and_then(|e| e.to_str()).unwrap_or("txt").to_string())
    }

    /// 지정 줄(0-base)로 이동: 스크롤·현재줄 표시·커서를 그 줄 시작으로(명시적 줄 이동 공용).
    pub fn jump_to_line(&mut self, line0: usize) {
        self.find.scroll_to = Some(line0);
        self.find.cur = line0;
        self.find.pending_cursor = Some(crate::textpos::line_col_to_offset(&self.text, line0, 1));
    }

    /// 글자수·줄수를 돌려준다. 텍스트 길이가 바뀐 경우에만 전체 스캔(매 프레임 재스캔 회피).
    pub fn text_stats(&mut self) -> (usize, usize) {
        let len = self.text.len();
        if self.stats_cache.0 != len {
            let chars = self.text.chars().count();
            let lines = self.text.split('\n').count();
            self.stats_cache = (len, chars, lines);
        }
        (self.stats_cache.1, self.stats_cache.2)
    }

    /// 일반(비대용량) 문서의 공통 기본값 — 생성 보일러플레이트 축약.
    #[allow(clippy::too_many_arguments)]
    pub fn make(
        title: String,
        path: PathBuf,
        remote: Option<(nabi_proto::SftpId, String)>,
        text: String,
        loaded: bool,
        font_size: f32,
        encoding: String,
        eol: &'static str,
    ) -> Self {
        EditorDoc {
            title, path, remote, text, dirty: false, loaded, font_size, encoding, eol,
            highlight: true, wrap: true, show_ws: false, readonly: false, big: None, edit: None,
            find: Default::default(), show_menu: false, hex: None, stats_cache: (usize::MAX, 0, 0), minimap: false, outline: false, show_lineno: true, bookmarks: Vec::new(), cur_line: 0, syntax_ext: None,
            diags: Vec::new(), cur_off: 0,
        }
    }
}

/// 렌더가 모은 액션(central/floating이 적용). dirty/줌은 렌더가 doc에 직접 반영.
#[derive(Default)]
pub struct EditorAct {
    pub save: bool,
    /// 열린 모든 변경 문서를 저장(파일 메뉴·팔레트 공용).
    pub save_all: bool,
    /// 다른 이름으로 저장(네이티브 대화상자).
    pub save_as: bool,
    /// 인코딩을 지정 라벨로 재디코드(상태바 드롭다운).
    pub set_encoding: Option<String>,
    /// 줄 끝을 지정 형식(LF/CRLF/CR)으로 변환(상태바 드롭다운).
    /// toggle_menu_bar: 메뉴바 표시/숨김 토글(앱이 EditorConfig에 반영·저장).
    /// toggle_hex: 텍스트↔HEX 편집 모드 전환. reload: 디스크에서 다시 읽기(되돌리기).
    pub set_eol: Option<&'static str>, pub toggle_menu_bar: bool, pub toggle_hex: bool, pub reload: bool,
    /// 현재 텍스트를 지정 인코딩으로 저장(인코딩 메뉴 ▸ 이 인코딩으로 저장).
    pub save_encoding: Option<String>,
    /// 파일 닫기 요청(파일 메뉴 ▸ 닫기). 변경사항 있으면 저장 확인 모달.
    pub close: bool,
    /// nabiPad 자체 설정 창 열기 요청(메뉴 ▸ 설정).
    pub open_settings: bool,
    /// 파일 메뉴 ▸ 새 문서 / 열기(파일 대화상자) 요청(단독 에디터 필수).
    pub new_doc: bool, pub open_file: bool,
    /// 최근 파일 메뉴에서 선택한 경로(열기).
    pub open_recent: Option<String>,
    /// 현재 문서를 디스크 원본과 비교(변경사항 diff).
    pub diff_disk: bool,
    /// 선택 텍스트를 포커스 외 첫 터미널 pane에서 실행(우클릭 ▸ 터미널에서 실행).
    pub run_in_term: Option<String>,
}

/// 경로의 파일명(표시용).
pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

