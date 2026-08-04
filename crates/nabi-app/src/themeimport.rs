//! Windows Terminal 색 구성표(JSON) 가져오기 — fg/bg/커서/선택색을 설정에 적용.

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};
use serde::Deserialize;

/// WT 색 구성표에서 우리가 쓰는 필드만(나머지 16색 팔레트는 코어 교체 후 확장).
#[derive(Deserialize)]
pub(crate) struct WtScheme {
    pub name: Option<String>,
    pub background: Option<String>,
    pub foreground: Option<String>,
    #[serde(rename = "cursorColor")]
    pub cursor_color: Option<String>,
    #[serde(rename = "selectionBackground")]
    pub selection_background: Option<String>,
}

/// WT JSON 텍스트를 파싱한다(스킴 객체 1개).
pub(crate) fn parse_wt_scheme(json: &str) -> Result<WtScheme, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Alacritty 색 테마(TOML)를 파싱해 WtScheme로 변환한다(`[colors.primary]`/`cursor`/`selection`).
pub(crate) fn parse_alacritty(toml_str: &str) -> Result<WtScheme, String> {
    #[derive(Deserialize)]
    struct Root {
        colors: Option<Colors>,
    }
    #[derive(Deserialize)]
    struct Colors {
        primary: Option<Primary>,
        cursor: Option<CursorC>,
        selection: Option<SelectionC>,
    }
    #[derive(Deserialize)]
    struct Primary {
        background: Option<String>,
        foreground: Option<String>,
    }
    #[derive(Deserialize)]
    struct CursorC {
        cursor: Option<String>,
    }
    #[derive(Deserialize)]
    struct SelectionC {
        background: Option<String>,
    }
    let c = toml::from_str::<Root>(toml_str).map_err(|e| e.to_string())?.colors.ok_or("[colors] 없음")?;
    let norm = |o: Option<String>| o.map(|s| s.strip_prefix("0x").map(|h| format!("#{h}")).unwrap_or(s));
    let (bg, fg) = c.primary.map_or((None, None), |p| (p.background, p.foreground));
    let s = WtScheme {
        name: Some("Alacritty".to_string()),
        background: norm(bg),
        foreground: norm(fg),
        cursor_color: norm(c.cursor.and_then(|x| x.cursor)),
        selection_background: norm(c.selection.and_then(|x| x.background)),
    };
    // 색을 하나도 못 뽑으면 에러로 — 다른 포맷(WezTerm 등)이 자동 시도되도록.
    if s.background.is_none() && s.foreground.is_none() && s.cursor_color.is_none() && s.selection_background.is_none() {
        return Err("Alacritty 색 없음".into());
    }
    Ok(s)
}

/// 파싱된 스킴을 설정 색 오버라이드에 적용한다. 적용 항목 수를 돌려준다.
pub(crate) fn apply(cfg: &mut AppConfig, s: &WtScheme) -> usize {
    let mut n = 0;
    let mut set = |dst: &mut String, v: &Option<String>| {
        if let Some(c) = v {
            *dst = c.clone();
            n += 1;
        }
    };
    set(&mut cfg.appearance.bg_color, &s.background);
    set(&mut cfg.appearance.fg_color, &s.foreground);
    set(&mut cfg.appearance.cursor_color, &s.cursor_color);
    set(&mut cfg.appearance.selection_color, &s.selection_background);
    n
}

/// 설정 다이얼로그의 "테마 가져오기" 섹션: JSON 붙여넣기 → 적용(미리보기에 즉시 반영).
pub(crate) fn import_section(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.import.hint"));
    let id = egui::Id::new("wt_import_json");
    let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(5)
            .desired_width(f32::INFINITY)
            .hint_text("{ \"background\": \"#0C0C0C\", \"foreground\": \"#CCCCCC\", ... }"),
    );
    ui.data_mut(|d| d.insert_temp(id, text.clone()));
    let msg_id = egui::Id::new("wt_import_msg");
    ui.horizontal(|ui| {
        if ui.button(tr(lang, "settings.import.apply")).clicked() {
            // 한 입력칸에서 4개 형식 자동 감지: WT JSON → Alacritty TOML → Xresources → iTerm2.
            use crate::themeimport2::{parse_iterm, parse_konsole, parse_wezterm, parse_xresources};
            let parsed = parse_wt_scheme(&text)
                .or_else(|_| parse_alacritty(&text))
                .or_else(|_| parse_wezterm(&text))
                .or_else(|_| parse_konsole(&text))
                .or_else(|_| parse_xresources(&text))
                .or_else(|_| parse_iterm(&text));
            let msg = match parsed {
                Ok(s) => {
                    let n = apply(cfg, &s);
                    format!("\u{2713} {} ({n})", s.name.unwrap_or_default())
                }
                Err(e) => format!("\u{2715} {e}"),
            };
            ui.data_mut(|d| d.insert_temp(msg_id, msg));
        }
        if let Some(msg) = ui.data(|d| d.get_temp::<String>(msg_id)) {
            ui.weak(msg);
        }
    });
    // 현재 테마 내보내기(클립보드로 복사) — WT JSON / Alacritty TOML.
    ui.horizontal(|ui| {
        if ui.button(tr(lang, "settings.export.wt")).clicked() {
            ui.ctx().copy_text(crate::themeimport2::export_wt_json(cfg));
        }
        if ui.button(tr(lang, "settings.export.alacritty")).clicked() {
            ui.ctx().copy_text(crate::themeimport2::export_alacritty(cfg));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{apply, parse_wt_scheme};

    #[test]
    fn parses_and_applies_wt_scheme() {
        let json = r##"{ "name": "Campbell", "background": "#0C0C0C",
            "foreground": "#CCCCCC", "cursorColor": "#FFFFFF",
            "selectionBackground": "#264F78", "red": "#C50F1F" }"##;
        let s = parse_wt_scheme(json).unwrap();
        let mut cfg = nabi_config::AppConfig::default();
        assert_eq!(apply(&mut cfg, &s), 4);
        assert_eq!(cfg.appearance.bg_color, "#0C0C0C");
        assert_eq!(cfg.appearance.selection_color, "#264F78");
        assert!(parse_wt_scheme("not json").is_err());
    }

    #[test]
    fn parses_alacritty_toml() {
        let toml = "[colors.primary]\nbackground = \"0x1e1e2e\"\nforeground = \"#cdd6f4\"\n\
                    [colors.cursor]\ncursor = \"#f5e0dc\"\n[colors.selection]\nbackground = \"#585b70\"\n";
        let s = super::parse_alacritty(toml).unwrap();
        let mut cfg = nabi_config::AppConfig::default();
        assert_eq!(apply(&mut cfg, &s), 4);
        assert_eq!(cfg.appearance.bg_color, "#1e1e2e"); // 0x→# 정규화.
        assert_eq!(cfg.appearance.fg_color, "#cdd6f4");
    }
}
