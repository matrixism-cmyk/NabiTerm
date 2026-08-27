//! 설정▸AI 터미널 — AI 터미널 프로필 편집 페이지(aiprof.rs의 순수 헬퍼를 쓰는 UI).
//!
//! 프로필 = 이름 + 셸 + CLI + 인자. 알려진 CLI는 스위치 체크박스를, 그 외는 자유 입력을
//! 제공한다. 저장은 전부 args Vec 하나로(SSOT — 체크박스는 편집기일 뿐).

use crate::aiprof::{extra_args_string, preset_switches, replace_stale, set_extra_args, stale_in, toggle_arg, CLI_CHOICES};
use nabi_config::{AiProfileCfg, AppConfig};
use nabi_i18n::{tr, Lang};

impl crate::app::NabiApp {
    /// 프로필 관리 독립창(새 SSH 연결과 같은 패턴).
    ///
    /// 입력 폼에는 **저장·취소 버튼이 있어야 한다**(사용자 지적 2026-08-25 — 그전에는 창을
    /// 닫는 것이 곧 저장이라, 새 프로필을 채워 넣고도 누를 것이 없었다). 창의 X는 대화상자
    /// 관례대로 취소와 같게 동작한다.
    pub(crate) fn show_ai_profiles(&mut self, ctx: &egui::Context) {
        if !self.ai_prof_open {
            return;
        }
        let lang = self.lang;
        // 창을 열 때 원본을 찍어 둔다 — 어느 경로로 열렸든 여기 한 곳에서(SSOT).
        if self.ai_prof_backup.is_none() {
            self.ai_prof_backup = Some(self.config.terminal.ai_profiles.clone());
        }
        let (mut open, mut done) = (true, None);
        egui::Window::new(tr(lang, "settings.sec.aiprof"))
            .id(egui::Id::new("ai_prof_win"))
            .open(&mut open)
            .default_width(440.0)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                    ai_profile_rows(ui, &mut self.config, lang);
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "settings.save")).clicked() {
                        done = Some(true);
                    }
                    if ui.button(tr(lang, "qc.cancel")).clicked() {
                        done = Some(false);
                    }
                });
            });
        let Some(save) = done.or(if open { None } else { Some(false) }) else { return };
        let backup = self.ai_prof_backup.take();
        if !save {
            if let Some(b) = backup {
                self.config.terminal.ai_profiles = b;
            }
        }
        self.ai_prof_open = false;
        self.save_config();
        // 설정 대화상자가 열려 있으면 그 취소 스냅샷에도 반영한다 — 그러지 않으면
        // 설정을 Esc로 닫는 순간 여기서 정한 내용이 되돌려져 사라진다(리뷰 2026-08-19).
        if let Some(b) = self.settings_backup.as_mut() {
            b.terminal.ai_profiles = self.config.terminal.ai_profiles.clone();
        }
    }
}

