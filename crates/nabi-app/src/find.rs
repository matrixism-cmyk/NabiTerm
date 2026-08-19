//! 스크롤백/화면 검색(Ctrl+F): 일치 셀 하이라이트(리터럴) + 일치 수 + n/N. 정규식 지원(B7, 탐색·카운트).

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 검색 매처: 리터럴(스마트케이스) 또는 정규식. 단어 단위는 두 경우 모두 정규식으로 만든다.
enum Matcher {
    Lit { needle: String, cs: bool },
    Re(regex::Regex),
}

impl Matcher {
    fn is_match(&self, line: &str) -> bool {
        match self {
            Matcher::Lit { needle, cs } => if *cs { line.contains(needle.as_str()) } else { line.to_lowercase().contains(needle.as_str()) },
            Matcher::Re(re) => re.is_match(line),
        }
    }
    fn count(&self, line: &str) -> usize {
        match self {
            Matcher::Lit { needle, cs } => if *cs { line.matches(needle.as_str()).count() } else { line.to_lowercase().matches(needle.as_str()).count() },
            Matcher::Re(re) => re.find_iter(line).count(),
        }
    }
}

/// 쿼리 옵션으로 매처를 만든다. 빈 쿼리/잘못된 정규식이면 None. 스마트케이스(소문자=무시).
///
/// 단어 단위(whole)는 리터럴이든 정규식이든 `\b…\b`로 감싼 정규식이 된다 — 리터럴은
/// escape해서 넣으므로 `a.b` 같은 쿼리가 갑자기 정규식처럼 동작하지 않는다.
fn build_matcher(query: &str, regex: bool, whole: bool) -> Option<Matcher> {
    if query.is_empty() {
        return None;
    }
    let cs = query.chars().any(|c| c.is_uppercase());
    if whole {
        let inner = if regex { query.to_string() } else { regex::escape(query) };
        return regex::RegexBuilder::new(&format!(r"\b(?:{inner})\b"))
            .case_insensitive(!cs).build().ok().map(Matcher::Re);
    }
    if regex {
        regex::RegexBuilder::new(query).case_insensitive(!cs).build().ok().map(Matcher::Re)
    } else {
        Some(Matcher::Lit { needle: if cs { query.to_string() } else { query.to_lowercase() }, cs })
    }
}

impl NabiApp {
    /// 셀 하이라이트에 쓸 검색어 — **리터럴 모드일 때만**.
    ///
    /// 하이라이트는 리터럴 셀 매칭이라 정규식·단어 단위 결과와 어긋난다(엉뚱한 셀을 칠한다).
    /// 탭·분리 창·창 안에 띄우기가 모두 이 한 함수를 쓴다 — 예전엔 탭만 정규식을 걸러냈다.
    pub(crate) fn find_highlight(&self) -> Option<String> {
        (self.find_open && !self.find_regex && !self.find_whole && !self.find_query.is_empty())
            .then(|| self.find_query.clone())
    }

    /// Ctrl+F 토글(shortcuts에서 consume 후 호출 — 터미널로 ^F 누수 방지). 단일 줄 선택을 검색어로.
    pub(crate) fn toggle_find(&mut self) {
        self.find_open = !self.find_open;
        if self.find_open {
            if let Some(t) = self.selection_text() {
                if !t.contains('\n') {
                    self.find_query = t;
                }
            }
        }
    }

    /// F3/Shift+F3 다음/이전 일치로 이동(shortcuts에서 consume 후 호출, 바가 닫혀 있어도 동작).
    pub(crate) fn find_nav(&mut self, forward: bool) {
        if !self.find_query.is_empty() {
            self.scroll_focused_match(forward);
        }
    }

