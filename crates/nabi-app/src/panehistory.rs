//! pane 의 **전체 기록**을 읽을 수 있는 글로 만들어 nabiPad 로 연다.
//!
//! ## 왜 이 길이 필요한가
//!
//! 사용자가 계속 겪은 일이다 — 클로드 코드로 몇 시간을 일한 뒤 휠을 올려 보면 지나간
//! 것이 없다. 그 원인은 실측으로 확인했다(2026-08-29).
//!
//! * 스크롤백을 지우는 신호는 **한 번도 오지 않았다**. 우리가 지우는 것이 아니다.
//! * 그 프로그램은 커서를 특정 줄로 옮겨 그 자리를 덮어 그린다. 6시간 동안 절대 행 이동이
//!   117,424번, 그중 대부분이 화면 아래쪽 두 줄이었다.
//! * 그래서 터미널 스크롤백에 올라간 것은 450줄뿐이었다. 올라가지 않은 것이지 지워진 것이
//!   아니다 — 스크롤백은 **흘러 내려간 줄**만 담는 곳이라 덮어 그린 것은 담기지 않는다.
//!
//! 같은 시간의 바이트 기록은 9MB 였다. **모든 것이 거기 있다.** 다만 제어 신호가 섞인
//! 형식이라 그냥 열면 읽을 수 없었다.
//!
//! 그래서 여는 순간에 글자만 남겨 파일로 만들고, 큰 파일을 다루는 nabiPad 로 넘긴다.

use nabi_types::PaneId;

impl crate::app::NabiApp {
    /// 이 pane 의 전체 기록을 읽을 수 있는 글로 만들어 편집기로 연다.
    pub(crate) fn open_pane_history(&mut self, pane: PaneId) {
        // 방금까지의 것도 보여야 한다 — 안 흘려보내면 몇 초 전 것이 빠진다.
        self.flush_session_logs();
        let Some(src) = self.session_logs.get(&pane).map(|l| l.path.clone()) else {
            self.notify = Some((
                nabi_i18n::tr(self.lang, "hist.notrecording").to_string(),
                std::time::Instant::now(),
            ));
            return;
        };
        // 기록은 지금도 쓰이는 중이다 — 끝이 잘려 있어도 읽어야 한다(castplain::read_log).
        let text = match crate::castplain::read_log(&src) {
            Ok(t) => t,
            Err(e) => {
                self.notify = Some((format!("\u{2715} {e}"), std::time::Instant::now()));
                return;
            }
        };
        let plain = crate::castplain::cast_to_plain(&text);
        if plain.trim().is_empty() {
            self.notify = Some((
                nabi_i18n::tr(self.lang, "hist.empty").to_string(),
                std::time::Instant::now(),
            ));
            return;
        }
        // 원본 옆에 둔다 — 어느 기록에서 나온 글인지 이름으로 알 수 있어야 한다.
        let out = src.with_extension("txt");
        if let Err(e) = std::fs::write(&out, plain) {
            self.notify = Some((format!("\u{2715} {e}"), std::time::Instant::now()));
            return;
        }
        self.open_editor_local(out);
    }
}
