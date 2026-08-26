//! **만든 글을 편집기 탭으로 여는 한 가지 길.**
//!
//! diff 결과, 명령 출력, 폴더 통계 — 파일이 아닌 글을 편집기에 띄우는 일이 여러 곳에서
//! 필요하다. 같은 여덟 인자짜리 생성자를 자리마다 되풀이해 적고 있었고, 그러다 보니
//! `dirty` 표시를 켜는 것도 자리마다 따로였다.
//!
//! 한 줄이라도 어긋나면 "어떤 결과 창은 닫을 때 저장을 묻고, 어떤 것은 안 묻는" 차이가
//! 생긴다. 그래서 한 곳으로 모은다.

use crate::app::NabiApp;
use crate::editor::EditorDoc;
use std::path::PathBuf;

impl NabiApp {
    /// 파일 없는 글을 편집기 탭으로 연다(제목만 붙는 임시 문서).
    ///
    /// **`dirty`를 켠다** — 이 글은 디스크 어디에도 없으므로, 닫을 때 물어야 사용자가
    /// 잃지 않는다. 여기까지가 이 함수가 정하는 전부다.
    pub(crate) fn open_text_as_doc(&mut self, title: &str, body: String) {
        let mut doc = EditorDoc::make(
            title.to_string(),
            PathBuf::new(),
            None,
            body,
            true,
            self.font_size,
            "UTF-8".into(),
            "\n",
        );
        doc.dirty = true;
        self.add_editor_tab(doc);
    }
}
