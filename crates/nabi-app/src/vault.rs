//! 자격증명 볼트 UI 연동(잠금 해제 + 저장/조회).
//!
//! 크립토(Argon2id+AES-256-GCM)는 nabi-secret에서 단위 검증됨. 여기서는 master
//! password 잠금 해제와 vault.bin 영속, SSH 비밀 저장/조회만 담당한다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 시작 시 OS 자격증명에 저장된 마스터 비밀번호로 볼트를 자동 잠금 해제한다(F1, opt-in).
/// vault_remember가 켜져 있고 저장된 비밀번호로 복호되면 (볼트, 비밀번호)를 돌려준다.
pub(crate) fn auto_unlock(
    config: &nabi_config::AppConfig,
    vault_path: &std::path::Path,
) -> (Option<nabi_secret::Vault>, Option<String>) {
    if !config.terminal.vault_remember || !vault_path.exists() {
        return (None, None);
    }
    let Some(pw) = nabi_secret::keyringstore::load_master() else { return (None, None) };
    match std::fs::read(vault_path).ok().and_then(|b| nabi_secret::open_vault(&b, &pw).ok()) {
        Some(v) => (Some(v), Some(pw)),
        None => (None, None),
    }
}

impl NabiApp {
    pub(crate) fn show_vault_unlock(&mut self, ctx: &egui::Context) {
        if !self.vault_unlock_open {
            return;
        }
        let lang = self.lang;
        let exists = self.vault_path.exists();
        let unlocked = self.vault.is_some();
        let count = self.vault.as_ref().map(|v| v.entries.len()).unwrap_or(0);
        let mut open = true;
        let mut unlock = false;
        let mut reset = false;
        let mut lock = false;
        let mut close = false;
        let was_remember = self.config.terminal.vault_remember;
        let mut remember = was_remember;
        {
            let pw = &mut self.vault_pw_input;
            let status = self.vault_status.clone();
            egui::Window::new(tr(lang, "vault.title"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(tr(lang, "vault.about"));
                    ui.separator();
                    if unlocked {
                        // 이미 잠금 해제됨 → 상태 표시 + 잠금 버튼.
                        ui.label(format!("{} ({})", tr(lang, "vault.unlocked"), count));
                        ui.horizontal(|ui| {
                            if ui.button(tr(lang, "vault.lock")).clicked() {
                                lock = true;
                            }
                            if ui.button(tr(lang, "qc.cancel")).clicked() {
                                close = true;
                            }
                        });
                        return;
                    }
                    // 잠겨 있음 → 비밀번호 입력(없으면 "첫 사용=생성").
                    let hint = if exists { "vault.prompt" } else { "vault.firstuse" };
                    ui.label(tr(lang, hint));
                    let resp = ui.add(egui::TextEdit::singleline(pw).password(true));
                    if resp.has_focus() {
                        // 비밀번호는 영문 전제 — 포커스 동안 IME 비활성(한글 조합 끔).
                        ui.output_mut(|o| o.ime = None);
                    }
                    // Enter로 즉시 잠금 해제(생성).
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        unlock = true;
                    }
                    if !status.is_empty() {
                        ui.colored_label(egui::Color32::LIGHT_RED, status);
                    }
                    // 비밀번호 기억(OS 자격증명) — 편의↑·보안↓ 절충(F1).
                    ui.checkbox(&mut remember, tr(lang, "vault.remember"))
                        .on_hover_text(tr(lang, "vault.remember.warn"));
                    ui.horizontal(|ui| {
                        let label = if exists { "vault.unlock" } else { "vault.create" };
                        if ui.button(tr(lang, label)).clicked() {
                            unlock = true;
                        }
                        if exists && ui.button(tr(lang, "vault.reset")).clicked() {
                            reset = true;
                        }
                        if ui.button(tr(lang, "qc.cancel")).clicked() {
                            close = true;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                });
        }
        // 비밀번호 기억 토글이 바뀌면 저장 + 끄면 OS 자격증명에서 즉시 삭제.
        if remember != was_remember {
            self.config.terminal.vault_remember = remember;
            self.save_config();
            if !remember {
                nabi_secret::keyringstore::clear_master();
            }
        }
        if unlock {
            self.try_unlock_vault();
            // 성공(잠금 해제됨)했을 때만 닫는다 — 이미 해제된 상태로 재오픈해도 안 닫히게.
            if self.vault.is_some() {
                open = false;
                // 기억 옵션이 켜져 있으면 마스터 비밀번호를 OS 자격증명에 저장(F1).
                if remember {
                    if let Some(pw) = &self.vault_password {
                        nabi_secret::keyringstore::store_master(pw);
                    }
                }
            }
        }
        if reset {
            self.reset_vault();
            nabi_secret::keyringstore::clear_master(); // 볼트 삭제 시 저장된 비밀번호도 제거.
        }
        if lock {
            self.vault = None;
            self.vault_password = None;
            self.vault_status.clear();
        }
        if close {
            open = false;
        }
        self.vault_unlock_open = open;
    }

    /// 볼트 파일을 삭제하고 상태를 초기화한다(비밀번호 분실 시 복구용 — 저장 비밀은 소실).
    fn reset_vault(&mut self) {
        let _ = std::fs::remove_file(&self.vault_path);
        self.vault = None;
        self.vault_password = None;
        self.vault_pw_input.clear();
        self.vault_status = tr(self.lang, "vault.reset_done").to_owned();
    }

    fn try_unlock_vault(&mut self) {
        let pw = self.vault_pw_input.clone();
        if pw.is_empty() {
            self.vault_status = tr(self.lang, "vault.wrong").to_owned();
            return;
        }
        let result = match std::fs::read(&self.vault_path) {
            Ok(bytes) => nabi_secret::open_vault(&bytes, &pw),
            Err(_) => Ok(nabi_secret::Vault::default()),
        };
        match result {
            Ok(v) => {
                self.vault = Some(v);
                self.vault_password = Some(pw);
                self.vault_pw_input.clear();
                self.vault_status.clear();
                self.persist_vault();
            }
            Err(_) => self.vault_status = tr(self.lang, "vault.wrong").to_owned(),
        }
    }

    /// 볼트를 디스크에 쓴다.
    ///
    /// ## 실패를 삼키지 않는다
    ///
    /// 예전에는 봉인도 쓰기도 `let _ =` 였다. 그래서 디스크가 차거나 권한이 막히면
    /// **비밀번호를 저장했다고 믿는데 실제로는 안 저장된다.** 다음에 켜면 없다.
    /// 자격증명은 없어진 것을 나중에 알수록 손해가 큰 종류다(2026-08-30 메뉴 전수 점검).
    fn persist_vault(&mut self) {
        let Some(msg) = self.persist_vault_result().err() else { return };
        self.vault_status = msg;
    }

    /// 실제로 쓰는 부분 — 실패하면 이유를 돌려준다.
    fn persist_vault_result(&self) -> Result<(), String> {
        let (Some(v), Some(pw)) = (&self.vault, &self.vault_password) else {
            return Ok(()); // 잠겨 있으면 쓸 것이 없다.
        };
        let bytes = nabi_secret::seal_vault(v, pw).map_err(|e| format!("\u{2715} {e}"))?;
        if let Some(parent) = self.vault_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("\u{2715} {}: {e}", parent.display()))?;
        }
        std::fs::write(&self.vault_path, bytes)
            .map_err(|e| format!("\u{2715} {}: {e}", self.vault_path.display()))
    }

    /// 비밀을 볼트에 저장한다(잠금 해제 상태에서만). 저장되면 true.
    pub(crate) fn vault_store(&mut self, key: &str, secret: &str) -> bool {
        match self.vault.as_mut() {
            Some(v) => v.set(key, secret),
            None => return false,
        }
        self.persist_vault();
        true
    }

    /// 볼트에서 비밀을 조회한다.
    pub(crate) fn vault_get(&self, key: &str) -> Option<String> {
        self.vault
            .as_ref()
            .and_then(|v| v.get(key).map(str::to_owned))
    }
}
