//! 상태바 칩 헬퍼(SSH KEX 배지 등) — statusbar.rs에서 분리(라인 한도).

use nabi_i18n::{tr, Lang};
use nabi_types::PaneId;

/// **누르면 기록을 멈춘다**(배치 AK). 켜는 길은 메뉴에 있는데 끄는 길은 거기까지 찾아
/// 들어가야 했다. 기록을 멈추고 싶은 순간은 대개 급하다 — 비밀번호를 치기 직전이다.
/// 그래서 지금 눈에 보이는 그 자리에서 바로 끌 수 있게 한다.
///
/// 눌렀는지 돌려준다. 실제로 멈추는 일은 부르는 쪽이 한다(여기는 화면만 그린다).
pub(crate) fn rec_badge(ui: &mut egui::Ui, lang: Lang, on: bool, cast: bool) -> bool {
    if !on {
        return false;
    }
    ui.separator();
    // 붉은 점은 어디서나 "지금 기록 중"으로 읽힌다.
    let label = if cast { "\u{25cf} REC \u{23fa}" } else { "\u{25cf} REC" };
    let tip = format!("{}\n{}", tr(lang, "status.recording"), tr(lang, "status.recstop"));
    ui.add(egui::Label::new(egui::RichText::new(label).color(crate::theme_ui::ERR)).sense(egui::Sense::click()))
        .on_hover_text(tip)
        .clicked()
}

/// 이 pane 에서 **실패한 명령이 몇 개**인지. 누르면 그 자리로 간다.
///
/// 실패 지점으로 건너뛰는 길은 전부터 있었는데(`jump_failed_prompt`), 건너뛸 것이
/// 있는지를 알 길이 없었다 — 눌러 보고 "실패한 명령이 없습니다"를 읽어야 알았다.
/// 개수를 세는 함수도 이미 있었고 주석에 "화면에 개수를 보여 주려면 필요하다"고
/// 적혀 있었는데 아무도 부르지 않았다(`xtask unused` 로 찾았다).
///
/// 0 이면 아무것도 그리지 않는다. 잘 되고 있을 때 눈에 걸리적거릴 이유가 없다.
///
/// 눌렀는지 돌려준다 — 실제로 옮기는 일은 부르는 쪽이 한다.
pub(crate) fn failed_badge(ui: &mut egui::Ui, lang: Lang, n: usize) -> bool {
    if n == 0 {
        return false;
    }
    ui.separator();
    let label = egui::RichText::new(format!("\u{2717} {n}")).color(crate::theme_ui::ERR);
    ui.add(egui::Label::new(label).sense(egui::Sense::click()))
        .on_hover_text(format!("{} \u{2014} {}", tr(lang, "status.failed"), tr(lang, "status.failedjump")))
        .clicked()
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