/// 프로필 **목록 + 선택한 하나의 편집**(마스터-디테일).
///
/// 예전에는 모든 프로필의 편집 폼을 세로로 쌓아 보여 줬다. 프로필이 셋만 넘어가도 화면이
/// 폼의 벽이 되고, 무엇이 있는지 한눈에 보이지 않는다(사용자 지적 2026-08-25 — 목록이 먼저
/// 나오고 그 안에서 등록·수정·삭제가 되어야 상식적이다).
pub(crate) fn ai_profile_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "aiprof.hint"));
    ui.add_space(6.0);

    // 선택한 줄은 egui 임시 메모리에 둔다 — 이 함수는 자기 상태를 갖지 않는다.
    let sid = ui.id().with("aiprof_sel");
    let mut sel: usize = ui.data(|d| d.get_temp(sid)).unwrap_or(0);
    let n = cfg.terminal.ai_profiles.len();
    sel = sel.min(n.saturating_sub(1));

    ui.label(tr(lang, "aiprof.list"));
    let mut remove = None;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().max_height(150.0).id_salt("aiprof_list").show(ui, |ui| {
            if n == 0 {
                ui.weak(tr(lang, "aiprof.empty"));
            }
            for i in 0..n {
                let p = &cfg.terminal.ai_profiles[i];
                let label = format!("{}\u{2003}\u{2014}\u{2003}{}", p.name, summary(p, lang));
                if ui.selectable_label(sel == i, label).clicked() {
                    sel = i;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        if ui.button(tr(lang, "aiprof.add")).clicked() {
            cfg.terminal.ai_profiles.push(AiProfileCfg { name: format!("AI {}", n + 1), ..Default::default() });
            sel = n; // 방금 추가한 것을 바로 편집하게 한다.
        }
        if ui.add_enabled(n > 0, egui::Button::new(tr(lang, "aiprof.remove"))).clicked() {
            remove = Some(sel);
        }
    });
    if let Some(i) = remove {
        cfg.terminal.ai_profiles.remove(i);
        sel = sel.min(cfg.terminal.ai_profiles.len().saturating_sub(1));
    }
    ui.data_mut(|d| d.insert_temp(sid, sel));

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    match cfg.terminal.ai_profiles.get_mut(sel) {
        Some(p) => {
            ui.label(tr(lang, "aiprof.edit"));
            ui.add_space(4.0);
            egui::Frame::group(ui.style()).show(ui, |ui| profile_editor(ui, p, lang, sel));
        }
        None => {
            ui.weak(tr(lang, "aiprof.pick"));
        }
    }
}

/// 목록 줄에 붙는 한 줄 요약 — 무엇을 실행하는 프로필인지 펼쳐 보지 않고 알 수 있게.
fn summary(p: &AiProfileCfg, lang: Lang) -> String {
    if p.cmd.is_empty() {
        return tr(lang, "aiprof.nocmd").to_string();
    }
    let mut s = p.cmd.clone();
    for a in &p.args {
        s.push(' ');
        s.push_str(a);
        if s.chars().count() > 46 {
            s.truncate(s.char_indices().nth(46).map_or(s.len(), |(i, _)| i));
            s.push('\u{2026}');
            break;
        }
    }
    s
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

        // 그 CLI에서 없어진 옵션이 남아 있으면 먼저 알린다 — 그대로 두면 실행 자체가 실패한다.
        // (codex는 0.149에서 --full-auto가 사라졌다. 표에서 지우는 것만으로는 이미 저장된
        // 프로필이 조용히 깨진 채로 남는다.)
        let stale = stale_in(&p.args, &p.cmd);
        if !stale.is_empty() {
            ui.label(egui::RichText::new("\u{26a0}").color(egui::Color32::from_rgb(220, 130, 60)));
            ui.vertical(|ui| {
                ui.colored_label(egui::Color32::from_rgb(220, 130, 60), tr(lang, "aiprof.stale"));
                for (dead, fix, note) in stale {
                    ui.horizontal(|ui| {
                        ui.code(dead);
                        if ui.small_button(tr(lang, "aiprof.stalefix")).clicked() {
                            replace_stale(&mut p.args, dead, fix);
                        }
                    });
                    ui.indent((dead, "n"), |ui| ui.weak(tr(lang, note)));
                }
            });
            ui.end_row();
        }

        // 알려진 CLI의 스위치 체크박스(제품 옵션명 — 번역 없음) + 아래 설명(선택 도움).
        let presets = preset_switches(&p.cmd);
        if !presets.is_empty() {
            ui.label(tr(lang, "aiprof.switches"));
            ui.vertical(|ui| {
                for (sw, desc_key) in presets {
                    let mut on = p.args.iter().any(|a| a == sw);
                    if ui.checkbox(&mut on, *sw).changed() {
                        toggle_arg(&mut p.args, sw, on);
                    }
                    ui.indent((sw, "d"), |ui| ui.weak(tr(lang, desc_key)));
                    ui.add_space(2.0);
                }
            });
            ui.end_row();
        }

        // 자유 인자 한 줄(공백 분리) — 프리셋 체크 상태는 유지된다.
        //
        // 편집 중 텍스트는 egui 임시 메모리에 둔다. args(토큰 목록)에서 매 프레임 다시
        // 만들면 **공백을 칠 수 없다**(“--model ” → join으로 꼬리 공백이 사라져 다음 글자가
        // 붙어버림 — 리뷰 2026-08-19). 외부에서 args가 바뀌면(프로필/CLI 교체) 다시 맞춘다.
        ui.label(tr(lang, "aiprof.extra"));
        let derived = extra_args_string(&p.args, &p.cmd);
        let bid = ui.id().with(("aiprof_extra", i));
        let mut extra: String = ui.data(|d| d.get_temp(bid)).unwrap_or_else(|| derived.clone());
        if extra.split_whitespace().ne(derived.split_whitespace()) {
            extra = derived; // 버퍼와 실제 인자가 어긋남 = 외부 변경 → 버퍼 재동기화.
        }
        if ui.add(egui::TextEdit::singleline(&mut extra).desired_width(260.0).hint_text("--model opus …")).changed() {
            set_extra_args(&mut p.args, &p.cmd, &extra);
        }
        ui.data_mut(|d| d.insert_temp(bid, extra));
        ui.end_row();
    });
}
