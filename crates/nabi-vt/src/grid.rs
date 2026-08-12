//! 터미널 화면 모델(alacritty_terminal 래퍼) + damage(dirty) 추적.
//!
//! vt100에서 교체(T1): 스크롤백 언더플로 버그 해결 + 향후 이미지/reflow 기반.

use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{test::TermSize, Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use nabi_types::GridSize;

/// 터미널이 올린 이벤트(제목·벨·질의 응답) 수집기 — evsink.rs.
pub(crate) use crate::evsink::EvSink;

/// 한 pane의 권위 있는 화면 상태.
pub struct TermModel {
    pub(crate) term: Term<EvSink>,
    parser: Processor<StdSyncHandler>,
    sink: EvSink,
    /// 제목 캐시(&str 반환 유지를 위해 process에서 동기화).
    title: String,
    /// 명령 블록(OSC 133;A 절대 줄 + 종료코드) 기록 — 점프/블록 바용(prompts.rs).
    pub(crate) prompts: Vec<crate::prompts::PromptMark>,
    /// 인라인 이미지(Sixel/iTerm/Kitty) 스캐너·저장소·셀 픽셀 높이·Kitty 청크 누적(images.rs).
    pub(crate) escan: crate::images::EscScan,
    pub(crate) images: Vec<crate::images::PlacedImage>,
    pub(crate) next_img_id: u64,
    pub(crate) cell_px_h: f32,
    pub(crate) kitty: Option<crate::images::KittyPending>,
    dirty: bool,
    /// 내용 변경 세대(process/resize마다 +1) — 렌더 캐시 무효화 키(cache.rs).
    pub(crate) dirty_gen: u64,
    /// 렌더 계산 캐시(render_rows / 밑줄맵) — 별개 RefCell(빌림 충돌 방지).
    pub(crate) rows_cache: std::cell::RefCell<crate::cache::RowsCache>,
    pub(crate) ul_cache: std::cell::RefCell<crate::cache::UlCache>,
    /// 인코딩 자동 감지용 raw(디코드 전) 바이트 표본(앞 8KB까지). chardetng 입력(B9).
    detect_sample: Vec<u8>,
    /// DEC 1007(alternate scroll) 추적 — 코어가 모르는 모드라 따로 관찰한다(altscroll.rs).
    alt_scroll: crate::altscroll::AltScroll,
}

impl TermModel {
    /// 주어진 그리드 크기와 스크롤백 줄 수로 모델을 만든다.
    pub fn new(size: GridSize, scrollback: usize) -> Self {
        let cfg = Config {
            scrolling_history: scrollback,
            ..Default::default()
        };
        let sink = EvSink::default();
        let term = Term::new(
            cfg,
            &TermSize::new(size.cols() as usize, size.rows() as usize),
            sink.clone(),
        );
        Self {
            term,
            parser: Processor::new(),
            sink,
            title: String::new(),
            prompts: Vec::new(),
            escan: crate::images::EscScan::default(),
            images: Vec::new(),
            next_img_id: 0,
            cell_px_h: 17.0,
            kitty: None,
            dirty: true,
            dirty_gen: 0,
            rows_cache: std::cell::RefCell::default(),
            ul_cache: std::cell::RefCell::default(),
            detect_sample: Vec::new(),
            alt_scroll: crate::altscroll::AltScroll::default(),
        }
    }

    /// 한 바이트를 alacritty 파서에 직접 먹인다(이미지 줄 확보용 — images.rs).
    pub(crate) fn feed_parser(&mut self, b: u8) {
        self.parser.advance(&mut self.term, b);
    }

    /// 인코딩 감지용 raw 바이트 표본을 앞에서부터 8KB까지 모은다(디코드 전 원본).
    pub fn push_detect_sample(&mut self, raw: &[u8]) {
        const CAP: usize = 8192;
        if self.detect_sample.len() >= CAP {
            return;
        }
        let take = (CAP - self.detect_sample.len()).min(raw.len());
        self.detect_sample.extend_from_slice(&raw[..take]);
    }

    /// 수집된 raw 표본(인코딩 자동 감지 입력).
    pub fn detect_sample(&self) -> &[u8] {
        &self.detect_sample
    }

    /// PTY/SSH 바이트 청크를 파서에 먹인다(스트림 가능).
    pub fn process(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        for &b in bytes {
            self.parser.advance(&mut self.term, b);
            self.esc_observe(b); // 인라인 이미지(Sixel/iTerm/Kitty) 병렬 관찰.
            self.alt_scroll.observe(b); // DEC 1007(휠→커서 키) 요청 관찰.
        }
        let t = self.sink.title();
        if t != self.title {
            self.title = t;
        }
        self.dirty = true;
        self.dirty_gen = self.dirty_gen.wrapping_add(1); // 렌더 캐시 무효화.
    }

    /// 터미널 전체 리셋(RIS, ESC c) — 화면·속성·모드를 초기화한다(`reset` 명령 대용).
    pub fn reset(&mut self) {
        self.process(b"\x1bc");
        self.alt_scroll.clear(); // RIS는 사설 모드도 초기화한다(코어가 모르는 1007은 우리가).
    }

    /// 스크롤백(히스토리)만 비운다 — 현재 화면은 유지(xterm ED 3, "Clear Buffer").
    pub fn clear_scrollback(&mut self) {
        self.process(b"\x1b[3J");
    }

    /// 보이는 row번째 줄이 다음 줄로 소프트랩(자동 줄바꿈)되었는지(복사 시 개행 생략용).
    pub fn row_wrapped(&self, row: u16) -> bool {
        use alacritty_terminal::term::cell::Flags;
        let line = self.visible_line(row);
        let cols = self.term.columns();
        if cols == 0 {
            return false;
        }
        self.term.grid()[line][Column(cols - 1)].flags.contains(Flags::WRAPLINE)
    }

    /// 보이는 행 인덱스(0=맨 위) → 그리드 Line(스크롤백 오프셋 반영).
    pub(crate) fn visible_line(&self, row: u16) -> Line {
        Line(row as i32 - self.term.grid().display_offset() as i32)
    }

    /// 그리드 크기를 변경한다.
    pub fn resize(&mut self, size: GridSize) {
        self.term
            .resize(TermSize::new(size.cols() as usize, size.rows() as usize));
        self.dirty = true;
        self.dirty_gen = self.dirty_gen.wrapping_add(1); // 렌더 캐시 무효화(reflow).
    }

    /// 현재 그리드 크기(열·행).
    pub fn size(&self) -> GridSize {
        GridSize::new(self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    /// 마지막 clear 이후 변경이 있었는지(재그리기 게이트).
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// 렌더 후 dirty 플래그를 내린다.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// 검색 등에서 화면 갱신 표시.
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// 현재 커서 상태(가시 영역 좌표).
    pub fn cursor(&self) -> crate::cursor::CursorState {
        let p: Point = self.term.grid().cursor.point;
        crate::cursor::CursorState {
            row: p.line.0.max(0) as u16,
            col: p.column.0 as u16,
            visible: self.term.mode().contains(TermMode::SHOW_CURSOR),
        }
    }

    /// OSC 10/11 색 질의에 답할 색을 현재 테마로 맞춘다(렌더마다 호출해도 싸다).
    pub fn set_query_colors(&self, theme: &crate::cell::Theme) {
        let (f, b) = (theme.fg, theme.bg);
        self.sink.set_colors((f.r, f.g, f.b), (b.r, b.g, b.b));
    }

    /// 앱이 alternate scroll(DEC 1007)을 요청했는가 — 휠을 커서 키로 바꿔 보내라는 뜻.
    ///
    /// 대체 화면을 쓰지 않고 주 화면을 통째로 다시 그리는 TUI는 스크롤백에 아무것도 남기지
    /// 않아 휠이 무용지물이 된다. 그 앱들이 이 모드로 "휠을 키로 달라"고 요청한다.
    pub fn alt_scroll(&self) -> bool { self.alt_scroll.enabled() }

    /// 대체 화면 모드(vim/less 등)면 스크롤백을 비활성화해야 한다.
    pub fn alt_screen(&self) -> bool { self.term.mode().contains(TermMode::ALT_SCREEN) }

    /// 터미널이 설정한 제목(OSC 0/2). 없으면 빈 문자열.
    pub fn title(&self) -> &str { &self.title }

    /// 애플리케이션 커서 키 모드(DECCKM). 방향키 시퀀스에 영향.
    pub fn app_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// bracketed paste 모드(DECSET 2004). 붙여넣기 래핑에 영향.
    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// 벨 누적 횟수(시각 벨 트리거용).
    pub fn bell_count(&self) -> usize { self.sink.bells() }

    /// 터미널 질의 응답을 꺼낸다(호출측이 PTY로 써야 함). 없으면 빈 Vec.
    ///
    /// 장치 속성(`ESC[c`)·커서 위치(`ESC[6n`) 같은 질의는 응답이 돌아와야 프로그램이 진행한다.
    /// `process()` 직후 호출해 전송하지 않으면 질의한 쪽이 타임아웃한다.
    pub fn take_replies(&mut self) -> Vec<u8> {
        self.sink.take_replies()
    }

    /// 마우스 리포팅이 켜져 있는지.
    pub fn mouse_on(&self) -> bool {
        self.term.mode().intersects(TermMode::MOUSE_MODE)
    }

    /// 릴리스 이벤트도 보고해야 하는지(현 프로토콜들은 모두 보고).
    pub fn mouse_wants_release(&self) -> bool {
        self.mouse_on()
    }

    /// 드래그/이동 보고가 필요한지.
    pub fn mouse_wants_motion(&self) -> bool {
        self.term
            .mode()
            .intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION)
    }

    /// SGR 마우스 인코딩인지.
    pub fn mouse_sgr(&self) -> bool {
        self.term.mode().contains(TermMode::SGR_MOUSE)
    }

    /// 스크롤백을 delta줄 이동(+=과거, -=최신). 상한은 코어가 클램프한다.
    pub fn scroll_by(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
        self.dirty = true;
    }

    /// 현재 스크롤백 오프셋(0=최신).
    pub fn scrollback_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// 최신(하단)으로 즉시 복귀한다.
    pub fn scroll_to_bottom(&mut self) {
        if self.scrollback_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);
            self.dirty = true;
        }
    }

    /// 스크롤백 맨 위(가장 오래된)로 이동한다.
    pub fn scroll_to_top(&mut self) {
        self.term.scroll_display(Scroll::Top);
        self.dirty = true;
    }

    /// 히스토리(스크롤백) 줄 수(렌더러 스크롤바 길이 산출에도 사용).
    pub fn history_size(&self) -> usize {
        self.term.grid().history_size()
    }

    /// 커서가 있는 화면 행(그리드 좌표).
    pub(crate) fn cursor_line(&self) -> i32 {
        self.term.grid().cursor.point.line.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_wrap_sets_row_wrapped() {
        // 폭 10에 16자 출력 → 첫 줄이 다음 줄로 소프트랩(WRAPLINE)되어야 한다.
        let mut m = TermModel::new(GridSize::new(10, 5), 100);
        m.process(b"abcdefghijklmnop");
        assert!(m.row_wrapped(0), "긴 줄의 첫 행이 wrapped여야 함");
        assert!(!m.row_wrapped(1), "둘째 행은 wrap 아님");
        let rows = m.render_rows(&crate::cell::Theme::default());
        let r0: String = rows[0].iter().map(|c| c.text.as_str()).collect();
        assert_eq!(r0, "abcdefghij");
    }

    #[test]
    fn writes_text_into_grid() {
        let mut m = TermModel::new(GridSize::new(20, 5), 100);
        m.process(b"hi");
        let rows = m.render_rows(&crate::cell::Theme::default());
        assert_eq!(rows[0][0].text, "h");
        assert_eq!(rows[0][1].text, "i");
        assert!(m.dirty());
        m.clear_dirty();
        assert!(!m.dirty());
    }

    #[test]
    fn title_and_modes() {
        let mut m = TermModel::new(GridSize::new(20, 5), 100);
        m.process(b"\x1b]0;hello\x07");
        assert_eq!(m.title(), "hello");
        m.process(b"\x1b[?1049h"); // alt screen
        assert!(m.alt_screen());
        m.process(b"\x1b[?2004h");
        assert!(m.bracketed_paste());
        m.process(b"\x07");
        assert!(m.bell_count() >= 1);
    }

    /// 터미널 질의(DA1·커서 위치)는 응답을 만들어야 하고, 그 응답은 꺼내 쓸 수 있어야 한다.
    /// 이걸 버리면 질의한 프로그램이 응답을 기다리다 멈춘다.
    #[test]
    fn queries_produce_replies() {
        let mut m = TermModel::new(GridSize::new(20, 5), 100);
        assert!(m.take_replies().is_empty(), "질의 전에는 응답이 없다");

        m.process(b"\x1b[c"); // DA1 — 장치 속성.
        let da = m.take_replies();
        assert!(!da.is_empty(), "DA1은 응답해야 한다");
        assert_eq!(da[0], 0x1b, "CSI 응답이어야 한다");
        assert!(m.take_replies().is_empty(), "한 번 꺼내면 비워진다");

        m.process(b"\x1b[6n"); // DSR — 커서 위치.
        let dsr = m.take_replies();
        assert!(!dsr.is_empty(), "커서 위치 질의는 응답해야 한다");
        // 형식: ESC [ row ; col R
        assert_eq!(dsr.last().copied(), Some(b'R'), "커서 위치 응답은 R로 끝난다");
    }

    #[test]
    fn scroll_navigates_history() {
        let mut m = TermModel::new(GridSize::new(20, 3), 100);
        // 3행 그리드에 10줄 출력 → 스크롤백 생성.
        for i in 1..=10 { m.process(format!("line{i}\r\n").as_bytes()); }
        assert_eq!(m.scrollback_offset(), 0); // 시작은 하단(최신).
        m.scroll_by(1);
        assert!(m.scrollback_offset() > 0); // 위로 스크롤됨.
        m.scroll_to_bottom();
        assert_eq!(m.scrollback_offset(), 0); // 하단(라이브) 복귀.
        m.scroll_to_top();
        assert!(m.scrollback_offset() > 0); // 맨 위(가장 오래된).
    }
}