    pub(crate) fn show_find_bar(&mut self, ctx: &egui::Context) {
        if !self.find_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.find_open = false;
            return;
        }
        let lang = self.lang;
        let count = self.find_match_count();
        let total = self.find_total_cached();
        let bad_re = self.find_regex && !self.find_query.is_empty() && build_matcher(&self.find_query, true, self.find_whole).is_none();
        let mut open = true;
        let (mut enter, mut forward, mut up, mut down) = (false, false, false, false);
        egui::Window::new(tr(lang, "find.title"))
            .open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::RIGHT_TOP, [-8.0, 40.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.find_query).hint_text(tr(lang, "find.hint")).desired_width(180.0));
                    nabi_editor::uiutil::focus_once(&resp); // 매 프레임 request_focus는 IME 조합 파괴(egui 0.36).
                    let (e, sh) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift));
                    enter = e;
                    forward = sh;
                    if bad_re {
                        ui.colored_label(crate::theme_ui::ERR, "\u{26a0}").on_hover_text(tr(lang, "find.badregex"));
                    } else if total == 0 && !self.find_query.is_empty() {
                        ui.colored_label(crate::theme_ui::ERR, "0"); // 검색 실패 단서.
                    } else {
                        // 화면 일치/스크롤백 전체(F5). 예: 2/17 = 화면 2개·전체 17개.
                        ui.label(format!("{count}/{total}")).on_hover_text(tr(lang, "find.counthint"));
                    }
                    if ui.small_button("\u{25b2}").clicked() { up = true; }
                    if ui.small_button("\u{25bc}").clicked() { down = true; }
                    ui.toggle_value(&mut self.find_regex, ".*").on_hover_text(tr(lang, "find.regex"));
                    // 단어 단위 — 에디터 찾기에는 있고 터미널에만 없던 옵션(표면 통일).
                    ui.toggle_value(&mut self.find_whole, "ab").on_hover_text(tr(lang, "find.whole"));
                    // 스마트케이스 표시: 대문자 있으면 구분(Aa), 없으면 무시(aa).
                    let cs = self.find_query.chars().any(|c| c.is_uppercase());
                    ui.label(if cs { "Aa" } else { "aa" }).on_hover_text(tr(lang, "find.smartcase"));
                });
            });
        if !open {
            self.find_open = false;
        }
        if enter { self.scroll_focused_match(forward); }
        if up { self.scroll_focused_match(false); }
        if down { self.scroll_focused_match(true); }
    }

    /// 터미널 선택 텍스트를 새 nabiPad 문서로 연다(분석·편집·AI 붙여넣기). 선택 없으면 무동작.
    pub(crate) fn selection_to_pad(&mut self) {
        if let Some(t) = self.selection_text() {
            let mut doc = crate::editor::EditorDoc::make(nabi_i18n::tr(self.lang, "nabipad.newdoc").to_string(), std::path::PathBuf::new(), None, t, true, self.font_size, "UTF-8".into(), "LF");
            doc.dirty = true; // 스크래치 문서(저장 시 다른 이름으로).
            self.add_editor_tab(doc);
        }
    }

    /// 열린 모든 일반 텍스트 문서를 '## 제목 + 코드블록'으로 묶어 복사(AI에 작업 세트 전체 제공).
    pub(crate) fn copy_open_tabs_md(&mut self, ctx: &egui::Context) {
        let mut docs: Vec<&crate::editor::EditorDoc> = self.editors.values().filter(|d| d.hex.is_none() && d.big.is_none() && d.edit.is_none()).collect();
        docs.sort_by(|a, b| a.title.cmp(&b.title));
        let mut out = String::new();
        for d in &docs {
            let hint = d.path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let head = if d.path.as_os_str().is_empty() { d.title.clone() } else { d.path.display().to_string() }; // 경로 우선(AI가 파일 참조·수정).
            out.push_str(&format!("## {head}\n```{hint}\n{}\n```\n\n", d.text.trim_end()));
        }
        if !out.is_empty() {
            ctx.copy_text(out);
            self.notify = Some((tr(self.lang, "cmd.tabsmdcopied").to_string(), std::time::Instant::now()));
        }
    }

    /// 포커스 pane의 마지막 명령 출력을 클립보드에 복사한다(Warp식 블록 복사, C4).
    pub(crate) fn copy_last_output(&mut self, ctx: &egui::Context) {
        let Some(p) = self.focused_pane() else { return };
        let t = self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned())
            .and_then(|v| v.model.lock().ok().and_then(|md| md.last_command_output()));
        if let Some(t) = t {
            ctx.copy_text(t);
            self.notify = Some((tr(self.lang, "cmd.outputcopied").to_string(), std::time::Instant::now()));
        }
    }

    /// 마지막 명령(명령+출력)을 마크다운 코드블록으로 복사 — AI 채팅에 바로 붙여넣기용(바이브코딩).
    pub(crate) fn copy_last_output_md(&mut self, ctx: &egui::Context) {
        let Some(p) = self.focused_pane() else { return };
        let view = self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned());
        let (out, cmd) = match view {
            Some(v) => match v.model.lock() {
                Ok(md) => (md.last_command_output(), self.run_cmd.get(&p).cloned()),
                Err(_) => return,
            },
            None => return,
        };
        if let Some(out) = out {
            // AI 디버깅 컨텍스트: cwd·명령·출력·종료코드(성공 0 포함, 알고 있을 때)를 한 코드블록에 담는다.
            let cwd = self.cwds.get(&p).map(|c| format!("# cwd: {}\n", crate::workspace::strip_uri_slash(c))).unwrap_or_default();
            let header = cmd.map(|c| format!("$ {c}\n")).unwrap_or_default();
            let exit = self.last_exit.get(&p).map(|e| format!("\n# exit: {e}")).unwrap_or_default();
            ctx.copy_text(format!("```\n{cwd}{header}{}{exit}\n```", out.trim_end()));
            self.notify = Some((tr(self.lang, "cmd.outputcopiedmd").to_string(), std::time::Instant::now()));
        }
    }

    /// 포커스된 pane을 일치로 스크롤한다(forward=Shift+Enter는 더 최신, 아니면 더 오래된).
    /// 포커스 pane을 스크롤백 맨 아래(라이브)로 이동(팔레트).
    pub(crate) fn scroll_focused_bottom(&mut self) {
        if let Some(view) = self.focused_pane().and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned())) {
            if let Ok(mut md) = view.model.lock() { md.scroll_to_bottom(); }
        }
    }

    /// 포커스 pane을 스크롤백 맨 위로 이동(팔레트).
    pub(crate) fn scroll_focused_top(&mut self) {
        if let Some(view) = self.focused_pane().and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned())) {
            if let Ok(mut md) = view.model.lock() { md.scroll_to_top(); }
        }
    }

    fn scroll_focused_match(&mut self, forward: bool) {
        let Some(m) = build_matcher(&self.find_query, self.find_regex, self.find_whole) else { return };
        let limit = self.config.terminal.search_limit;
        let Some(p) = self.focused_pane() else { return };
        if let Some(view) = self.orch.panes.read().ok().and_then(|mp| mp.get(&p).cloned()) {
            if let Ok(mut model) = view.model.lock() {
                if forward {
                    model.scroll_to_next_match(|line| m.is_match(line), limit);
                } else {
                    model.scroll_to_prev_match(|line| m.is_match(line), limit);
                }
            }
        }
    }

    /// 스크롤백 전체(검색 상한)의 일치 총수 — 쿼리별 캐시(매 프레임 재스캔 방지, F5).
    /// 라이브 출력 중에는 약간 지연될 수 있으나(쿼리 변경 시 갱신) 검색 보조엔 충분.
    fn find_total_cached(&mut self) -> usize {
        if let Some((q, r, n)) = &self.find_count_cache {
            if *q == self.find_query && *r == self.find_regex { return *n; }
        }
        let n = self.find_total_count();
        self.find_count_cache = Some((self.find_query.clone(), self.find_regex, n));
        n
    }

    fn find_total_count(&mut self) -> usize {
        let Some(m) = build_matcher(&self.find_query, self.find_regex, self.find_whole) else { return 0 };
        let limit = self.config.terminal.search_limit;
        let Some(p) = self.focused_pane() else { return 0 };
        let Some(view) = self.orch.panes.read().ok().and_then(|mp| mp.get(&p).cloned()) else { return 0 };
        let Ok(model) = view.model.lock() else { return 0 };
        let total = model.total_abs_lines();
        let from = total.saturating_sub(limit);
        model.lines_abs_text(from, total).iter().map(|l| m.count(l)).sum()
    }

    /// 포커스된 pane의 화면에서 현재 검색어 일치 수를 센다(매처=리터럴/정규식).
    fn find_match_count(&mut self) -> usize {
        let Some(m) = build_matcher(&self.find_query, self.find_regex, self.find_whole) else { return 0 };
        let Some(p) = self.focused_pane() else { return 0 };
        let Some(view) = self.orch.panes.read().ok().and_then(|mp| mp.get(&p).cloned()) else { return 0 };
        let Ok(model) = view.model.lock() else { return 0 };
        model
            .render_rows(&self.theme)
            .iter()
            .map(|row| {
                let s: String = row.iter().map(|c| if c.text.is_empty() { " " } else { c.text.as_str() }).collect();
                m.count(&s)
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::build_matcher;

    #[test]
    fn matcher_literal_and_regex() {
        let m = build_matcher("err", false, false).unwrap();
        assert!(m.is_match("an error here"));
        assert_eq!(m.count("err err"), 2);
        assert!(build_matcher("err", false, false).unwrap().is_match("ERR")); // 소문자→무시
        assert!(!build_matcher("ERR", false, false).unwrap().is_match("err")); // 대문자→구분
        let re = build_matcher("e.*r", true, false).unwrap();
        assert!(re.is_match("eXXr"));
        assert_eq!(re.count("eXr eYr"), 1); // 탐욕적 → 한 번
        assert!(build_matcher("(", true, false).is_none()); // 잘못된 정규식
        assert!(build_matcher("", false, false).is_none());
    }

    #[test]
    fn whole_word_matches_only_full_words() {
        let m = build_matcher("err", false, true).unwrap();
        assert!(m.is_match("an err here"));
        assert!(!m.is_match("error here")); // 단어 일부는 제외.
        // 리터럴은 escape되므로 정규식 메타문자가 그대로 글자로 취급된다.
        let dot = build_matcher("a.b", false, true).unwrap();
        assert!(dot.is_match("x a.b y"));
        assert!(!dot.is_match("x axb y"));
    }

}
