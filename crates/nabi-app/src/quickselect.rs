//! Quick Select(E4) — 보이는 화면에서 URL/이메일/IP/해시/경로 등 토큰을 추출해 클릭 복사.
//! 마우스 드래그보다 빠르게 자주 쓰는 값(커밋 해시·IP·경로)을 집어 복사한다. 순수 추출+팝업.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 화면 텍스트에서 흔한 토큰을 추출한다(등장 순서·중복 제거). 순수 함수(단위테스트).
pub(crate) fn find_tokens(text: &str) -> Vec<String> {
    // URL / 이메일 / IPv4 / git 해시(7~40 hex) / 절대경로(win·unix).
    const PATS: [&str; 5] = [
        r"https?://[^\s'\x22<>)]+",
        r"[\w.+-]+@[\w-]+\.[\w.-]+",
        r"\b\d{1,3}(?:\.\d{1,3}){3}\b",
        r"\b[0-9a-f]{7,40}\b",
        r"(?:[A-Za-z]:\\|/)[\w./\\-]{3,}",
    ];
    let mut out: Vec<String> = Vec::new();
    for p in PATS {
        if let Ok(re) = regex::Regex::new(p) {
            for m in re.find_iter(text) {
                let t = m.as_str().trim_end_matches(['.', ',', ')']).to_string();
                if t.len() >= 4 && !out.contains(&t) {
                    out.push(t);
                }
            }
        }
    }
    out
}

impl NabiApp {
    /// Quick Select 팝업(팔레트에서 토글) — 화면 토큰 목록, 클릭 시 클립보드 복사.
    pub(crate) fn quick_select_popup(&mut self, ctx: &egui::Context) {
        if !self.quick_select_open {
            return;
        }
        let lang = self.lang;
        let tokens = self.focused_pane()
            .and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()))
            .and_then(|v| v.model.lock().ok().map(|md| find_tokens(&md.visible_text(200))))
            .unwrap_or_default();
        let (mut open, mut copied) = (true, None);
        egui::Window::new(tr(lang, "qsel.title"))
            .open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .show(ctx, |ui| {
                if tokens.is_empty() {
                    ui.label(tr(lang, "qsel.none"));
                }
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for t in &tokens {
                        if ui.selectable_label(false, t).clicked() {
                            copied = Some(t.clone());
                        }
                    }
                });
            });
        if let Some(t) = copied {
            ctx.copy_text(t);
            self.quick_select_open = false;
            self.notify = Some((tr(lang, "qsel.copied").to_string(), std::time::Instant::now()));
        } else if !open {
            self.quick_select_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_tokens;

    #[test]
    fn extracts_common_tokens() {
        let t = find_tokens("clone https://x.io/a.git at 10.0.0.1 sha deadbeef1234 file /etc/hosts");
        assert!(t.iter().any(|s| s == "https://x.io/a.git"));
        assert!(t.iter().any(|s| s == "10.0.0.1"));
        assert!(t.iter().any(|s| s == "deadbeef1234"));
        assert!(t.iter().any(|s| s == "/etc/hosts"));
        // 중복 제거.
        let d = find_tokens("10.0.0.1 10.0.0.1");
        assert_eq!(d.iter().filter(|s| *s == "10.0.0.1").count(), 1);
    }
}
