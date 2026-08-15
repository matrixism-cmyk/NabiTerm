//! 설정 ▸ 텔레그램 브리지 페이지. 봇 토큰은 keyring(평문 금지), 나머지는 config.telegram.
//! 보안: 허용 chat 화이트리스트 필수 + "모든 권한 부여"(grant_all)가 입력 주입을 허용.

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

pub(crate) fn telegram_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    let have_token = nabi_secret::keyringstore::load_telegram_token().is_some();

    ui.label(tr(lang, "tg.enabled"));
    ui.checkbox(&mut cfg.telegram.enabled, "").on_hover_text(tr(lang, "tg.usage")); // 첫 설정 순서 안내.
    ui.end_row();

    // 활성화 상태 — 무엇이 더 필요한지 안내(브리지가 도는 조건: 사용 + 토큰 + 허용 chat).
    ui.label(tr(lang, "tg.status"));
    let st = if !cfg.telegram.enabled { "tg.st.off" }
        else if !have_token { "tg.st.notoken" }
        else if cfg.telegram.allowed_chats.is_empty() { "tg.st.nochat" }
        else { "tg.st.active" };
    ui.label(tr(lang, st));
    ui.end_row();

    // 봇 토큰(keyring) — 입력 버퍼는 임시 상태에만 두고 저장/삭제 버튼으로 반영(설정 평문 금지).
    ui.label(tr(lang, "tg.token"));
    ui.horizontal(|ui| {
        let id = egui::Id::new("tg_token_buf");
        let mut buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut buf).password(true).desired_width(200.0).hint_text("123456:ABC-..."));
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
        if ui.button(tr(lang, "qc.save")).clicked() && !buf.trim().is_empty() {
            nabi_secret::keyringstore::store_telegram_token(buf.trim());
            ui.data_mut(|d| d.insert_temp(id, String::new()));
        }
        if ui.button(tr(lang, "sessions.delete")).clicked() {
            nabi_secret::keyringstore::clear_telegram_token();
        }
        ui.label(tr(lang, if have_token { "tg.token.set" } else { "tg.token.none" }));
    });
    ui.end_row();

    // 허용 chat ID(쉼표 구분). 편집 중 쉼표 유지 위해 임시 버퍼 사용, 변경 시 파싱.
    ui.label(tr(lang, "tg.chats"));
    let id = egui::Id::new("tg_chats_buf");
    let mut s = ui.data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| cfg.telegram.allowed_chats.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(280.0).hint_text("123456789,-1001234..."));
    ui.data_mut(|d| d.insert_temp(id, s.clone()));
    if r.changed() {
        cfg.telegram.allowed_chats = s.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).collect();
    }
    ui.end_row();
    // 오너 규칙 안내 — 첫 chat만 셸 제어 가능(C2, OpenClaw 오너 모델).
    ui.label("");
    ui.weak(tr(lang, "tg.ownerhint"));
    ui.end_row();

    ui.label(tr(lang, "tg.grantall"));
    ui.checkbox(&mut cfg.telegram.grant_all, "").on_hover_text(tr(lang, "tg.grantall.hint"));
    ui.end_row();

    ui.label(tr(lang, "tg.replylines"));
    ui.add(egui::DragValue::new(&mut cfg.telegram.reply_lines).range(1..=200));
    ui.end_row();
    ui.label(tr(lang, "tg.idle"));
    ui.add(egui::DragValue::new(&mut cfg.telegram.idle_timeout_ms).range(500..=120_000).suffix(" ms"));
    ui.end_row();
    ui.label(tr(lang, "tg.poll"));
    ui.add(egui::DragValue::new(&mut cfg.telegram.poll_secs).range(1..=60).suffix(" s"));
    ui.end_row();
}
