//! nabiPad(내장 에디터) 전용 설정.
//!
//! 향후 에디터를 별도 프로그램으로 분리하기 쉽도록 터미널 설정(config.toml)과
//! 완전히 분리된 파일(nabipad.toml)·env 네임스페이스(`NABI_EDITOR_*`)로 둔다.

use serde::{Deserialize, Serialize};

/// nabiPad 설정(터미널과 독립 저장).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    /// 파일 편집 시 기본을 도크 탭이 아니라 별도 창으로 연다(기본 true).
    pub open_in_window: bool,
    /// 메뉴바 표시 여부(기본 false=숨김). 토글 값은 저장되어 재시작 후 유지된다.
    pub show_menu_bar: bool,
    /// 본문 글꼴 크기(px). 0이면 앱 글꼴 크기를 따른다.
    pub font_size: f32,
    /// 본문 글꼴 패밀리(빈 값이면 앱 글꼴).
    pub font_family: String,
    /// 구문 강조(syntect) 기본 사용.
    pub syntax_highlight: bool,
    /// 자동 줄바꿈 기본값.
    pub word_wrap: bool,
    /// 공백 문자 표시 기본값.
    pub show_whitespace: bool,
    /// 탭 폭(공백 칸 수).
    pub tab_size: usize,
    /// "줄바꿈" 변환이 쓸 칸 수. 80은 관례일 뿐이고 요즘은 100·120도 흔하다.
    #[serde(default = "default_wrap_col")] pub wrap_col: usize,
    /// 세로 눈금을 그을 열들(`"80,100"`). 비우면 안 그린다.
    #[serde(default)]
    pub rulers: String,
    /// 탭 대신 공백으로 들여쓰기.
    pub indent_spaces: bool,
    /// 현재 줄 강조.
    pub highlight_current_line: bool,
    /// 저장 시 각 줄의 후행 공백을 제거한다(VS Code files.trimTrailingWhitespace).
    pub trim_on_save: bool,
    /// 저장 시 파일 끝에 개행을 하나 보장한다(VS Code files.insertFinalNewline).
    pub final_newline: bool,
    /// 구문 강조 테마 이름(syntect ThemeSet 키). 사용자 themes 폴더의 .tmTheme도 포함.
    pub theme: String,
    /// 확장자 → 구문 이름 강제 매핑(예: {"h"="C++"}). 자동 감지를 덮어쓴다.
    pub ext_map: std::collections::BTreeMap<String, String>,
    /// 최근 연 파일(최신 우선, 최대 보관 수는 사용처에서 제한).
    pub recent_files: Vec<String>,
    /// 자동 저장(VS Code files.autoSave): 켜면 경로 있는 변경 문서를 주기적으로 저장. 기본 끔.
    #[serde(default)]
    pub autosave: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            open_in_window: true,
            show_menu_bar: false,
            font_size: 0.0,
            font_family: String::new(),
            syntax_highlight: true,
            word_wrap: false,
            show_whitespace: false,
            tab_size: 4,
            wrap_col: 80,
            rulers: String::new(),
            indent_spaces: true,
            highlight_current_line: true,
            trim_on_save: false,
            final_newline: false,
            theme: "base16-mocha.dark".into(),
            ext_map: std::collections::BTreeMap::new(),
            recent_files: Vec::new(),
            autosave: false,
        }
    }
}

/// 줄바꿈 기본 칸 수(오래 쓰인 관례값).
fn default_wrap_col() -> usize { 80 }
