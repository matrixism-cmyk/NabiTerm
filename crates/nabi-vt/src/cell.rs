//! 렌더용 RenderCell/테마 타입 + 256색 팔레트(코어 비의존 — 추출은 render.rs).

use nabi_types::{ansi16, CellAttrs, Rgba};

/// 렌더러가 그리는 한 셀.
#[derive(Clone, Debug, Default)]
pub struct RenderCell {
    /// 셀 글자(보통 1자, 빈 셀은 빈 문자열).
    pub text: String,
    pub fg: Rgba,
    pub bg: Rgba,
    pub attrs: CellAttrs,
    /// 밑줄 색(SGR 58, 없으면 전경색 사용).
    pub ul_color: Option<Rgba>,
}

/// 커서 모양.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

impl CursorShape {
    /// 라벨("block"/"bar"/"underline")로 변환(미상은 Block).
    pub fn from_label(s: &str) -> Self {
        match s {
            "bar" => CursorShape::Bar,
            "underline" => CursorShape::Underline,
            _ => CursorShape::Block,
        }
    }
}

/// 기본 전경/배경 색 + 커서 모양.
#[derive(Clone, Copy)]
pub struct Theme {
    pub fg: Rgba,
    pub bg: Rgba,
    pub cursor_shape: CursorShape,
    /// 커서 색(None이면 전경색 사용).
    pub cursor_color: Option<Rgba>,
    /// 텍스트 선택 강조 배경색.
    pub sel_color: Rgba,
    /// 검색(Find) 일치 강조 배경색.
    pub match_color: Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            cursor_shape: CursorShape::Block,
            cursor_color: None,
            sel_color: Rgba::rgb(0x33, 0x55, 0x88),
            match_color: Rgba::rgb(0xff, 0xd7, 0x33),
        }
    }
}

impl Theme {
    /// 이름으로 색 스킴 프리셋을 만든다(미상은 기본 검정 배경).
    pub fn preset(name: &str) -> Theme {
        let (fg, bg) = match name {
            "solarized" => ((0x83, 0x94, 0x96), (0x00, 0x2b, 0x36)),
            "nord" => ((0xd8, 0xde, 0xe9), (0x2e, 0x34, 0x40)),
            "gruvbox" => ((0xeb, 0xdb, 0xb2), (0x28, 0x28, 0x28)),
            "dracula" => ((0xf8, 0xf8, 0xf2), (0x28, 0x2a, 0x36)),
            "monokai" => ((0xf8, 0xf8, 0xf2), (0x27, 0x28, 0x22)),
            "onedark" => ((0xab, 0xb2, 0xbf), (0x28, 0x2c, 0x34)),
            "tokyonight" => ((0xc0, 0xca, 0xf5), (0x1a, 0x1b, 0x26)),
            "light" => ((0x33, 0x33, 0x33), (0xfa, 0xfa, 0xfa)),
            _ => return Theme::default(),
        };
        // 선택 강조색은 bg를 fg쪽으로 25% 섞어 테마에 맞게(가독성).
        let mix = |b: u8, f: u8| (b as i16 + (f as i16 - b as i16) / 4) as u8;
        Theme {
            fg: Rgba::rgb(fg.0, fg.1, fg.2),
            bg: Rgba::rgb(bg.0, bg.1, bg.2),
            cursor_shape: CursorShape::Block,
            cursor_color: None,
            sel_color: Rgba::rgb(mix(bg.0, fg.0), mix(bg.1, fg.1), mix(bg.2, fg.2)),
            match_color: Rgba::rgb(0xff, 0xd7, 0x33),
        }
    }

    /// 프리셋 키의 표시용 이름(UI 콤보).
    pub fn preset_label(name: &str) -> &'static str {
        match name {
            "solarized" => "Solarized",
            "nord" => "Nord",
            "gruvbox" => "Gruvbox",
            "dracula" => "Dracula",
            "monokai" => "Monokai",
            "onedark" => "One Dark",
            "tokyonight" => "Tokyo Night",
            "light" => "Light",
            _ => "Default",
        }
    }

    /// 선택 가능한 프리셋 이름 목록.
    pub fn preset_names() -> &'static [&'static str] {
        &[
            "default",
            "solarized",
            "nord",
            "gruvbox",
            "dracula",
            "monokai",
            "onedark",
            "tokyonight",
            "light",
        ]
    }
}

/// xterm 256색 인덱스 → RGBA.
pub(crate) fn idx_to_rgba(i: u8) -> Rgba {
    if i < 16 {
        ansi16(i)
    } else if i < 232 {
        let i = i - 16;
        let conv = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        Rgba::rgb(conv(i / 36), conv((i % 36) / 6), conv(i % 6))
    } else {
        let v = 8 + (i - 232) * 10;
        Rgba::rgb(v, v, v)
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;
    use nabi_types::Rgba;

    #[test]
    fn theme_presets() {
        assert_eq!(Theme::preset("dracula").bg, Rgba::rgb(0x28, 0x2a, 0x36));
        assert_eq!(Theme::preset("tokyonight").bg, Rgba::rgb(0x1a, 0x1b, 0x26));
        // 미상 이름은 기본 테마.
        assert_eq!(Theme::preset("nope").bg, Theme::default().bg);
        assert_eq!(Theme::preset_names().len(), 9);
        // 선택색이 bg와 fg 사이로 도출되는지(고정 파란색이 아님).
        let d = Theme::preset("dracula");
        assert_ne!(d.sel_color, d.bg);
        assert_ne!(d.sel_color, Rgba::rgb(0x33, 0x55, 0x88));
    }
}
