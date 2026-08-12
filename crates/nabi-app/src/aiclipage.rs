//! AI CLI 관리 표 — 설치·제거에 더해 배포된 최신 버전과 견주어 업데이트를 안내한다.
//!
//! 최신 버전 조회는 네트워크라 UI 스레드에서 하지 않는다([`crate::aicliupd::start_latest_check`]).
//! 조회 결과가 오기 전에도 표는 그대로 쓸 수 있어야 하므로 "확인 중"만 표시하고 넘어간다.

use nabi_i18n::Lang;

/// 이 페이지에서 쓰는 낱말(파일 안에서만 쓰는 라벨이라 카탈로그 대신 여기 둔다).
struct Words {
    title: &'static str,
    missing: &'static str,
    refresh: &'static str,
    install: &'static str,
    remove: &'static str,
    update: &'static str,
    latest: &'static str,
    checking: &'static str,
    up_to_date: &'static str,
    auto: &'static str,
    auto_hint: &'static str,
}

fn words(lang: Lang) -> Words {
    match lang {
        Lang::Ko => Words {
            title: "AI CLI 관리",
            missing: "미설치",
            refresh: "새로고침",
            install: "설치",
            remove: "제거",
            update: "업데이트",
            latest: "최신",
            checking: "확인 중…",
            up_to_date: "최신 상태",
            auto: "자동 업데이트",
            auto_hint: "켜면 시작할 때 하루 한 번 확인하고 새 버전이 있으면 조용히 올립니다.",
        },
        Lang::Ja => Words {
            title: "AI CLI 管理",
            missing: "未インストール",
            refresh: "更新",
            install: "インストール",
            remove: "削除",
            update: "アップデート",
            latest: "最新",
            checking: "確認中…",
            up_to_date: "最新です",
            auto: "自動アップデート",
            auto_hint: "起動時に一日一回確認し、新しい版があれば自動で更新します。",
        },
        Lang::En => Words {
            title: "AI CLI manager",
            missing: "Not installed",
            refresh: "Refresh",
            install: "Install",
            remove: "Remove",
            update: "Update",
            latest: "Latest",
            checking: "Checking…",
            up_to_date: "Up to date",
            auto: "Auto-update",
            auto_hint: "Checks once a day at startup and quietly installs newer versions.",
        },
    }
}

const KEY: &str = "ai_cli_status";
const JOB: &str = "ai_cli_job";
const LATEST: &str = "ai_cli_latest";

/// 설치된 AI CLI와 버전을 보여주고 설치·제거·업데이트를 실행한다.
///
/// `auto`를 바꾸면 true를 돌려준다 — 호출부가 설정을 저장하도록.
pub(crate) fn ai_cli_manager(ui: &mut egui::Ui, lang: Lang, auto: &mut bool) -> bool {
    let w = words(lang);
    let (key, job_key, latest_key) =
        (egui::Id::new(KEY), egui::Id::new(JOB), egui::Id::new(LATEST));
    let mut statuses = ui.ctx().data(|d| d.get_temp::<Vec<crate::aicli::CliStatus>>(key));
    let job = ui.ctx().data(|d| d.get_temp::<crate::aicli::ActionJob>(job_key));
    let busy = job.as_ref().and_then(|j| j.lock().ok()).is_some_and(|p| !p.done);
    ui.horizontal(|ui| {
        ui.strong(w.title);
        if ui.small_button(format!("\u{21bb} {}", w.refresh)).clicked() {
            statuses = None;
            ui.ctx().data_mut(|d| d.remove::<crate::aicliupd::LatestJob>(latest_key));
        }
    });
    let statuses = statuses.unwrap_or_else(|| {
        let v = crate::aicli::detect_all();
        ui.ctx().data_mut(|d| d.insert_temp(key, v.clone()));
        v
    });
    let latest = latest_versions(ui, latest_key, &statuses);
    table(ui, &w, &statuses, latest.as_deref(), busy, job_key);
    ui.add_space(6.0);
    let changed = ui.checkbox(auto, w.auto).on_hover_text(w.auto_hint).changed();
    progress(ui, job, key);
    changed
}

/// 최신 버전 조회 결과를 얻는다. 아직 없으면 조회를 시작하고 None(=확인 중).
fn latest_versions(
    ui: &mut egui::Ui,
    latest_key: egui::Id,
    statuses: &[crate::aicli::CliStatus],
) -> Option<Vec<(String, String)>> {
    let job = ui.ctx().data(|d| d.get_temp::<crate::aicliupd::LatestJob>(latest_key));
    let job = job.unwrap_or_else(|| {
        let ids = statuses.iter().filter(|c| c.installed()).map(|c| c.id.to_string()).collect();
        let j = crate::aicliupd::start_latest_check(ids);
        ui.ctx().data_mut(|d| d.insert_temp(latest_key, j.clone()));
        j
    });
    let done = job.lock().ok().and_then(|g| g.clone());
    if done.is_none() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }
    done
}

