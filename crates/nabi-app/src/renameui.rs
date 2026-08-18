//! 인라인 이름변경 오버레이(탐색기식) — 별도 입력칸 대신 파일명 위치에서 바로 편집.
//! 로컬/SFTP 브라우저의 자세히(Details) 테이블 셀이 공유한다. + 브라우저 Ctrl+휠 줌 헬퍼.

/// 패널 위에서 Ctrl+휠 방향을 돌려준다(+1 확대/-1 축소/0 없음). 양쪽 브라우저 공용.
pub(crate) use nabi_editor::uiutil::ctrl_wheel_zoom;

/// 이름변경 편집기 상태(패널 필드를 빌려 구성). 대상 행에서 `edit`로 오버레이.
/// target/buf는 비활성이면 None(더미 버퍼 불필요).
pub(crate) struct RenameUi<'a> {
    pub target: Option<String>,
    pub buf: Option<&'a mut String>,
    /// 시작 직후 1프레임 포커스 요청 플래그(편집기에 자동 포커스).
    pub focus: &'a mut bool,
    /// 결과: 확정(Enter/포커스 이탈) / 취소(Esc).
    pub commit: bool,
    pub cancel: bool,
}

impl RenameUi<'_> {
    /// 이 이름이 현재 편집 대상인가.
    pub fn active(&self, name: &str) -> bool {
        self.target.as_deref() == Some(name)
    }

    /// 대상 행이면 편집기를 그리고 true(상호작용 생략). 셀 렌더러가 첫 줄에서 호출.
    pub fn try_edit(&mut self, ui: &mut egui::Ui, rect: egui::Rect, name: &str) -> bool {
        if !self.active(name) {
            return false;
        }
        self.edit(ui, rect);
        true
    }

    /// rect 위치에 인라인 편집기를 그린다(대상 행에서만 호출).
    /// Enter 또는 포커스 이탈=확정, Esc=취소(탐색기식).
    pub fn edit(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let Some(buf) = self.buf.as_deref_mut() else {
            return;
        };
        let resp = ui.put(rect, egui::TextEdit::singleline(buf).margin(egui::vec2(2.0, 0.0)));
        if *self.focus {
            nabi_editor::uiutil::focus_once(&resp);
            // 포커스 시 기본 이름(확장자 제외)을 선택 — 바로 새 이름 타이핑(탐색기식).
            if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), resp.id) {
                let end = buf.rfind('.').filter(|&i| i > 0).map_or_else(|| buf.chars().count(), |i| buf[..i].chars().count());
                let r = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(end));
                st.cursor.set_char_range(Some(r));
                st.store(ui.ctx(), resp.id);
            }
            *self.focus = false;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel = true;
        } else if resp.lost_focus() {
            // 싱글라인 TextEdit은 Enter·다른 곳 클릭 모두 lost_focus → 확정.
            self.commit = true;
        }
    }
}
