//! 상태바 칩 헬퍼(SSH KEX 배지 등) — statusbar.rs에서 분리(라인 한도).

use nabi_i18n::{tr, Lang};
use nabi_types::PaneId;

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
                    let color = if kex.as_ref().is_some_and(|k| k.is_pq()) { crate::theme_ui::OK } else { crate::theme_ui::ACCENT };
                    let r = ui.colored_label(color, label);
                    if !tip.is_empty() { r.on_hover_text(tip); }
}
