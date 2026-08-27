//! 상태바 칩 헬퍼(SSH KEX 배지 등) — statusbar.rs에서 분리(라인 한도).

use nabi_i18n::{tr, Lang};
use nabi_types::PaneId;

/// **기록 중** 배지 — 이 pane의 출력이 파일로 남고 있음을 늘 보여 준다(배치 Y S3).
///
/// 알림은 시작할 때 한 번 뜨고 사라진다. 그런데 `autolog.rs`가 새 창의 로깅을 **저절로**
/// 켜므로, 사용자가 알림을 놓치면 기록되는 줄 모른 채 비밀번호를 칠 수 있다.
/// 몰래 기록되는 것으로 보이면 안 된다 — 그래서 켜져 있는 동안 계속 보인다.
pub(crate) fn rec_badge(ui: &mut egui::Ui, lang: Lang, on: bool, cast: bool) {
    if !on {
        return;
    }
    ui.separator();
    // 붉은 점은 어디서나 "지금 기록 중"으로 읽힌다.
    let label = if cast { "\u{25cf} REC \u{23fa}" } else { "\u{25cf} REC" };
    ui.colored_label(crate::theme_ui::ERR, label)
        .on_hover_text(tr(lang, "status.recording"));
}

/// SSH 배지: 협상 KEX가 PQ(ML-KEM 하이브리드)면 방패 + 상세 툴팁.
pub(crate) fn ssh_badge(ui: &mut egui::Ui, lang: Lang, focused: Option<PaneId>) {
                    // T1-2: 협상된 KEX가 PQ(ML-KEM 하이브리드)면 방패 배지 + 상세 툴팁.
                    let kex = focused.and_then(nabi_ssh::kexinfo::get);
                    let (label, tip) = match &kex {
                        Some(k) => {
                            let detail = format!("{}: {}\n{}: {}", tr(lang, "status.kex"), k.kex, tr(lang, "status.cipher"), k.cipher);
                            if k.is_pq() {
                                ("SSH \u{1f6e1}PQ".to_string(), format!("{}\n{detail}", tr(lang, "status.pq")))
                            } else {
                                ("SSH".to_string(), detail)
                            }
                        }
                        None => ("SSH".to_string(), String::new()),
                    };
                    // 이 연결을 SFTP가 함께 타고 있으면 배지에 고리를 붙인다. 칩을 새로 만들지
                    // 않는 이유: 같은 사실을 두 자리에서 말하면 언젠가 한쪽만 고쳐진다.
                    let riders = focused.map(nabi_ssh::conns::riders).unwrap_or(0);
                    let label = if riders > 0 { format!("{label} \u{1f517}") } else { label };
                    let tip = match riders {
                        0 => tip,
                        n => {
                            let line = format!("{} ({n})", tr(lang, "status.shared"));
                            if tip.is_empty() { line } else { format!("{tip}\n{line}") }
                        }
                    };
                    let color = if kex.as_ref().is_some_and(|k| k.is_pq()) { crate::theme_ui::OK } else { crate::theme_ui::ACCENT };
                    let r = ui.colored_label(color, label);
                    if !tip.is_empty() { r.on_hover_text(tip); }
}
