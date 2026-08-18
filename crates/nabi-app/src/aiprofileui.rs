//! 설정▸AI 터미널 — AI 터미널 프로필 편집 페이지(aiprof.rs의 순수 헬퍼를 쓰는 UI).
//!
//! 프로필 = 이름 + 셸 + CLI + 인자. 알려진 CLI는 스위치 체크박스를, 그 외는 자유 입력을
//! 제공한다. 저장은 전부 args Vec 하나로(SSOT — 체크박스는 편집기일 뿐).

use crate::aiprof::{extra_args_string, preset_switches, set_extra_args, toggle_arg, CLI_CHOICES};
use nabi_config::{AiProfileCfg, AppConfig};
use nabi_i18n::{tr, Lang};

pub(crate) fn ai_profile_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "aiprof.hint"));
    ui.add_space(4.0);
    let mut remove = None;
    let n = cfg.terminal.ai_profiles.len();
    for i in 0..n {
        let p = &mut cfg.terminal.ai_profiles[i];
        egui::Frame::group(ui.style()).show(ui, |ui| {
            profile_editor(ui, p, lang, i);
            if ui.small_button(tr(lang, "aiprof.remove")).clicked() {
                remove = Some(i);
            }
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        cfg.terminal.ai_profiles.remove(i);
    }
    if ui.button(tr(lang, "aiprof.add")).clicked() {
        let n = cfg.terminal.ai_profiles.len() + 1;
        cfg.terminal.ai_profiles.push(AiProfileCfg { name: format!("AI {n}"), ..Default::default() });
    }
}

/// 프로필 한 개의 편집 폼(이름·셸·CLI·스위치·추가 인자).
fn profile_editor(ui: &mut egui::Ui, p: &mut AiProfileCfg, lang: Lang, i: usize) {
    egui::Grid::new(("aiprof", i)).num_columns(2).min_col_width(90.0).spacing([16.0, 6.0]).show(ui, |ui| {
        ui.label(tr(lang, "aiprof.name"));
        ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(200.0));
        ui.end_row();

        ui.label(tr(lang, "aiprof.shell"));
        let shell_label = if p.shell.is_empty() { tr(lang, "aiprof.shell.default").to_string() } else { p.shell.clone() };
        egui::ComboBox::from_id_salt(("aiprof_shell", i)).selected_text(shell_label).show_ui(ui, |ui| {
            ui.selectable_value(&mut p.shell, String::new(), tr(lang, "aiprof.shell.default"));
            for s in ["powershell", "pwsh", "cmd", "wsl", "gitbash"] {
                ui.selectable_value(&mut p.shell, s.to_string(), s);
            }
        });
        ui.end_row();

        // CLI 종류 — 목록에 없는 명령이 저장돼 있으면 "custom"으로 표시.
        ui.label(tr(lang, "aiprof.cmd"));
        ui.horizontal(|ui| {
            let known = CLI_CHOICES[..CLI_CHOICES.len() - 1].contains(&p.cmd.as_str());
            let sel = if known { p.cmd.clone() } else { "custom".to_string() };
            egui::ComboBox::from_id_salt(("aiprof_cmd", i)).selected_text(sel.clone()).show_ui(ui, |ui| {
                for c in CLI_CHOICES {
                    if ui.selectable_label(sel == c, c).clicked() {
                        if c == "custom" {
                            if known {
                                p.cmd = String::new(); // known→custom 전환: 직접 입력 시작.
                            }
                        } else {
                            p.cmd = c.to_string();
                        }
                    }
                }
            });
            if !CLI_CHOICES[..CLI_CHOICES.len() - 1].contains(&p.cmd.as_str()) {
                ui.add(egui::TextEdit::singleline(&mut p.cmd).desired_width(140.0).hint_text("agy ..."));
            }
        });
        ui.end_row();

        // 알려진 CLI의 스위치 체크박스(제품 옵션명 — 번역 없음).
        let presets = preset_switches(&p.cmd);
        if !presets.is_empty() {
            ui.label(tr(lang, "aiprof.switches"));
            ui.vertical(|ui| {
                for sw in presets {
                    let mut on = p.args.iter().any(|a| a == sw);
                    if ui.checkbox(&mut on, *sw).changed() {
                        toggle_arg(&mut p.args, sw, on);
                    }
                }
            });
            ui.end_row();
        }

        // 자유 인자 한 줄(공백 분리) — 프리셋 체크 상태는 유지된다.
        ui.label(tr(lang, "aiprof.extra"));
        let mut extra = extra_args_string(&p.args, &p.cmd);
        if ui.add(egui::TextEdit::singleline(&mut extra).desired_width(260.0).hint_text("--model opus …")).changed() {
            set_extra_args(&mut p.args, &p.cmd, &extra);
        }
        ui.end_row();
    });
}
