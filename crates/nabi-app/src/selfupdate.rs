//! 명령으로 부르는 **스스로 올리기** — 사람이 단추를 누르지 않아도 된다.
//!
//! ## 왜 필요한가
//!
//! 지금까지 최신판으로 올리려면 도움말 창을 열고 확인 → 내려받기 → 설치를 눌러야 했다.
//! 사람이 앞에 있어야만 되는 길이다. 그런데 사용자가 원한 것은 이것이다 —
//! "배포하고 나서 스스로 올라가서 계속 일하기"(2026-08-29).
//!
//! ## 무엇을 하는가
//!
//! `nabi cli update` 하나로 확인 → 내려받기 → **SHA-256 대조** → 조용한 설치 → 다시 켜기가
//! 이어진다. 화면 단추가 하던 일을 그대로 하되 부르는 자리만 달라진 것이라, 검증을
//! 건너뛰는 지름길은 만들지 않았다. 대조에 실패하면 설치하지 않는다.
//!
//! `--check` 를 주면 확인만 하고 멈춘다.
//!
//! ## 권한
//!
//! 설치는 **주입 등급**이다(`dispatch::group_of`). 프로그램 자체를 바꿔 치우고 다시 켜는
//! 일이라 pane 에 글자를 밀어 넣는 것과 같은 무게로 다룬다 — `ask` 모드라면 한 번은
//! 사람이 허락해야 한다. 확인만 하는 것은 아무것도 바꾸지 않으니 보통 등급이다.

use nabi_release::UpdateStatus;

impl crate::app::NabiApp {
    /// 최신판을 확인하고, `check` 가 아니면 이어서 조용히 설치한다.
    pub(crate) fn control_self_update(&mut self, check: bool) {
        // 이미 확인한 결과가 있으면 그것으로 바로 간다 — 다시 물을 이유가 없다.
        if let UpdateStatus::Available(r) = self.updater.get_status() {
            self.note_update(&format!("\u{2b06} v{}", r.version));
            if !check {
                self.updater.download_async(r, self.update_quit.clone());
            }
            return;
        }
        self.updater.check_async();
        // 확인은 딴 실에서 돈다. 결과가 오면 아래 `tick_self_update` 가 이어받는다.
        self.self_update_pending = !check;
        self.note_update(nabi_i18n::tr(self.lang, "update.checking"));
    }

    /// 확인 결과가 오면 이어서 설치한다. 매 프레임 부른다.
    ///
    /// 확인이 딴 실에서 돌기 때문에 부르는 자리에서 기다릴 수 없다. 그래서 요청만 적어
    /// 두고, 답이 온 프레임에 여기서 이어 간다.
    pub(crate) fn tick_self_update(&mut self) {
        if !self.self_update_pending {
            return;
        }
        match self.updater.get_status() {
            UpdateStatus::Available(r) => {
                self.self_update_pending = false;
                self.note_update(&format!("\u{2b07} v{}", r.version));
                self.updater.download_async(r, self.update_quit.clone());
            }
            UpdateStatus::UpToDate => {
                self.self_update_pending = false;
                self.note_update(nabi_i18n::tr(self.lang, "update.uptodate"));
            }
            UpdateStatus::Error(e) => {
                self.self_update_pending = false;
                self.note_update(&format!("\u{2715} {e}"));
            }
            _ => {}
        }
    }

    fn note_update(&mut self, msg: &str) {
        self.notify = Some((msg.to_string(), std::time::Instant::now()));
    }
}
