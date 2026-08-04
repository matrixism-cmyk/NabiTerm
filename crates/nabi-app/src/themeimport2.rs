//! 색 테마 추가 포맷: Xresources/`.Xdefaults` 가져오기 + iTerm2 `.itermcolors` 가져오기,
//! 현재 테마를 Windows Terminal JSON / Alacritty TOML로 내보내기. 모두 순수 함수(테스트 가능).

use crate::themeimport::WtScheme;
use nabi_config::AppConfig;

/// Xresources(`*foreground: #ccc` / `URxvt.background: #111` / `*.cursorColor: #fff`) → WtScheme.
pub(crate) fn parse_xresources(text: &str) -> Result<WtScheme, String> {
    let (mut bg, mut fg, mut cur) = (None, None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('!') || line.starts_with("//") {
            continue; // 주석.
        }
        let Some((key, val)) = line.split_once(':') else { continue };
        // `*`, `*.`, `URxvt.`, `XTerm.` 등 접두 제거 후 마지막 구성요소만 비교.
        let key = key.trim().rsplit(['.', '*']).next().unwrap_or("").to_ascii_lowercase();
        let val = val.split_whitespace().next().unwrap_or("").to_string();
        if val.is_empty() {
            continue;
        }
        match key.as_str() {
            "foreground" => fg = Some(val),
            "background" => bg = Some(val),
            "cursorcolor" => cur = Some(val),
            _ => {}
        }
    }
    if bg.is_none() && fg.is_none() && cur.is_none() {
        return Err("Xresources 색 없음".into());
    }
    Ok(WtScheme { name: Some("Xresources".into()), background: bg, foreground: fg, cursor_color: cur, selection_background: None })
}

/// iTerm2 `.itermcolors`(plist XML, 채널 0~1 실수) → WtScheme.
pub(crate) fn parse_iterm(xml: &str) -> Result<WtScheme, String> {
    let bg = iterm_color(xml, "Background Color");
    let fg = iterm_color(xml, "Foreground Color");
    let cur = iterm_color(xml, "Cursor Color");
    let sel = iterm_color(xml, "Selection Color");
    if bg.is_none() && fg.is_none() && cur.is_none() {
        return Err("iTerm2 색 없음".into());
    }
    Ok(WtScheme { name: Some("iTerm2".into()), background: bg, foreground: fg, cursor_color: cur, selection_background: sel })
}

