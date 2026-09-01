//! 분리 창(별도 OS 창·"창 안에 띄우기")의 **끌어서 선택·복사·스크롤바**.
//!
//! ## 왜 따로 생겼나
//!
//! 2026-09-01에 탭 터미널과 분리 창이 부르는 함수를 나란히 세어 봤더니, 분리 창에는
//! 열넷이 없었다. 그중 셋은 터미널이라면 당연히 되는 것이었다.
//!
//! * **끌어서 선택이 아예 없었다.** 분리 창은 그리는 쪽에 선택을 `None`으로 넘기고 있어서,
//!   글자를 끌어도 파랗게 칠해지지 않고 복사할 길도 없었다. 우클릭 붙여넣기는 되는데
//!   복사는 안 되는, 한쪽만 뚫린 창이었다.
//! * **스크롤바가 없었다.** 뱃지("아래로")만 있고 막대가 없어 어디쯤인지 볼 수 없었다.
//! * **한글 조합 글자가 안 보였다.** 조합 중인 글자를 그리는 자리에 빈 글을 넘기고 있어서,
//!   분리 창에서 한글을 치면 **다 치고 나서야** 글자가 나타났다.
//!
//! 셋 다 새로 만들 것이 없었다 — 탭이 쓰는 함수를 그대로 부르면 된다. 그래서 이 모듈은
//! 기능이 아니라 **배선**이다. 새 구현을 두면 다음에 한쪽만 고쳐지는 날이 온다.

use crate::selection::Sel;
use nabi_types::PaneId;
use nabi_vt::{TermModel, Theme};

/// 분리 창이 선택을 하려면 필요한 것들 — 인자 스무 개짜리 함수에 넷을 더 붙이지 않으려고 묶었다.
pub(crate) struct FloatParity<'a> {
    /// 앱 전체가 공유하는 선택 상태(탭과 같은 것 — 창을 오가도 선택이 하나뿐이다).
    pub sel: &'a mut Option<Sel>,
    /// 더블클릭한 토막이 `파일:줄`이면 여기에 담아 caller가 편집기로 연다.
    pub pathline: &'a mut Option<(String, usize)>,
    /// 강조할 낱말 목록(설정) — 탭에만 칠해지면 그것도 표면 어긋남이다.
    pub keywords: &'a [String],
    /// 끌기를 놓는 순간 자동으로 복사할지(설정).
    pub copy_on_select: bool,
    /// 위험 명령 확인이 켜져 있는가(설정).
    pub guard_on: bool,
    /// 운영·스테이징 표식이 붙은 창들.
    pub risky: &'a std::collections::HashSet<PaneId>,
    /// 붙잡힌 입력 — 확인창이 이것을 보고 뜬다.
    pub pending_send: &'a mut Option<crate::guard::PendingSend>,
    /// 알림 자리(휠 안내 등) — 앱이 띄운다.
    pub notify: &'a mut Option<(String, std::time::Instant)>,
    /// 안내를 이미 본 창들 — 같은 말을 휠 굴릴 때마다 하면 잔소리가 된다.
    pub hinted: &'a mut std::collections::HashSet<PaneId>,
}

/// 터미널이 그려지는 자리와 글자 한 칸의 크기 — 선택 계산에 늘 셋이 함께 간다.
#[derive(Clone, Copy)]
pub(crate) struct Cells {
    pub rect: egui::Rect,
    pub cw: f32,
    pub ch: f32,
}

/// 이번 프레임에 그릴 선택 — 범위와 사각 여부, 그리고 방금 놓았는지.
pub(crate) struct SelPaint {
    pub span: Option<(usize, usize, usize, usize)>,
    pub rect: bool,
    /// 이번 프레임에 끌기를 놓았다(자동 복사 시점).
    pub released: bool,
}

