//! 터미널 화면 모델(alacritty_terminal 래퍼) + 바뀜 세대(dirty_gen) 추적.
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
    /// 내용 변경 세대(process/resize마다 +1) — 렌더 캐시 무효화 키(cache.rs).
    pub(crate) dirty_gen: u64,
    /// 렌더 계산 캐시(render_rows / 밑줄맵) — 별개 RefCell(빌림 충돌 방지).
    pub(crate) rows_cache: std::cell::RefCell<crate::cache::RowsCache>,
    pub(crate) ul_cache: std::cell::RefCell<crate::cache::UlCache>,
    /// 인코딩 자동 감지용 raw(디코드 전) 바이트 표본(앞 8KB까지). chardetng 입력(B9).
    detect_sample: Vec<u8>,
    /// DEC 1007(alternate scroll) 추적 — 코어가 모르는 모드라 따로 관찰한다(altscroll.rs).
    alt_scroll: crate::altscroll::AltScroll,
    /// 오간 바이트를 그대로 보낼 곳(세션 기록). 없으면 아무 일도 하지 않는다.
    raw_tap: Option<std::sync::mpsc::Sender<Vec<u8>>>,
}

impl TermModel {
    /// 주어진 그리드 크기와 스크롤백 줄 수로 모델을 만든다.
    pub fn new(size: GridSize, scrollback: usize) -> Self {
        // Kitty keyboard protocol 협상 허용(T2-3). 기본 false면 push/query가 무시되어
        // 지원 앱(claude CLI 등)이 Shift+Enter 구분을 못 쓴다. 인코딩은 nabi-render.
        let cfg = Config { scrolling_history: scrollback, kitty_keyboard: true, ..Default::default() };
        let sink = EvSink::default();
        let term = Term::new(cfg, &TermSize::new(size.cols() as usize, size.rows() as usize), sink.clone());
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
            dirty_gen: 0,
            rows_cache: std::cell::RefCell::default(),
            ul_cache: std::cell::RefCell::default(),
            detect_sample: Vec::new(),
            alt_scroll: crate::altscroll::AltScroll::default(),
            raw_tap: None,
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

    /// 오간 바이트를 **그대로 흘려보낼 곳**을 건다(세션 기록용).
    ///
    /// ## 왜 필요한가
    ///
    /// 세션 기록은 지금까지 "화면 밖으로 밀려난 줄"만 적었다. 그래서 제자리에 덮어 그리는
    /// 프로그램(Claude Code 등)에서는 밀려나는 줄이 거의 없어 **기록이 멈췄다**
    /// (사용자 보고 2026-08-29 — "페이지가 이어지지 않는다").
    ///
    /// 줄이 아니라 바이트를 적어야 전부 남는다. `.cast` 형식이 원래 그것을 위한 것이다.
    ///
    /// 파일에 쓰는 일은 여기서 하지 않는다 — 이 함수는 출력 실에서 도는데, 거기서 디스크를
    /// 만지면 느린 디스크가 터미널을 멈춘다. 보내기만 하고 쓰는 것은 UI 실이 한다.
    pub fn set_raw_tap(&mut self, tx: std::sync::mpsc::Sender<Vec<u8>>) {
        self.raw_tap = Some(tx);
    }

    /// 기록을 멈춘다.
    pub fn clear_raw_tap(&mut self) {
        self.raw_tap = None;
    }

    /// PTY/SSH 바이트 청크를 파서에 먹인다(스트림 가능).
    pub fn process(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(tx) = &self.raw_tap {
            // 받는 쪽이 사라졌으면 조용히 끈다 — 실패해도 터미널은 계속 돌아야 한다.
            if tx.send(bytes.to_vec()).is_err() {
                self.raw_tap = None;
            }
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
        self.dirty_gen = self.dirty_gen.wrapping_add(1); // 렌더 캐시 무효화(reflow).
    }

    /// 현재 그리드 크기(열·행).
    pub fn size(&self) -> GridSize {
        GridSize::new(self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    /// 검색 등에서 화면 갱신 표시.
    pub(crate) fn mark_dirty(&mut self) {

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

    /// Kitty keyboard protocol 활성 플래그(스펙 비트: 1=disambiguate 2=event types
    /// 4=alternate 8=all-as-esc 16=associated text). 협상은 코어가 처리 — 여기선 읽기만.
    pub fn kitty_keys(&self) -> u8 {
        let md = self.term.mode();
        let bits = [TermMode::DISAMBIGUATE_ESC_CODES, TermMode::REPORT_EVENT_TYPES, TermMode::REPORT_ALTERNATE_KEYS, TermMode::REPORT_ALL_KEYS_AS_ESC, TermMode::REPORT_ASSOCIATED_TEXT];
        bits.iter().enumerate().fold(0, |acc, (i, b)| acc | (md.contains(*b) as u8) << i)
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

    }

    /// 현재 스크롤백 오프셋(0=최신).
    pub fn scrollback_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// 최신(하단)으로 즉시 복귀한다.
    pub fn scroll_to_bottom(&mut self) {
        if self.scrollback_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);

        }
    }

    /// 지금 화면 맨 위에 있는 줄의 절대 번호. [`scroll_to_abs_line`]의 반대다.
    ///
    /// 표식(스크롤백 마크)이 "지금 보는 자리"를 잡으려면 이 값이 필요하다. 표시 오프셋만
    /// 들고 있으면 출력이 흘렀을 때 같은 자리를 가리키지 못한다 — 오프셋은 최신 기준이라
    /// 새 줄이 올 때마다 같은 화면이 다른 값이 된다.
    pub fn top_abs_line(&self) -> usize {
        let rows = self.size().rows() as usize;
        let total = self.history_size() + rows;
        total.saturating_sub(rows).saturating_sub(self.scrollback_offset())
    }

    /// 절대 줄 번호(0 = 스크롤백 맨 위)가 화면 맨 위에 오도록 스크롤한다.
    ///
    /// 표시 오프셋은 "최신에서 얼마나 거슬러 올라갔나"이고 절대 줄은 "맨 위에서 몇 번째"라
    /// 방향이 반대다. 그래서 전체에서 화면 높이와 목표를 빼서 뒤집는다. 범위를 벗어나면
    /// 코어가 알아서 클램프한다.
    pub fn scroll_to_abs_line(&mut self, abs: usize) {
        let rows = self.size().rows() as usize;
        let total = self.history_size() + rows;
        let delta = total.saturating_sub(rows).saturating_sub(abs);
        self.term.scroll_display(Scroll::Bottom);
        if delta > 0 {
            self.term.scroll_display(Scroll::Delta(delta.min(i32::MAX as usize) as i32));
        }

    }

    /// 스크롤백 맨 위(가장 오래된)로 이동한다.
    pub fn scroll_to_top(&mut self) {
        self.term.scroll_display(Scroll::Top);

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

    /// **왕복이 맞아야 한다** — 표식은 이 두 함수 사이에서 자리를 잃지 않아야 한다.
    #[test]
    fn the_top_line_survives_a_round_trip() {
        let mut m = TermModel::new(GridSize::new(20, 5), 1000);
        for i in 0..100 {
            m.process(format!("line {i}\r\n").as_bytes());
        }
        for target in [0usize, 7, 42, 90] {
            m.scroll_to_abs_line(target);
            assert_eq!(m.top_abs_line(), target, "{target}줄로 갔다가 다른 자리를 돌려줬다");
        }
    }

    /// 맨 아래(최신)에서는 화면 맨 위가 '전체 - 화면높이'다.
    #[test]
    fn at_the_bottom_the_top_line_is_the_last_screenful() {
        let mut m = TermModel::new(GridSize::new(20, 5), 1000);
        for i in 0..30 {
            m.process(format!("line {i}\r\n").as_bytes());
        }
        m.scroll_to_bottom();
        assert_eq!(m.top_abs_line(), m.total_abs_lines().saturating_sub(5));
    }

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

    /// 절대 줄로 보내면 그 줄이 화면에 들어와야 한다 — "모든 창에서 찾기"가 이걸로 점프한다.
    #[test]
    fn jumping_to_an_absolute_line_brings_it_on_screen() {
        let mut m = TermModel::new(GridSize::new(20, 4), 500);
        for i in 0..100 {
            m.process(format!("line {i}
").as_bytes());
        }
        let total = m.total_abs_lines();
        m.scroll_to_abs_line(10);
        let seen = m.lines_abs_text(0, total);
        assert!(m.scrollback_offset() > 0, "과거로 거슬러 올라갔어야 한다");
        // 화면 맨 위가 목표 줄 근처인지 — 정확히 같은 줄이 위에 오는지 본다.
        let top = total - m.size().rows() as usize - m.scrollback_offset();
        assert_eq!(top, 10, "요청한 절대 줄이 화면 맨 위여야 한다");
        assert!(seen[10].contains("line"), "그 줄에 내용이 있어야 한다");
    }

    /// 범위를 벗어난 요청은 터지지 않고 끝으로 클램프된다.
    #[test]
    fn jumping_past_the_end_is_clamped_not_a_panic() {
        let mut m = TermModel::new(GridSize::new(20, 4), 100);
        m.process(b"only one line
");
        m.scroll_to_abs_line(999_999);
        assert_eq!(m.scrollback_offset(), 0, "미래로는 갈 수 없다");
    }
}

#[cfg(test)]
mod cleartests {
    use super::*;
    use nabi_types::GridSize;

    fn model() -> TermModel {
        TermModel::new(GridSize::new(20, 5), 1000)
    }

    /// `ESC[2J` 는 화면을 지우는데, **윈도우 콘솔은 지우기 전에 스크롤백으로 밀어 올린다.**
    /// xterm 계열은 그 자리에서 지운다. 지금 우리가 어느 쪽인지 먼저 확인한다.
    #[test]
    fn what_does_clear_screen_do_to_history_now() {
        let mut m = model();
        m.process(b"one\r\ntwo\r\nthree\r\n");
        let before = m.history_size();
        m.process(b"\x1b[2J");
        println!("2J 전 히스토리 {before} · 후 {}", m.history_size());
    }

    /// 밀어 올리기(SU)가 실제로 히스토리를 늘리는가 — 고칠 방법이 있는지 확인한다.
    #[test]
    fn scrolling_up_moves_lines_into_history() {
        let mut m = model();
        m.process(b"one\r\ntwo\r\nthree\r\n");
        let before = m.history_size();
        m.process(b"\x1b[5S");
        let after = m.history_size();
        println!("SU 전 {before} · 후 {after}");
        assert!(after > before, "SU 가 히스토리를 늘려야 고칠 수 있다");
    }
}
