//! 용량 무제한 편집기(`textview`)의 우클릭 메뉴.
//!
//! ## 왜 따로 있나
//!
//! 이 저장소에는 글을 보여 주는 창이 넷이다 — 작은 문서(`editorctx`), rope 편집기
//! (`editbufmenu`), HEX(`edithexmenu`), 그리고 **무제한 편집기**. 앞의 셋은 오른쪽
//! 버튼에 답하는데 이 창만 아무 반응이 없었다(2026-09-10에 세어 보고 알았다).
//!
//! 그런데 이 창이야말로 **거대한 로그를 여는 창**이다. AI 에게 한 토막 넘기거나
//! "이 파일 이 줄을 보라"고 알려 줄 일이 가장 잦은 자리인데, 거기에 길이 없었다.
//!
//! ## 왜 rope 쪽 메뉴를 그대로 못 쓰나
//!
//! 저장소가 다르다. rope 편집기는 문서 전체를 메모리에 펼쳐 두고 **문자 번호**로 말하고,
//! 이쪽은 파일을 그대로 두고 **바이트 자리**로 말한다. 그래서 메뉴 항목의 뜻은 같아도
//! 값을 꺼내는 길이 다르다.
//!
//! 대신 **돌려주는 그릇([`BufMenuAct`])과 상한은 같은 것을 쓴다.** 그래야 부르는 쪽이
//! 두 벌이 되지 않고, "AI 로 복사"의 상한이 창마다 다른 일도 안 생긴다.
//!
//! ## 여기에 일부러 없는 것
//!
//! * **전체 선택** — 이 편집기는 파일이 512MB 를 넘어서 열린 창이다. 전체를 고르면
//!   그 순간 전부를 메모리에 펼쳐야 하므로, 이 편집기가 존재하는 이유가 사라진다.
//! * **파일 전체를 AI 로** — 같은 까닭이고, rope 쪽도 같은 이유로 빼 두었다.
//! * **접기·다중 커서** — 문서 전체를 훑어야 하는 기능이라 이 창에는 아예 없다.
//!
//! 없는 까닭을 여기 적어 둔다. 안 적으면 다음 회차가 "쌍둥이가 어긋났다"며 또 넣는다.

use crate::editbufmenu::{BufMenuAct, MAX_AI_COPY};
use crate::textbuf::TextBuf;
use nabi_i18n::{tr, Lang};

/// 우클릭 메뉴를 그린다. 값 꺼내기는 여기서, 실행은 부르는 쪽에서(빌림 충돌 회피).
pub fn context_menu(
    ui: &mut egui::Ui,
    tb: &TextBuf,
    lang: Lang,
    readonly: bool,
    path: &std::path::Path,
    hint: &str,
) -> BufMenuAct {
    let mut act = BufMenuAct::default();
    let sel = tb.has_selection();
    if ui.add_enabled(sel && !readonly, egui::Button::new(tr(lang, "ctx.cut"))).clicked() {
        act.copy = Some(tb.selected_text());
        act.cut = true;
        ui.close();
    }
    if ui.add_enabled(sel, egui::Button::new(tr(lang, "menu.copy"))).clicked() {
        act.copy = Some(tb.selected_text());
        ui.close();
    }
    if ui.add_enabled(!readonly, egui::Button::new(tr(lang, "menu.paste"))).clicked() {
        act.paste = true;
        ui.close();
    }
    ui.separator();
    ai_menu(ui, tb, lang, path, hint, &mut act);
    ui.separator();
    if ui.button(tr(lang, "menu.find")).clicked() {
        act.find = true;
        ui.close();
    }
    act
}

/// **AI 로 복사** — 고른 만큼만 코드블록으로, 또는 "여기를 보라"는 자리 표시로.
fn ai_menu(
    ui: &mut egui::Ui,
    tb: &TextBuf,
    lang: Lang,
    path: &std::path::Path,
    hint: &str,
    act: &mut BufMenuAct,
) {
    ui.menu_button(tr(lang, "ctx.aicopy"), |ui| {
        let (a, b) = tb.selection();
        let len = (b - a) as usize;
        let ok = (1..=MAX_AI_COPY).contains(&len);
        if ui.add_enabled(ok, egui::Button::new(tr(lang, "ctx.copymd"))).clicked() {
            let head = header(path);
            act.copy = Some(format!("{head}```{hint}\n{}\n```", tb.selected_text().trim_end()));
            ui.close();
        }
        // 너무 크면 **왜 안 되는지** 말해 준다 — 눌러도 아무 일 없는 것이 가장 나쁘다.
        if len > MAX_AI_COPY {
            ui.label(tr(lang, "editor.xform.toobig"));
        }
        let has_path = !path.as_os_str().is_empty();
        if ui.add_enabled(has_path, egui::Button::new(tr(lang, "ctx.copyloc"))).clicked() {
            act.copy = Some(format!("{}:{}", path.display(), linespec(tb)));
            ui.close();
        }
    });
}

/// 출처 경로 머리글 — AI 가 어느 파일 이야기인지 알게 한다. 경로가 없으면 빈 글.
fn header(path: &std::path::Path) -> String {
    match path.as_os_str().is_empty() {
        true => String::new(),
        false => format!("`{}`\n", path.display()),
    }
}

/// 지금 자리를 `12` 나 `12-40` 으로. 줄 번호는 **색인에 직접 묻는다** —
/// 글로 펼치면 이 편집기를 쓰는 뜻이 사라진다.
fn linespec(tb: &TextBuf) -> String {
    let (a, b) = tb.selection();
    let line = |off: u64| tb.data.line_of(off) + 1;
    match a == b {
        true => line(a).to_string(),
        false => crate::editorloc::linespec(line(a), line(b.saturating_sub(1))),
    }
}
