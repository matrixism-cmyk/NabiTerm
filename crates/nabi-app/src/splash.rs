//! 시작 스플래시 — 첫 몇 초 동안 제품명·버전·지원 문구를 덮어 보여 준다.
//!
//! **별도 OS 창이 아니라 본 창 위 오버레이다.** 분리 창(viewport)은 포커스를 빼앗고 작업
//! 표시줄에 잠깐 나타났다 사라져 오히려 어수선하다. 어차피 본 창이 떠야 사용자 눈에 띄므로,
//! 그 위에 덮는 편이 보이는 결과는 같고 사고는 적다.
//!
//! 터미널은 하루에도 몇 번씩 여는 프로그램이다. 그래서 **아무 키나 클릭으로 즉시 넘어가고**,
//! 설정에서 끌 수 있다. 끌 수 없는 3초는 두 번째 실행부터 방해가 된다.

use nabi_i18n::{tr, Lang};
use std::time::{Duration, Instant};

/// 화면에 머무는 시간.
const HOLD: Duration = Duration::from_millis(2600);
/// 마지막에 서서히 사라지는 시간.
const FADE: Duration = Duration::from_millis(400);

/// 남은 수명 비율(1.0=막 떴다, 0.0=끝). 시간이 다 되면 None.
fn alpha(since: Instant) -> Option<f32> {
    let e = since.elapsed();
    if e >= HOLD + FADE {
        return None;
    }
    if e <= HOLD {
        return Some(1.0);
    }
    Some(1.0 - (e - HOLD).as_secs_f32() / FADE.as_secs_f32())
}

/// 스플래시를 그린다. 계속 보여야 하면 true.
pub(crate) fn show(ctx: &egui::Context, since: Instant, lang: Lang) -> bool {
    let Some(a) = alpha(since) else { return false };
    // 아무 입력이나 들어오면 바로 걷는다 — 기다리게 만들지 않는다.
    let pressed = ctx.input(|i| {
        i.pointer.any_click() || i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
    });
    if pressed {
        return false;
    }
    let screen = ctx.input(|i| i.viewport_rect());
    let fade = |c: egui::Color32| egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8);
    egui::Area::new(egui::Id::new("nabi_splash"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            // 화면 전체를 먹는 응답을 먼저 잡아 둔다 — 그러지 않으면 스플래시를 걷으려고 누른
            // 클릭이 아래 터미널까지 함께 눌러 버린다.
            ui.allocate_rect(screen, egui::Sense::click_and_drag());
            let p = ui.painter();
            p.rect_filled(screen, egui::CornerRadius::ZERO, fade(egui::Color32::from_rgb(16, 18, 24)));
            let mid = screen.center();
            let (title, body, small) = (
                egui::FontId::proportional(44.0),
                egui::FontId::proportional(16.0),
                egui::FontId::proportional(12.0),
            );
            p.text(mid - egui::vec2(0.0, 70.0), egui::Align2::CENTER_CENTER,
                "\u{1f98b} nabiTerm", title, fade(egui::Color32::from_rgb(232, 236, 245)));
            p.text(mid - egui::vec2(0.0, 28.0), egui::Align2::CENTER_CENTER,
                tr(lang, "help.desc"), body, fade(egui::Color32::from_rgb(150, 160, 180)));
            p.text(mid + egui::vec2(0.0, 2.0), egui::Align2::CENTER_CENTER,
                concat!("v", env!("CARGO_PKG_VERSION")), small.clone(), fade(egui::Color32::from_rgb(120, 130, 150)));
            // 지원 문구 — 이 화면에 두면 매번 눈에 띄면서도 평소 화면을 어지럽히지 않는다.
            p.text(mid + egui::vec2(0.0, 74.0), egui::Align2::CENTER_CENTER,
                tr(lang, "help.about.funding"), small.clone(), fade(egui::Color32::from_rgb(120, 130, 150)));
            p.text(egui::pos2(mid.x, screen.max.y - 22.0), egui::Align2::CENTER_CENTER,
                tr(lang, "splash.skip"), small, fade(egui::Color32::from_rgb(90, 98, 115)));
        });
    ctx.request_repaint_after(Duration::from_millis(16));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_fully_opaque_at_first_then_fades_then_ends() {
        assert_eq!(alpha(Instant::now()), Some(1.0));
        let mid = Instant::now() - (HOLD + FADE / 2);
        let a = alpha(mid).expect("아직 사라지지 않았어야 한다");
        assert!((0.0..1.0).contains(&a), "페이드 중 알파가 이상함: {a}");
        assert_eq!(alpha(Instant::now() - (HOLD + FADE)), None);
    }

    /// 전체 수명이 사람이 읽을 수 있으면서도 성가시지 않은 범위인지.
    #[test]
    fn the_whole_thing_is_about_three_seconds() {
        let total = (HOLD + FADE).as_millis();
        assert!((2000..=4000).contains(&total), "{total}ms");
    }
}
