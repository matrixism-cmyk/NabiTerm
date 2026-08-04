//! 코드 하이라이트(syntect) — editorsyntax의 전역 자산 사용. 캐시 키에 테마 이름을 포함해
//! 테마 전환 시 자동 재계산된다. onig(C) 회피로 syntect는 fancy-regex(순수 Rust) 빌드.

use egui::text::LayoutJob;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::util::LinesWithEndings;

/// 하이라이트를 적용할 최대 크기(이보다 크면 평문 — 편집 응답성 유지, 대용량은 뷰어가 담당).
pub(crate) const MAX_HL_BYTES: usize = 200_000;

#[derive(Default)]
struct Hl;

impl egui::util::cache::ComputerMut<(&str, &str, u32, &str), LayoutJob> for Hl {
    fn compute(&mut self, (text, ext, fsize, theme): (&str, &str, u32, &str)) -> LayoutJob {
        let lock = crate::editorsyntax::assets();
        let Ok(a) = lock.read() else { return LayoutJob::default() };
        // 확장자 강제 매핑 > 확장자 자동 > 첫 줄 > 평문.
        let syntax = crate::editorsyntax::mapped_syntax(ext)
            .and_then(|n| a.ps.find_syntax_by_name(&n))
            .or_else(|| a.ps.find_syntax_by_extension(ext))
            .or_else(|| a.ps.find_syntax_by_first_line(text))
            .unwrap_or_else(|| a.ps.find_syntax_plain_text());
        let th = match a.themes.themes.get(theme).or_else(|| a.themes.themes.values().next()) {
            Some(t) => t,
            None => return LayoutJob::default(),
        };
        let mut h = HighlightLines::new(syntax, th);
        let font = egui::FontId::monospace(fsize as f32);
        let mut job = LayoutJob::default();
        for line in LinesWithEndings::from(text) {
            let ranges = h.highlight_line(line, &a.ps).unwrap_or_default();
            for (style, piece) in ranges {
                let c = style.foreground;
                job.append(
                    piece,
                    0.0,
                    egui::TextFormat {
                        font_id: font.clone(),
                        color: egui::Color32::from_rgb(c.r, c.g, c.b),
                        italics: style.font_style.contains(FontStyle::ITALIC),
                        ..Default::default()
                    },
                );
            }
        }
        job
    }
}

type HlCache = egui::util::cache::FrameCache<LayoutJob, Hl>;

/// (확장자, 글꼴크기, 현재 테마)로 text를 하이라이트한 LayoutJob. 캐시로 변경 시에만 재계산.
pub(crate) fn highlight(ctx: &egui::Context, text: &str, ext: &str, font_size: f32) -> LayoutJob {
    let theme = crate::editorsyntax::current_theme();
    ctx.memory_mut(|m| m.caches.cache::<HlCache>().get((text, ext, font_size as u32, &theme)))
}