impl FloatParity<'_> {
    /// 끌기를 좇아 선택을 갱신한다.
    ///
    /// 앱이 마우스를 먹는 모드(vim·htop 등)이거나 포인터가 스크롤바 위면 선택하지 않는다 —
    /// 탭과 같은 규칙이다. 분리 창은 한 pane만 보여 주므로 탭 쪽의 가림 판정(다른 레이어가
    /// 덮었나)은 필요 없다.
    pub(crate) fn track(
        &mut self,
        ui: &egui::Ui,
        c: Cells,
        pane: PaneId,
        model: &TermModel,
        mouse_on: bool,
    ) -> SelPaint {
        let over_sb = !model.alt_screen()
            && model.history_size() > 0
            && crate::scrollbar::over_scrollbar(c.rect, ui.input(|i| i.pointer.interact_pos()));
        if mouse_on || over_sb {
            return SelPaint { span: None, rect: false, released: false };
        }
        let (span, released, rect_sel) =
            crate::selection::track_selection(ui, c.rect, c.cw, c.ch, pane, model, self.sel);
        SelPaint { span, rect: rect_sel, released }
    }

    /// 놓았을 때 자동 복사 + 더블클릭 낱말 복사. 클립보드로 보낸다.
    ///
    /// 더블클릭한 토막이 `파일:줄`이면 복사 대신 편집기로 보낸다(탭과 같다) — 복사해 놓고
    /// 사람이 다시 찾아가게 두면 그 기능이 있는 줄도 모른다.
    pub(crate) fn copy(
        &mut self,
        ui: &egui::Ui,
        c: Cells,
        pane: PaneId,
        model: &TermModel,
        theme: &Theme,
        sp: &SelPaint,
        mouse_on: bool,
    ) {
        if !mouse_on {
            if let Some(text) =
                crate::clicks::handle_click_select(ui, c.rect, c.cw, c.ch, pane, model, theme, self.sel)
            {
                let dbl =
                    ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
                match dbl.then(|| crate::pathline::parse_path_line(&text)).flatten() {
                    Some(pl) => *self.pathline = Some(pl),
                    None => ui.ctx().copy_text(text),
                }
            }
        }
        if !(sp.released && self.copy_on_select) {
            return;
        }
        let Some((sr, sc, er, ec)) = sp.span else { return };
        let rows = model.render_rows(theme);
        let wrapped: Vec<bool> = (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
        let text = crate::selection::extract_selection(&rows, sr, sc, er, ec, &wrapped, sp.rect);
        if !text.is_empty() {
            ui.ctx().copy_text(text);
        }
    }

    /// 되돌릴 수 없는 명령이면 붙잡는다 — 붙잡았으면 `true`(보내지 말 것).
    ///
    /// **운영 표식을 붙인 세션을 창으로 떼는 순간 보호가 사라지고 있었다.** 표식은 창이
    /// 아니라 세션에 붙는 것이니, 어느 창에서 치든 같은 확인을 거쳐야 한다. 분리 창은
    /// 자기 pane 하나만 보여 주므로 브로드캐스트 대상은 빈 목록이다(=이 창만 본다).
    #[must_use = "붙잡았으면 그 바이트를 보내면 안 된다"]
    pub(crate) fn hold_risky(
        &mut self,
        panes: &nabi_orchestrator::SharedPanes,
        pane: PaneId,
        bytes: &[u8],
    ) -> bool {
        match crate::guard::guard_input(self.guard_on, self.risky, panes, pane, bytes, &[]) {
            Some(p) => {
                *self.pending_send = Some(p);
                true
            }
            None => false,
        }
    }
}

impl FloatParity<'_> {
    /// 위로 올렸는데 **꿈쩍도 안 했으면** 왜 그런지 한 번 말해 준다.
    ///
    /// 화면을 덮어 그리는 프로그램은 지나간 화면을 스크롤백으로 흘려보내지 않는다.
    /// 그걸 모르면 "나비텀이 기록을 잃어버렸다"로 보인다 — 사용자가 실제로 그렇게 보고했다.
    /// 그래서 만든 안내인데, **분리 창에서는 한 번도 뜬 적이 없었다**(2026-09-01).
    pub(crate) fn wheel_hint(
        &mut self,
        pane: PaneId,
        lang: nabi_i18n::Lang,
        alt_screen: bool,
        stuck: bool,
        history: usize,
    ) {
        if crate::panewheel::needs_empty_hint(alt_screen, stuck, history) && self.hinted.insert(pane)
        {
            let msg = nabi_i18n::tr(lang, "wheel.nohistory").to_string();
            *self.notify = Some((msg, std::time::Instant::now()));
        }
    }
}

/// 이번 프레임에 조합 중인 글자(한글·일본어 IME).
///
/// 탭 쪽은 앱이 들고 있는 `ime_preedit`을 쓰지만, 분리 창은 **자기 뷰포트의 입력**을 받으므로
/// 여기서 직접 뽑는 편이 옳다. 배선을 하나 줄이는 김에 어느 창을 지우든 다른 창이 안 깨진다.
pub(crate) fn preedit_text(events: &[egui::Event]) -> String {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::preedit_text;

    /// 조합 중인 글자를 못 뽑으면 분리 창에서 한글이 **다 치고 나서야** 보인다.
    #[test]
    fn the_last_preedit_wins() {
        let ev = |s: &str| {
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: s.to_string(),
                active_range_chars: None,
            })
        };
        // 한 프레임에 여러 조합 사건이 오면 마지막이 지금 모양이다.
        assert_eq!(preedit_text(&[ev("ㄱ"), ev("가")]), "가");
        // 조합이 없으면 빈 글 — 그리는 쪽이 아무것도 덧그리지 않는다.
        assert_eq!(preedit_text(&[]), "");
        assert_eq!(preedit_text(&[egui::Event::WindowFocused(true)]), "");
    }
}
