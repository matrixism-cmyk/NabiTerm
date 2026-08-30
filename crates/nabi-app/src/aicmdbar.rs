//! AI 명령 바 — AI CLI가 실행 중인 pane 상단에 그 CLI의 명령을 클릭형으로 노출.
//!
//! "/를 눌러야 나오는 명령을 처음부터 위에"(2026-08-18) → "요약명으로"(08-19) →
//! "열려 있으면 색으로 표시하고 다시 누르면 닫히게"(08-19). 버튼에는 한눈에 읽히는
//! 이름(대화 요약·새 대화…), 툴팁에는 `"/compact 설명"` 형식으로 실제 명령과 설명을 보여준다.
//! 명령 표는 aicmdcmds.rs, 모드 감지는 aimode.rs.

use crate::aicmdcmds::{bar_kind, primary_commands};
use nabi_i18n::tr;

/// 바에서 고른 동작.
pub(crate) enum BarAction {
    /// 슬래시 명령 전송(명령, 이 명령이 CLI 화면을 여는가).
    Cmd(String, bool),
    /// 열어 둔 화면 닫기(ESC 전송).
    Esc,
    /// 권한 모드 순환(Shift+Tab 전송).
    ShiftTab,
}

/// pane별 명령 바 상태 — 버튼에 현재 값을 보여주고 열린 화면을 추적한다.
#[derive(Default, Clone)]
pub(crate) struct AiPicks {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// 바 버튼으로 지금 열어 둔 명령(활성 색 표시 + 재클릭 시 ESC).
    pub active: Option<String>,
}

/// 바 버튼 글자 크기 — 드롭다운(menu_button)과 일반 버튼의 **높이를 같게** 맞춘다.
/// 명령 바 글씨의 기준 크기(터미널 글꼴에 비례해 정한다).
///
/// 예전에는 12px로 못박혀 있었다. 터미널 글꼴을 크게 쓰는 사람에게는 바만 유난히 작아
/// 보인다(사용자 보고 2026-08-25). 터미널보다 조금 작게, 그러나 함께 커지게 한다.
fn bar_size(pane_font: f32) -> f32 {
    (pane_font * 0.8).clamp(10.0, 20.0)
}

/// 이번 프레임의 바 글씨 크기(그리기 직전에 정해 둔다 — RichText 헬퍼가 읽는다).
///
/// UI는 한 스레드에서만 도므로 경쟁이 없지만, 전역으로 두는 편이 헬퍼마다 크기를 들고
/// 다니는 것보다 호출부가 깨끗하다. f32를 비트로 담는다(원자적으로 다룰 수 있게).
static BAR_SIZE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// 이번 프레임의 바 글씨 크기를 정한다.
fn set_bar_size(px: f32) {
    BAR_SIZE.store(px.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// 지금 정해진 바 글씨 크기(아직 안 정했으면 기본 12px).
fn cur_bar_size() -> f32 {
    match BAR_SIZE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => 12.0,
        b => f32::from_bits(b),
    }
}

fn bar_text(s: impl Into<String>) -> egui::RichText {
    egui::RichText::new(s.into()).size(cur_bar_size())
}

/// 열려 있는 버튼의 강조색(노랑 배경 + 검은 글자) — 한눈에 "지금 이게 떠 있다"를 보여준다.
fn active_text(s: impl Into<String>) -> egui::RichText {
    bar_text(s).color(egui::Color32::BLACK).strong()
}

/// 명령 바가 필요로 하는 앱 상태(탭·분리 창 공용) — 표면마다 코드를 복사하지 않기 위한 묶음.
pub(crate) struct AiBarState<'a> {
    pub enabled: bool,
    pub run_cmd: &'a std::collections::HashMap<nabi_types::PaneId, String>,
    pub pane_status: &'a std::collections::HashMap<nabi_types::PaneId, std::collections::BTreeMap<String, String>>,
    pub picks: &'a mut std::collections::HashMap<nabi_types::PaneId, AiPicks>,
    pub screen: &'a mut std::collections::HashMap<nabi_types::PaneId, crate::aimode::AiScreen>,
    pub last_model: &'a str,
    pub last_effort: &'a str,
    pub pick_out: &'a mut Option<(String, String)>,
}

