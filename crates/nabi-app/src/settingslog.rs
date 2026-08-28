//! 세션 기록 설정 — 터미널에 오간 것을 파일로 남길지(배치 AM).
//!
//! ## 왜 옮겼나
//!
//! 이 설정들은 `cfg.terminal.*` 인데 **원격 연결** 페이지에 있었다. SSH 로 붙었을 때만
//! 남는 것처럼 보였지만 로컬 셸도 똑같이 남는다.
//!
//! 자리가 어긋나면 찾을 수 없다. 실제로 사용자에게 "설정 ▸ 터미널"에 있다고 알려 드렸다가
//! 없다는 말을 들었다(2026-08-28). **내가 있어야 할 자리로 착각한 곳이 곧 사용자가 찾는
//! 자리다.**
//!
//! ## 왜 파일을 갈랐나
//!
//! `settingsui2.rs` 가 줄 한도를 넘었다. 옮기면서 그 파일에서 덜어 내면 두 문제가 함께
//!풀린다 — 옮길 곳을 새로 만드는 편이 옮겨 붙이는 것보다 낫다.

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

/// 기록 관련 줄들. 터미널 페이지에서 부른다.
pub(crate) fn log_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.autolog"));
    ui.checkbox(&mut cfg.terminal.session_log_auto, "")
        .on_hover_text(tr(lang, "settings.autologhint"));
    ui.end_row();
    // 기록을 켜는 항목 바로 아래에 둔다 — "무엇으로 남길지"는 "남길지"의 다음 물음이다.
    ui.label(tr(lang, "settings.logcast"));
    ui.checkbox(&mut cfg.terminal.session_log_cast, "")
        .on_hover_text(tr(lang, "settings.logcast.hint"));
    ui.end_row();
    // 기록에서 비밀번호·토큰을 가리는 일도 기록의 일부다. 따로 떨어져 있으면
    // 기록을 켠 사람이 이것을 못 보고 지나간다.
    ui.label(tr(lang, "settings.redactlogs"));
    ui.checkbox(&mut cfg.terminal.redact_logs, "")
        .on_hover_text(tr(lang, "settings.redactlogshint"));
    ui.end_row();
}
