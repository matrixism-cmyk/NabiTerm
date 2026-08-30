//! 이 세션의 **호스트키를 그 자리에서 보여 준다** — 세션 우클릭 ▸ 호스트키 보기.
//!
//! ## 왜 필요한가
//!
//! 서버 관리자가 "우리 서버 키 지문이 이것 맞느냐"고 물으면 답할 길이 없었다.
//! 알려진 호스트 관리 창은 있지만 **저장된 것 전부를 늘어놓는다** — 수백 줄 중에서
//! 그 호스트를 눈으로 찾아야 했다.
//!
//! 찾는 함수(`known_hosts_find`)와 한 줄로 적는 함수(`to_line`)는 둘 다 이미 있었는데
//! 아무도 부르지 않았다(`xtask unused` 로 찾았다). 부를 자리만 없던 셈이다.
//!
//! ## 무엇을 보여 주는가
//!
//! 키 종류와 지문(SHA-256)을 알림으로 띄우고, known_hosts 한 줄은 클립보드에 담는다.
//! 지문은 서버 쪽에서 `ssh-keygen -lf` 로 찍은 것과 **글자 그대로 견줄 수 있는 형태**여야
//! 뜻이 있다. 눈으로 견주는 것은 틀리기 쉬우니 붙여 넣어 견줄 수 있게 한다.

use nabi_i18n::tr;

impl crate::app::NabiApp {
    /// 이 호스트의 호스트키를 보여 준다. 저장된 것이 없으면 없다고 말한다.
    ///
    /// 알림으로 내는 까닭은, 이것을 보는 순간이 대개 "지금 이 서버가 맞나"를 확인하는
    /// 짧은 순간이기 때문이다. 창을 하나 더 띄우면 그 흐름이 끊긴다.
    pub(crate) fn show_host_key(&mut self, host: &str, port: u16, ctx: &egui::Context) {
        let msg = match self.host_key_of(host, port) {
            Some((shown, line)) => {
                ctx.copy_text(line); // 붙여 넣어 견줄 수 있게 — 눈으로 견주면 틀린다.
                format!("{shown}\n{}", tr(self.lang, "hostkey.copied"))
            }
            None => format!("{} {host}:{port}", tr(self.lang, "hostkey.none")),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }

    /// (보여 줄 글, 클립보드에 담을 known_hosts 한 줄).
    fn host_key_of(&self, host: &str, port: u16) -> Option<(String, String)> {
        let content = std::fs::read_to_string(&self.known_hosts_path).ok()?;
        let e = nabi_ssh::knownhosts::known_hosts_find(&content, host, port)?;
        // 지문을 못 읽어도 종류와 줄은 보여 준다 — 아무것도 안 보여 주는 것보다 낫다.
        let fp = e.fingerprint().unwrap_or_else(|| tr(self.lang, "hostkey.unreadable").to_string());
        Some((format!("\u{1f511} {} {fp}", e.key_type), e.to_line()))
    }
}