/// 명령 바를 그리고 pane에 보낼 바이트를 돌려준다(없으면 None).
///
/// **탭과 분리 창이 같은 코드를 쓴다** — 예전엔 탭(tabsterm)에만 있어 분리 창·창 안에 띄우기
/// pane에서는 바가 아예 나오지 않았다(사용자 보고 2026-08-19, 표면 드리프트 결함 클래스).
pub(crate) fn draw_ai_bar(
    ui: &mut egui::Ui,
    panes: &nabi_orchestrator::SharedPanes,
    pane: nabi_types::PaneId,
    lang: nabi_i18n::Lang,
    st: &mut AiBarState,
) -> Option<Vec<u8>> {
    if !st.enabled {
        return None;
    }
    // 화면 판독(모드·모델·노력·종류)은 내용이 바뀐 프레임에만 — 결과는 pane별 캐시.
    let scr = screen_state(panes, pane, st.screen);
    // CLI 종류: 셸 통합이 있으면 실행 명령으로, 없으면(=SSH pane) 창 제목·화면 문구로 판정한다.
    let kind = st.run_cmd.get(&pane).and_then(|c| bar_kind(c)).or(scr.title_kind)?;
    let picks = st.picks.get(&pane).cloned().unwrap_or_default();
    // 모델 우선순위: CLI 상태줄(pane_status) → 화면 판독 → 이 pane에서 고른 값 → 설정의 마지막 값.
    let model = st
        .pane_status
        .get(&pane)
        .and_then(|m| m.get("model"))
        .cloned()
        .or(scr.model)
        .or(picks.model)
        .or_else(|| Some(st.last_model.to_owned()).filter(|s| !s.is_empty()));
    let effort = scr
        .effort
        .or(picks.effort)
        .or_else(|| Some(st.last_effort.to_owned()).filter(|s| !s.is_empty()));
    let view = BarView {
        kind,
        mode: scr.mode,
        model: model.as_deref(),
        effort: effort.as_deref(),
        active: picks.active.as_deref(),
    };
    let action = show_bar(ui, lang, &view)?;
    Some(match action {
        BarAction::Cmd(cmd, opens_ui) => {
            let entry = st.picks.entry(pane).or_default();
            // 값 선택(예: "/model opus")은 즉시 적용 → 기억하고 설정에도 남긴다.
            if let Some(v) = cmd.strip_prefix("/model ") {
                entry.model = Some(v.to_owned());
                *st.pick_out = Some(("model".into(), v.to_owned()));
            } else if let Some(v) = cmd.strip_prefix("/effort ") {
                entry.effort = Some(v.to_owned());
                *st.pick_out = Some(("effort".into(), v.to_owned()));
            }
            // 화면을 여는 명령이면 활성으로 표시(다시 누르면 ESC로 닫는다).
            entry.active = opens_ui.then(|| cmd.clone());
            let mut d = cmd.into_bytes();
            d.push(b'\r');
            d
        }
        BarAction::Esc => {
            st.picks.entry(pane).or_default().active = None;
            vec![0x1b]
        }
        BarAction::ShiftTab => crate::aimode::SHIFT_TAB.to_vec(),
    })
}

/// pane 화면 판독 결과 — 세대가 같으면 캐시를 그대로 쓴다(내용 변경 프레임에만 스캔).
fn screen_state(
    panes: &nabi_orchestrator::SharedPanes,
    pane: nabi_types::PaneId,
    cache: &mut std::collections::HashMap<nabi_types::PaneId, crate::aimode::AiScreen>,
) -> crate::aimode::AiScreen {
    let unknown = crate::aimode::AiScreen { mode: "aimode.unknown", ..Default::default() };
    let Some(md) = panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.model.clone())) else {
        return unknown;
    };
    let Ok(model) = md.lock() else { return unknown };
    let gen = model.render_gen();
    if let Some(c) = cache.get(&pane) {
        if c.gen == gen {
            return c.clone();
        }
    }
    let scanned = crate::aimode::scan(&model, gen);
    cache.insert(pane, scanned.clone());
    scanned
}

impl crate::tabs::TermTabViewer<'_> {
    /// 탭 pane에 AI 명령 바를 그린다(공통 구현 위임).
    pub(crate) fn ai_bar(&mut self, ui: &mut egui::Ui, pane: nabi_types::PaneId) {
        // 바 글씨를 이 pane의 터미널 글꼴에 맞춘다 — 큰 글꼴에서 바만 작아 보이지 않게.
        let pf = self.pane_font.get(&pane).copied().unwrap_or(self.font_size);
        set_bar_size(bar_size(pf));
        let mut st = AiBarState {
            enabled: self.ai_cmd_bar,
            run_cmd: self.run_cmd,
            pane_status: self.pane_status,
            picks: self.ai_picks,
            screen: self.ai_screen,
            last_model: self.ai_last_model,
            last_effort: self.ai_last_effort,
            pick_out: self.ai_pick_out,
        };
        let data = draw_ai_bar(ui, &self.orch.panes, pane, self.lang, &mut st);
        if let Some(data) = data {
            self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
        }
    }

    /// 사용자가 pane에 직접 입력하면 바가 표시하던 "열림" 상태를 해제한다 —
    /// 화면을 키보드로 닫았을 수 있으므로 바만 계속 노랗게 남지 않게 한다.
    pub(crate) fn clear_ai_active(&mut self, pane: nabi_types::PaneId) {
        if let Some(p) = self.ai_picks.get_mut(&pane) {
            p.active = None;
        }
    }
}

