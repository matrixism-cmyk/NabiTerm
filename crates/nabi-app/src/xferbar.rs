//! 전송 진행률 막대 — **업데이트·SFTP 큐·trzsz가 같이 쓰는** 하나뿐인 위젯.
//!
//! 같은 일(무언가를 옮기는 중)인데 화면마다 다르게 생기면 사용자가 매번 새로 읽어야 한다.
//! 그래서 새 막대를 또 만들지 않고, 정보가 가장 많은 SFTP 큐 쪽 모양으로 하나를 뽑아 공유한다
//! (사용자 의견 2026-08-21: "업데이트에 쓴 프로그레스바 같은 것을 쓰거나 새로 하나 만들거나").

use nabi_i18n::{tr, Lang};

/// 막대 하나가 보여줄 것.
pub(crate) struct XferView<'a> {
    /// 방향 표시(⬆/⬇) 또는 빈 문자열.
    pub arrow: &'a str,
    pub name: &'a str,
    pub done: u64,
    /// 0이면 전체 크기를 모른다 — 막대 대신 지금까지의 양을 보여 준다.
    pub total: u64,
    /// 초당 바이트. 0이면 아직 못 잰 것.
    pub bps: u64,
    /// `3/10`처럼 여러 개 중 몇 번째인지. (0,0)이면 표시하지 않는다.
    pub index: usize,
    pub count: usize,
    pub width: f32,
}

impl XferView<'_> {
    /// 진행률과 남은 시간은 **한 곳에서만** 센다(`nabi_trzsz::Progress`).
    ///
    /// 예전에는 같은 식을 여기 한 번 더 적어 뒀다. 두 벌이 있으면 한쪽만 고쳐지고, 그러면
    /// 같은 전송이 화면마다 다른 남은 시간을 말한다. 실제로 그쪽 함수는 아무도 쓰지 않는
    /// 채로 남아 있었다(2026-08-30 전수 점검).
    fn calc(&self) -> nabi_trzsz::Progress {
        nabi_trzsz::Progress {
            index: self.index,
            count: self.count,
            name: String::new(),
            done: self.done,
            total: self.total,
            bps: self.bps,
        }
    }

    fn fraction(&self) -> Option<f32> {
        self.calc().fraction()
    }

    /// `⬇ a.zip · 3/10 · 12.4MB/40MB · 3.1MB/s · 남은 9초`
    fn caption(&self, lang: Lang) -> String {
        let mut s = String::new();
        if !self.arrow.is_empty() {
            s.push_str(self.arrow);
            s.push(' ');
        }
        s.push_str(self.name);
        if self.count > 1 {
            s.push_str(&format!(" \u{00b7} {}/{}", self.index, self.count));
        }
        s.push_str(&format!(" \u{00b7} {}", amount(self.done, self.total)));
        if self.bps > 0 {
            s.push_str(&format!(" \u{00b7} {}/s", crate::browserfs::human(self.bps)));
            if let Some(eta) = self.eta() {
                s.push_str(&format!(" \u{00b7} {} {}", tr(lang, "xfer.left"), secs(eta)));
            }
        }
        s
    }

    fn eta(&self) -> Option<u64> {
        self.calc().eta_secs()
    }
}

/// 진행률 막대를 그린다. 크기를 모르면 막대 대신 글자만(멈춘 것과 구별되게 양이 는다).
pub(crate) fn xfer_bar(ui: &mut egui::Ui, lang: Lang, v: &XferView) {
    let text = v.caption(lang);
    match v.fraction() {
        Some(f) => {
            ui.add(egui::ProgressBar::new(f).desired_width(v.width).text(text));
        }
        None => {
            ui.label(format!("\u{23f3} {text}"));
        }
    }
}

/// `12.4MB/40MB` 또는 크기를 모를 때는 `12.4MB`.
fn amount(done: u64, total: u64) -> String {
    if total > 0 {
        format!("{}/{}", crate::browserfs::human(done), crate::browserfs::human(total))
    } else {
        crate::browserfs::human(done)
    }
}

/// 사람이 읽는 시간 — 초/분/시. 남은 시간은 정확도보다 감이 중요하다.
fn secs(s: u64) -> String {
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {}s", s / 60, s % 60),
        _ => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view<'a>(done: u64, total: u64, bps: u64) -> XferView<'a> {
        XferView { arrow: "\u{2b07}", name: "a.zip", done, total, bps, index: 0, count: 0, width: 180.0 }
    }

    #[test]
    fn fraction_is_clamped() {
        assert_eq!(view(50, 100, 0).fraction(), Some(0.5));
        assert_eq!(view(500, 100, 0).fraction(), Some(1.0), "원격이 거짓말해도 막대는 안 넘친다");
        assert_eq!(view(50, 0, 0).fraction(), None);
    }

    #[test]
    fn caption_shows_what_matters() {
        let c = view(1024, 4096, 512).caption(Lang::Ko);
        assert!(c.contains("a.zip"), "{c}");
        assert!(c.contains('/'), "받은 양/전체가 보여야 한다: {c}");
        assert!(c.contains("/s"), "속도가 보여야 한다: {c}");
    }

    #[test]
    fn caption_hides_speed_and_eta_until_measured() {
        let c = view(1024, 4096, 0).caption(Lang::Ko);
        assert!(!c.contains("/s"), "못 잰 속도를 0으로 보여주면 안 된다: {c}");
    }

    #[test]
    fn counts_show_only_when_there_are_several() {
        let mut v = view(1, 2, 0);
        v.index = 3;
        v.count = 10;
        assert!(v.caption(Lang::Ko).contains("3/10"));
        v.count = 1;
        assert!(!v.caption(Lang::Ko).contains("3/1"));
    }

    #[test]
    fn eta_is_only_shown_while_there_is_work_left() {
        assert_eq!(view(50, 100, 10).eta(), Some(5));
        assert_eq!(view(100, 100, 10).eta(), None);
    }

    #[test]
    fn time_is_readable_at_every_scale() {
        assert_eq!(secs(9), "9s");
        assert_eq!(secs(90), "1m 30s");
        assert_eq!(secs(7325), "2h 2m");
    }
}