/// 한 CLI의 최신 버전 칸에 무엇을 쓸지 — (문구, 색, 업데이트 필요 여부).
fn latest_cell(
    w: &Words,
    cli: &crate::aicli::CliStatus,
    latest: Option<&[(String, String)]>,
) -> (String, Option<egui::Color32>, bool) {
    let Some(map) = latest else {
        return (w.checking.to_string(), None, false);
    };
    let Some((_, ver)) = map.iter().find(|(id, _)| id == cli.id) else {
        return ("\u{2014}".into(), None, false); // 조회 대상이 아닌 CLI(수동 관리).
    };
    let installed = cli.version.as_deref().unwrap_or("");
    if crate::aicliver::is_outdated(installed, ver) {
        (format!("{} {ver}", w.latest), Some(crate::theme_ui::ACCENT), true)
    } else {
        (w.up_to_date.to_string(), None, false)
    }
}

fn table(
    ui: &mut egui::Ui,
    w: &Words,
    statuses: &[crate::aicli::CliStatus],
    latest: Option<&[(String, String)]>,
    busy: bool,
    job_key: egui::Id,
) {
    egui::Grid::new("ai_cli_manager")
        .num_columns(5)
        .spacing([12.0, 5.0])
        .striped(true)
        .show(ui, |ui| {
            for cli in statuses {
                ui.strong(cli.name);
                if !cli.installed() {
                    ui.weak(w.missing);
                    ui.label("");
                    ui.monospace(cli.command);
                    if small(ui, w.install, !busy) {
                        start(ui, job_key, crate::aicli::start_action(cli.id, false));
                    }
                    ui.end_row();
                    continue;
                }
                ui.colored_label(
                    crate::theme_ui::OK,
                    cli.version.as_deref().unwrap_or("Installed"),
                )
                .on_hover_text(cli.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
                let (text, color, outdated) = latest_cell(w, cli, latest);
                match color {
                    Some(c) => ui.colored_label(c, text),
                    None => ui.weak(text),
                };
                ui.monospace(cli.command);
                ui.horizontal(|ui| {
                    if outdated && small(ui, w.update, !busy) {
                        let path = cli.path.clone().unwrap_or_default();
                        start(ui, job_key, crate::aicliupd::start_update(cli.id, path));
                    }
                    if small(ui, w.remove, !busy) {
                        start(ui, job_key, crate::aicli::start_action(cli.id, true));
                    }
                });
                ui.end_row();
            }
        });
}

fn small(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label).small()).clicked()
}

fn start(ui: &mut egui::Ui, job_key: egui::Id, job: std::io::Result<crate::aicli::ActionJob>) {
    if let Ok(j) = job {
        ui.ctx().data_mut(|d| d.insert_temp(job_key, j));
    }
}

/// 진행 중인 작업의 진행률/결과를 그리고, 끝났으면 목록을 한 번 다시 읽게 한다.
fn progress(ui: &mut egui::Ui, job: Option<crate::aicli::ActionJob>, key: egui::Id) {
    let Some(job) = job else { return };
    let mut refresh = false;
    if let Ok(mut p) = job.lock() {
        ui.add(egui::ProgressBar::new(p.fraction).show_percentage().text(&p.message));
        if p.done {
            let color = if p.success { crate::theme_ui::OK } else { crate::theme_ui::ERR };
            ui.colored_label(color, &p.message);
            if !p.refresh_done {
                p.refresh_done = true;
                refresh = true;
            }
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    if refresh {
        ui.ctx().data_mut(|d| d.remove::<Vec<crate::aicli::CliStatus>>(key));
        // 버전이 바뀌었을 테니 최신 버전 비교도 다시 한다.
        ui.ctx().data_mut(|d| d.remove::<crate::aicliupd::LatestJob>(egui::Id::new(LATEST)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(version: &str) -> crate::aicli::CliStatus {
        crate::aicli::CliStatus {
            id: "codex",
            name: "OpenAI Codex",
            command: "codex",
            path: Some(std::path::PathBuf::from("codex.cmd")),
            version: Some(version.to_string()),
        }
    }

    #[test]
    fn shows_update_only_when_behind() {
        let w = words(Lang::Ko);
        let map = vec![("codex".to_string(), "0.148.0".to_string())];
        let (_, _, outdated) = latest_cell(&w, &cli("codex-cli 0.147.0"), Some(&map));
        assert!(outdated, "낮은 버전이면 업데이트를 권해야 한다");
        let (_, _, outdated) = latest_cell(&w, &cli("codex-cli 0.148.0"), Some(&map));
        assert!(!outdated, "같은 버전이면 권하지 않는다");
    }

    /// 조회 전/조회 대상 아님은 둘 다 업데이트를 권하지 않는다(모르면 건드리지 않는다).
    #[test]
    fn unknown_latest_never_prompts() {
        let w = words(Lang::Ko);
        let (text, _, outdated) = latest_cell(&w, &cli("0.147.0"), None);
        assert_eq!(text, w.checking);
        assert!(!outdated);
        let (_, _, outdated) = latest_cell(&w, &cli("0.147.0"), Some(&[]));
        assert!(!outdated);
    }
}
