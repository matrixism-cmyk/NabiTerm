//! 비밀 찾기의 화면 쪽 — 찾는 일은 `secretscan`, 규칙은 `redact`가 한다.
//!
//! **아무것도 막지 않는다.** 결과는 알림 한 줄이고, 줄 번호를 함께 보여 준다. 몇 줄인지·
//! 어디인지 알면 사용자가 판단할 수 있다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 이 문서에서 비밀로 보이는 줄을 찾아 알린다.
    pub(crate) fn find_secrets_in_doc(&mut self, pane: PaneId) {
        let Some(doc) = self.editors.get(&pane) else { return };
        // 대용량 편집기(rope)는 문자열로 펼치지 않는다 — 그 자체가 이 편집기의 존재 이유다.
        let text = match doc.edit.as_ref() {
            Some(eb) => eb.rope.to_string(),
            None => doc.text.clone(),
        };
        let found = crate::secretscan::scan(&text);
        let msg = match found.is_empty() {
            true => tr(self.lang, "secret.none").to_string(),
            false => {
                // 줄 번호를 몇 개만 보여 준다 — 전부 늘어놓으면 알림이 화면을 덮는다.
                let head: Vec<String> = found.lines.iter().take(5).map(|n| n.to_string()).collect();
                let more = found.lines.len().saturating_sub(head.len());
                let tail = if more > 0 { format!(" +{more}") } else { String::new() };
                format!("{} {} \u{b7} {}{tail}", tr(self.lang, "secret.found"), found.lines.len(), head.join(", "))
            }
        };
        let msg = match found.truncated {
            true => format!("{msg} \u{b7} {}", tr(self.lang, "secret.partial")),
            false => msg,
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

impl NabiApp {
    /// 올리려는 파일들에 비밀로 보이는 줄이 있으면 **개수만** 알린다.
    ///
    /// 막지 않는다 — 서버에 올리는 일은 되돌리기 어렵지만, 무엇이 옳은지는 사용자가 안다.
    /// 우리가 할 일은 **모르고 지나치지 않게** 하는 것까지다.
    pub(crate) fn warn_secrets_before_upload(&mut self, paths: &[std::path::PathBuf]) {
        // 이진 파일과 큰 파일은 건너뛴다 — 훑는 값보다 멈추는 대가가 크다.
        const MAX_BYTES: u64 = 4 << 20;
        let mut files = 0usize;
        let mut lines = 0usize;
        for p in paths {
            let big = std::fs::metadata(p).map(|m| m.len() > MAX_BYTES).unwrap_or(true);
            if big || nabi_editor::edithex::peek_is_binary(p) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(p) else { continue };
            let n = crate::secretscan::scan(&text).lines.len();
            if n > 0 {
                files += 1;
                lines += n;
            }
        }
        if files == 0 {
            return; // 조용한 것이 기본이다 — 아무것도 없을 때까지 말을 걸지 않는다.
        }
        let msg = format!("{} {files} \u{b7} {lines}", nabi_i18n::tr(self.lang, "secret.upload"));
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

impl NabiApp {
    /// 올리기 전에 한 줄 알리고, 그다음 올린다. 호출부를 한 줄로 유지한다.
    pub(crate) fn upload_with_warning(&mut self, paths: Vec<std::path::PathBuf>) {
        self.warn_secrets_before_upload(&paths);
        for p in paths {
            self.upload_local_path(p); // SFTP 업로드(다중 선택 일괄).
        }
    }
}
