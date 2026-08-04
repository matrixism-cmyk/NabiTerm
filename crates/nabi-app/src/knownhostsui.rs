//! 알려진 호스트(known_hosts) 관리 대화상자(C3) — 저장된 호스트키 항목을 보고 삭제한다.
//! known_hosts는 OpenSSH 형식(줄당 `host[:port] keytype base64`)이라 줄 단위로 다룬다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn show_known_hosts(&mut self, ctx: &egui::Context) {
        if !self.known_hosts_open {
            return;
        }
        let lang = self.lang;
        let path = self.known_hosts_path.clone();
        let lines: Vec<String> = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        let (mut open, mut delete, mut dedupe) = (true, None::<usize>, false);
        egui::Window::new(tr(lang, "knownhosts.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.weak(path.to_string_lossy());
                    if ui.button(tr(lang, "knownhosts.dedupe")).clicked() {
                        dedupe = true;
                    }
                });
                ui.separator();
                // 빈 줄·주석(#)을 뺀 실제 항목만, 원래 줄 인덱스를 유지해 정확히 삭제.
                let entries: Vec<(usize, &String)> = lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                    .collect();
                if entries.is_empty() {
                    ui.label(tr(lang, "knownhosts.empty"));
                    return;
                }
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    for (i, line) in &entries {
                        ui.horizontal(|ui| {
                            // 호스트/키타입 표시는 nabi_ssh 파서로 일원화(해시 등 비표준은 원문 폴백).
                            let (host, kind) = nabi_ssh::parse_known_hosts_line(line)
                                .map(|e| (e.hosts.join(","), e.key_type))
                                .unwrap_or_else(|| (line.split_whitespace().next().unwrap_or("").to_string(), String::new()));
                            if ui.button("\u{1f5d1}").on_hover_text(tr(lang, "knownhosts.delete")).clicked() {
                                delete = Some(*i);
                            }
                            ui.monospace(host);
                            ui.weak(kind);
                        });
                    }
                });
            });
        if let Some(di) = delete {
            let kept: Vec<&str> = lines.iter().enumerate().filter(|(i, _)| *i != di).map(|(_, l)| l.as_str()).collect();
            let body = if kept.is_empty() { String::new() } else { format!("{}\n", kept.join("\n")) };
            let _ = std::fs::write(&path, body);
            self.notify = Some((tr(lang, "knownhosts.deleted").to_string(), std::time::Instant::now()));
        } else if dedupe {
            let deduped = nabi_ssh::known_hosts_dedupe(&lines.join("\n"));
            let _ = std::fs::write(&path, format!("{deduped}\n"));
            self.notify = Some((tr(lang, "knownhosts.dedupe").to_string(), std::time::Instant::now()));
        }
        self.known_hosts_open = open;
    }
}
