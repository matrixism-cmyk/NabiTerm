//! 로컬 파일 브라우저 상태(SftpPanel과 대칭) — 사이드패널/탭 양쪽에서 공유.

use crate::browserfs::Sort;
use crate::viewmode::ViewMode;
use std::path::PathBuf;

pub(crate) struct BrowserPanel {
    /// 사이드패널 표시 여부(탭과 별개).
    pub open: bool,
    pub path: PathBuf,
    pub sort: Sort,
    pub sort_desc: bool,
    pub show_hidden: bool,
    pub filter: String,
    /// 새 폴더/파일 입력 버퍼(Some=입력 중).
    pub mkdir: Option<String>,
    pub new_is_file: bool,
    /// 이름변경 (원래, 새 이름) 버퍼.
    pub rename: Option<(String, String)>,
    pub rename_focus: bool, // 이름변경 시작 직후 1프레임 편집기 포커스.
    pub font_size: f32,     // 파일 목록 글꼴 크기(Ctrl+휠 — 이 브라우저만).
    pub selected: Option<String>,
    /// 다중 선택 집합(Ctrl/Shift+클릭). selected는 범위 기준점(anchor).
    pub multi: std::collections::HashSet<String>,
    /// 키보드 이동 시 선택 항목으로 스크롤(1프레임).
    pub scroll: bool,
    pub view: ViewMode,
    pub back: Vec<PathBuf>,
    pub fwd: Vec<PathBuf>,
    /// 편집 가능한 경로 주소창 버퍼(비편집 시 현재 경로 표시, Enter로 이동 — SFTP와 일관).
    pub addr: String,
}

impl Default for BrowserPanel {
    fn default() -> Self {
        Self {
            open: false,
            path: crate::browser::home_dir(),
            sort: Sort::Name,
            sort_desc: false,
            show_hidden: false,
            filter: String::new(),
            mkdir: None,
            new_is_file: false,
            rename: None,
            rename_focus: false,
            font_size: 13.0,
            selected: None,
            multi: std::collections::HashSet::new(),
            scroll: false,
            view: ViewMode::Details,
            back: Vec::new(),
            fwd: Vec::new(),
            addr: String::new(),
        }
    }
}
