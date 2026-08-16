//! SSH 키 생성 모달(T1 보안 편의) — ed25519 키쌍을 만들어 파일 저장 + 공개키 복사.
//!
//! 세션 관리 메뉴·팔레트에서 연다. 생성 후 공개키를 바로 보여 주고 복사 버튼을 제공해
//! "서버 authorized_keys에 붙여넣기"까지의 동선을 줄인다. RSA는 제공하지 않는다(ed25519만).

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

/// 모달 상태(Some=열림).
pub struct KeygenState {
    pub path: String,
    pub comment: String,
    /// 생성 완료된 공개키 한 줄(표시·복사용).
    pub done: Option<String>,
}

impl KeygenState {
    pub fn new() -> Self {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        KeygenState {
            path: format!("{home}\\.ssh\\id_ed25519_nabi"),
            comment: format!("{user}@nabiterm"),
            done: None,
        }
    }
}

impl NabiApp {
    pub(crate) fn show_keygen_modal(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.keygen.take() else { return };
        let lang = self.lang;
        let mut open = true;
        let mut notify = None;
        egui::Window::new(tr(lang, "keygen.title"))
            .open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(tr(lang, "keygen.about"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "keygen.path"));
                    ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(320.0));
                });
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "keygen.comment"));
                    ui.add(egui::TextEdit::singleline(&mut st.comment).desired_width(200.0));
                });
                ui.add_space(6.0);
                if ui.button(format!("\u{1f511} {}", tr(lang, "keygen.generate"))).clicked() {
                    match self.do_keygen(&st.path, &st.comment) {
                        Ok(pub_line) => st.done = Some(pub_line),
                        Err(e) => notify = Some(format!("\u{2715} {e}")),
                    }
                }
                if let Some(p) = &st.done {
                    ui.add_space(6.0);
                    ui.colored_label(crate::theme_ui::OK, tr(lang, "keygen.saved"));
                    // 공개키는 비밀이 아니다 — 그대로 보여 주고 복사만 쉽게.
                    ui.add(egui::TextEdit::multiline(&mut p.clone()).desired_rows(2).desired_width(420.0).interactive(false));
                    if ui.button(format!("\u{1f4cb} {}", tr(lang, "keygen.copypub"))).clicked() {
                        ctx.copy_text(p.clone());
                    }
                    ui.weak(tr(lang, "keygen.hint"));
                }
            });
        if let Some(n) = notify {
            self.notify = Some((n, Instant::now()));
        }
        if open {
            self.keygen = Some(st);
        }
    }

    /// 키쌍 생성 + 파일 저장(개인키·.pub). 기존 파일이 있으면 덮어쓰지 않는다(안전).
    fn do_keygen(&mut self, path: &str, comment: &str) -> Result<String, String> {
        let path = path.trim();
        if path.is_empty() {
            return Err(nabi_i18n::tr(self.lang, "keygen.needpath").to_string());
        }
        let p = std::path::Path::new(path);
        if p.exists() {
            return Err(nabi_i18n::tr(self.lang, "keygen.exists").to_string());
        }
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let (pem, pub_line) = nabi_ssh::keygen::generate_ed25519(comment)?;
        std::fs::write(p, pem).map_err(|e| e.to_string())?;
        std::fs::write(format!("{path}.pub"), format!("{pub_line}\n")).map_err(|e| e.to_string())?;
        Ok(pub_line)
    }
}