/// 바 한 줄을 그리는 데 필요한 현재 상태.
pub(crate) struct BarView<'a> {
    pub kind: &'static str,
    pub mode: &'static str,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub active: Option<&'a str>,
}

/// 명령 바 UI. 반환 = 고른 동작.
pub(crate) fn show_bar(ui: &mut egui::Ui, lang: nabi_i18n::Lang, v: &BarView) -> Option<BarAction> {
    let mut send = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // 모든 버튼이 같은 여백을 쓰게 고정 — 드롭다운/일반 버튼 높이 일치.
        ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
        ui.label(bar_text(format!("\u{1f916} {}", v.kind)).color(crate::theme_ui::ACCENT));
        // 모드 순환 버튼: 누를 때마다 shift+tab을 보내고, 현재 모드를 화면에서 읽어 보여준다.
        if ui
            .button(bar_text(format!("\u{21e7}\u{21e5} {}", tr(lang, v.mode))))
            .on_hover_text(tr(lang, "aicb.shifttab"))
            .clicked()
        {
            send = Some(BarAction::ShiftTab);
        }
        ui.separator();
        for bc in primary_commands(v.kind) {
            // /model·/effort는 명령 이름 대신 현재 값을 노출(사용자 요청) — 없으면 요약명.
            let cur = match bc.cmd {
                "/model" => v.model,
                "/effort" => v.effort,
                _ => None,
            };
            let label = cur.map_or_else(|| tr(lang, bc.label).to_string(), str::to_string);
            let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
            let open = v.active == Some(bc.cmd);
            if open {
                // 열려 있는 동안은 강조색 + 다시 누르면 ESC로 닫는다(드롭다운도 이때는 닫기 버튼).
                let btn = egui::Button::new(active_text(label)).fill(crate::theme_ui::BROADCAST);
                if ui.add(btn).on_hover_text(tr(lang, "aicb.close")).clicked() {
                    send = Some(BarAction::Esc);
                }
            } else if bc.sub.is_empty() {
                if ui.button(bar_text(label)).on_hover_text(tip).clicked() {
                    send = Some(BarAction::Cmd(bc.cmd.to_string(), bc.opens_ui));
                }
            } else {
                ui.menu_button(bar_text(format!("{label} \u{25be}")), |ui| {
                    for (sub_label, cmd) in bc.sub {
                        if ui.button(*sub_label).clicked() {
                            // 값이 붙지 않은 순수 명령만 화면을 연다(값 선택은 즉시 적용).
                            send = Some(BarAction::Cmd((*cmd).to_string(), *cmd == bc.cmd));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tip);
            }
        }
        // 종료 버튼 — "⋯" 바로 앞. 끝내려고 매번 더보기를 열지 않게(사용자 요청 2026-08-22).
        // 되돌릴 수 없는 동작이라 붉은 계열로 두고, 무엇을 보내는지 툴팁에 밝힌다.
        let quit = egui::Button::new(bar_text(format!("\u{2715} {}", tr(lang, "aicb.l.exit"))))
            .fill(crate::theme_ui::ERR);
        let tip = format!("{} {}", crate::aicmdcmds::QUIT_CMD, tr(lang, "aicb.quit.hint"));
        if ui.add(quit).on_hover_text(tip).clicked() {
            send = Some(BarAction::Cmd(crate::aicmdcmds::QUIT_CMD.to_owned(), false));
        }
        // 더보기는 명령이 많아(Claude 80+) 주제별 하위 메뉴 + 검색으로 낸다.
        if let Some(a) = crate::aicmdmore::more_menu(ui, lang, v.kind, bar_text("\u{22ef}")) {
            send = Some(a);
        }
    });
    send
}

#[cfg(test)]
mod bar_size_tests {
    use super::{bar_size, cur_bar_size, set_bar_size};

    /// 바 글씨는 터미널 글꼴을 **따라 커진다** — 12px에 못박혀 있던 것을 고쳤다.
    #[test]
    fn the_bar_grows_with_the_terminal_font() {
        assert!(bar_size(24.0) > bar_size(14.0));
        assert!(bar_size(14.0) < 14.0, "터미널보다는 작아야 한다");
    }

    /// 아주 작거나 큰 글꼴에서도 읽을 수 있는 범위를 지킨다.
    #[test]
    fn it_stays_inside_a_readable_range() {
        assert_eq!(bar_size(4.0), 10.0);
        assert_eq!(bar_size(100.0), 20.0);
    }

    /// 아직 정하지 않았으면 예전 기본값(12px)으로 그린다.
    #[test]
    fn before_the_first_frame_it_falls_back_to_the_old_default() {
        assert_eq!(cur_bar_size(), 12.0);
        set_bar_size(17.5);
        assert_eq!(cur_bar_size(), 17.5);
        set_bar_size(12.0);
    }
}