/// `<key>NAME</key><dict>…Red/Green/Blue Component…</dict>`에서 #RRGGBB를 뽑는다.
fn iterm_color(xml: &str, name: &str) -> Option<String> {
    let after = xml.split_once(&format!("<key>{name}</key>"))?.1;
    let dict = after.split_once("</dict>")?.0;
    let chan = |c: &str| -> Option<u8> {
        let a = dict.split_once(&format!("<key>{c} Component</key>"))?.1;
        let v = a.split_once("<real>")?.1.split_once("</real>")?.0.trim().parse::<f64>().ok()?;
        Some((v.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    Some(format!("#{:02X}{:02X}{:02X}", chan("Red")?, chan("Green")?, chan("Blue")?))
}

/// WezTerm 색 테마(TOML, 평면 `[colors]` 테이블) → WtScheme.
pub(crate) fn parse_wezterm(toml_str: &str) -> Result<WtScheme, String> {
    #[derive(serde::Deserialize)]
    struct Root {
        colors: Option<Colors>,
    }
    #[derive(serde::Deserialize)]
    struct Colors {
        foreground: Option<String>,
        background: Option<String>,
        cursor_bg: Option<String>,
        selection_bg: Option<String>,
    }
    let c = toml::from_str::<Root>(toml_str).map_err(|e| e.to_string())?.colors.ok_or("[colors] 없음")?;
    let s = WtScheme {
        name: Some("WezTerm".into()),
        background: c.background,
        foreground: c.foreground,
        cursor_color: c.cursor_bg,
        selection_background: c.selection_bg,
    };
    if s.background.is_none() && s.foreground.is_none() && s.cursor_color.is_none() && s.selection_background.is_none() {
        return Err("WezTerm 색 없음".into());
    }
    Ok(s)
}

/// Konsole `.colorscheme`(INI, `[Background] Color=r,g,b`) → WtScheme.
pub(crate) fn parse_konsole(text: &str) -> Result<WtScheme, String> {
    let (mut bg, mut fg, mut cur) = (None, None, None);
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = s.to_string();
        } else if let Some(rgb) = line.strip_prefix("Color=") {
            let n: Vec<u8> = rgb.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if n.len() == 3 {
                let hex = format!("#{:02X}{:02X}{:02X}", n[0], n[1], n[2]);
                match section.as_str() {
                    "Background" => bg = Some(hex),
                    "Foreground" => fg = Some(hex),
                    "Cursor" => cur = Some(hex),
                    _ => {}
                }
            }
        }
    }
    if bg.is_none() && fg.is_none() && cur.is_none() {
        return Err("Konsole 색 없음".into());
    }
    Ok(WtScheme { name: Some("Konsole".into()), background: bg, foreground: fg, cursor_color: cur, selection_background: None })
}

/// 현재 외관 색을 Windows Terminal 색 구성표 JSON으로 내보낸다.
pub(crate) fn export_wt_json(cfg: &AppConfig) -> String {
    let a = &cfg.appearance;
    format!(
        "{{\n  \"name\": \"nabiTerm\",\n  \"background\": \"{}\",\n  \"foreground\": \"{}\",\n  \"cursorColor\": \"{}\",\n  \"selectionBackground\": \"{}\"\n}}",
        a.bg_color, a.fg_color, a.cursor_color, a.selection_color
    )
}

/// 현재 외관 색을 Alacritty 색 테마 TOML로 내보낸다.
pub(crate) fn export_alacritty(cfg: &AppConfig) -> String {
    let a = &cfg.appearance;
    format!(
        "[colors.primary]\nbackground = \"{}\"\nforeground = \"{}\"\n\n[colors.cursor]\ncursor = \"{}\"\n\n[colors.selection]\nbackground = \"{}\"\n",
        a.bg_color, a.fg_color, a.cursor_color, a.selection_color
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themeimport::{apply, parse_alacritty, parse_wt_scheme};

    #[test]
    fn xresources_parsed() {
        let x = "! comment\n*background: #1e1e2e\nURxvt.foreground: #cdd6f4\nXTerm*cursorColor: #f5e0dc\n";
        let s = parse_xresources(x).unwrap();
        let mut cfg = AppConfig::default();
        assert_eq!(apply(&mut cfg, &s), 3);
        assert_eq!(cfg.appearance.bg_color, "#1e1e2e");
        assert_eq!(cfg.appearance.fg_color, "#cdd6f4");
        assert!(parse_xresources("nothing here").is_err());
    }

    #[test]
    fn iterm_parsed() {
        let xml = "<dict><key>Background Color</key><dict>\
            <key>Red Component</key><real>0.0</real>\
            <key>Green Component</key><real>0.5</real>\
            <key>Blue Component</key><real>1.0</real></dict>\
            <key>Foreground Color</key><dict>\
            <key>Red Component</key><real>1</real><key>Green Component</key><real>1</real><key>Blue Component</key><real>1</real></dict></dict>";
        let s = parse_iterm(xml).unwrap();
        assert_eq!(s.background.as_deref(), Some("#0080FF")); // 0,0.5,1 → 00 80 FF.
        assert_eq!(s.foreground.as_deref(), Some("#FFFFFF"));
    }

    #[test]
    fn wezterm_parsed() {
        let t = "[colors]\nforeground = \"#cdd6f4\"\nbackground = \"#1e1e2e\"\ncursor_bg = \"#f5e0dc\"\nselection_bg = \"#585b70\"\n";
        let s = parse_wezterm(t).unwrap();
        let mut cfg = AppConfig::default();
        assert_eq!(apply(&mut cfg, &s), 4);
        assert_eq!(cfg.appearance.bg_color, "#1e1e2e");
        assert_eq!(cfg.appearance.selection_color, "#585b70");
        assert!(parse_wezterm("[colors]\n").is_err()); // 색 없으면 에러(체인 진행).
    }

    #[test]
    fn konsole_parsed() {
        let k = "[Background]\nColor=30,30,46\n[Foreground]\nColor=205,214,244\n[Cursor]\nColor=245,224,220\n";
        let s = parse_konsole(k).unwrap();
        let mut cfg = AppConfig::default();
        assert_eq!(apply(&mut cfg, &s), 3);
        assert_eq!(cfg.appearance.bg_color, "#1E1E2E");
        assert_eq!(cfg.appearance.cursor_color, "#F5E0DC");
        assert!(parse_konsole("[General]\nName=x\n").is_err());
    }

    #[test]
    fn export_roundtrips_to_importers() {
        let mut cfg = AppConfig::default();
        cfg.appearance.bg_color = "#101020".into();
        cfg.appearance.fg_color = "#e0e0e0".into();
        cfg.appearance.cursor_color = "#ff8800".into();
        cfg.appearance.selection_color = "#334455".into();
        // WT JSON 왕복.
        let mut c2 = AppConfig::default();
        apply(&mut c2, &parse_wt_scheme(&export_wt_json(&cfg)).unwrap());
        assert_eq!(c2.appearance.bg_color, "#101020");
        assert_eq!(c2.appearance.selection_color, "#334455");
        // Alacritty TOML 왕복.
        let mut c3 = AppConfig::default();
        apply(&mut c3, &parse_alacritty(&export_alacritty(&cfg)).unwrap());
        assert_eq!(c3.appearance.fg_color, "#e0e0e0");
        assert_eq!(c3.appearance.cursor_color, "#ff8800");
    }
}
