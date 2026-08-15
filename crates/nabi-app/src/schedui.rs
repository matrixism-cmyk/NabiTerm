//! 스케줄 설정 페이지(C3) — 잡 목록(토글/삭제) + 추가 폼. 설정 창(free fn)에서 그린다.

use crate::scheduler::Job;
use nabi_i18n::{tr, Lang};

/// 추가 폼의 임시 상태(설정 창 생명주기 — egui temp data).
#[derive(Clone, Default)]
struct Draft {
    name: String,
    spec: String,
    kind_idx: usize, // 0=send 1=command 2=notify
    payload: String,
    pane_title: String,
    error: String,
}

const KINDS: [&str; 3] = ["send", "command", "notify"];

/// 설정 ▸ 스케줄 페이지 본문. jobs 변경은 즉시 파일에 저장한다.
pub(crate) fn schedule_rows(
    ui: &mut egui::Ui,
    lang: Lang,
    jobs: &std::cell::RefCell<Vec<Job>>,
    path: &std::path::Path,
) {
    let mut list = jobs.borrow_mut();
    let mut delete: Option<usize> = None;
    let mut changed = false;
    for (i, job) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut job.enabled, "").changed() {
                job.fails = 0; // 다시 켜면 실패 카운트 리셋(재시도 기회).
                changed = true;
            }
            ui.monospace(format!("[{}] {}", job.spec, job.name));
            ui.weak(format!("{} \u{2192} {}", job.kind, crate::humanfmt::ellipsis(&job.payload, 40)));
            if job.fails > 0 {
                ui.colored_label(crate::theme_ui::ERR, format!("\u{2715}{}", job.fails));
            }
            if ui.small_button("\u{1f5d1}").clicked() {
                delete = Some(i);
            }
        });
    }
    if let Some(i) = delete {
        list.remove(i);
        changed = true;
    }
    if changed {
        crate::scheduler::save(path, &list);
    }
    ui.separator();

    let id = egui::Id::new("sched_draft");
    let mut d = ui.data(|s| s.get_temp::<Draft>(id)).unwrap_or_default();
    ui.strong(tr(lang, "sched.add"));
    egui::Grid::new("sched_add").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(tr(lang, "sched.name"));
        ui.text_edit_singleline(&mut d.name);
        ui.end_row();
        ui.label(tr(lang, "sched.spec"));
        ui.add(egui::TextEdit::singleline(&mut d.spec).hint_text("*/30 * * * *  |  every 15m  |  at 09:30"));
        ui.end_row();
        ui.label(tr(lang, "sched.kind"));
        ui.horizontal(|ui| {
            for (i, k) in KINDS.iter().enumerate() {
                let key = format!("sched.kind.{k}");
                if ui.selectable_label(d.kind_idx == i, tr(lang, &key)).clicked() {
                    d.kind_idx = i;
                }
            }
        });
        ui.end_row();
        ui.label(tr(lang, "sched.payload"));
        ui.add(egui::TextEdit::singleline(&mut d.payload).desired_width(300.0));
        ui.end_row();
        if d.kind_idx == 0 {
            ui.label(tr(lang, "sched.pane"));
            ui.add(egui::TextEdit::singleline(&mut d.pane_title).hint_text(tr(lang, "sched.pane.hint")));
            ui.end_row();
        }
    });
    if !d.error.is_empty() {
        ui.colored_label(crate::theme_ui::ERR, &d.error);
    }
    if ui.button(tr(lang, "sched.create")).clicked() {
        let name = if d.name.trim().is_empty() { d.spec.clone() } else { d.name.clone() };
        match crate::scheduler::add(&mut list, path, name, d.spec.clone(), KINDS[d.kind_idx].into(), d.payload.clone(), d.pane_title.clone()) {
            Ok(()) => d = Draft::default(),
            Err(e) => d.error = e,
        }
    }
    ui.data_mut(|s| s.insert_temp(id, d));
}
