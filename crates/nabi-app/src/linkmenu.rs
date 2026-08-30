//! 터미널 링크 길게 누름 메뉴 — 링크 복사 / 파일 브라우저로 열기 / 탐색기로 열기 /
//! **내장 브라우저 탭으로 열기**.
//!
//! 웹 주소를 길게 눌러도 지금까지는 바깥 브라우저로 나가는 길밖에 없었다. 우리 안에 웹
//! 탭이 있는데 그리로 가는 길이 여기 없었다(사용자 요청 2026-08-30).

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 로컬 파일 경로(드라이브 `X:\`·`X:/` 또는 UNC `\\`)인가.
fn is_local_path(url: &str) -> bool {
    let b = url.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
        || url.starts_with("\\\\")
}

/// 웹 주소인가 — 내장 브라우저로 열 수 있는 것.
///
/// 로컬 경로는 뺀다. 파일은 `파일 브라우저로 열기`·`편집기로 열기` 가 이미 맡고 있고,
/// 웹 화면으로 여는 것이 사용자가 바라는 일이 아니다.
fn is_web_url(url: &str) -> bool {
    let u = url.trim();
    !is_local_path(u) && (u.starts_with("http://") || u.starts_with("https://"))
}

/// 링크 메뉴 한 개를 그린다(메인·분리 창 공용). 반환=(선택 액션, 닫아야 하는가).
/// 액션: 0=복사, 1=파일 브라우저, 2=탐색기/바깥 브라우저, 3=에디터, 4=내장 브라우저 탭.
pub(crate) fn link_menu_area(
    ctx: &egui::Context,
    lang: nabi_i18n::Lang,
    url: &str,
    pos: egui::Pos2,
) -> (Option<u8>, bool) {
    let path = is_local_path(url);
    // 파일참조(`경로:줄`)면 :줄을 떼어 실제 경로로 존재 확인(에디터 버튼 노출).
    let fpath = nabi_render::parse_file_ref(url).0;
    let is_file = is_local_path(&fpath) && std::path::Path::new(&fpath).is_file();
    let mut act: Option<u8> = None;
    let area = egui::Area::new(egui::Id::new("nabi_link_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);
                if ui.button(format!("\u{1f4cb} {}", tr(lang, "link.copy"))).clicked() {
                    act = Some(0);
                }
                if path && ui.button(format!("\u{1f4c1} {}", tr(lang, "link.browser"))).clicked() {
                    act = Some(1);
                }
                if is_file && ui.button(format!("\u{270e} {}", tr(lang, "link.editor"))).clicked() {
                    act = Some(3);
                }
                // 웹 주소면 우리 안의 웹 탭으로 여는 길을 먼저 준다 — 바깥 브라우저로
                // 나가는 것보다 이쪽이 기본에 가깝다(창을 옮기지 않아도 된다).
                if is_web_url(url) && ui.button(format!("\u{1f5d4} {}", tr(lang, "link.webtab"))).clicked() {
                    act = Some(4);
                }
                // 경로면 탐색기 reveal(🖥), URL이면 브라우저 열기(🌐) — act=2가 내부에서 분기.
                let (oico, okey) = if path { ("\u{1f5a5}", "link.explorer") } else { ("\u{1f310}", "link.open") };
                if ui.button(format!("{oico} {}", tr(lang, okey))).clicked() {
                    act = Some(2);
                }
            });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| area.response.rect.contains(p)));
    let close = act.is_some() || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape));
    (act, close)
}

impl NabiApp {
    /// 링크 길게 누름 메뉴를 그린다(설정 시). 액션 실행 또는 바깥 클릭/Esc로 닫는다.
    pub(crate) fn show_link_menu(&mut self, ctx: &egui::Context) {
        let Some((url, pos)) = self.link_menu.clone() else {
            return;
        };
        let (act, close) = link_menu_area(ctx, self.lang, &url, pos);
        self.apply_link_action(ctx, act, &url);
        if close {
            self.link_menu = None;
            self.selection = None; // 길게누르기가 만든 링크 블럭 선택을 해제(닫을 때 블럭 잔존 방지).
        }
    }

    /// 분리 OS 창의 링크 메뉴를 vctx에 그린다(P2 — 새 OS 창에서도 길게누르기 팝업).
    pub(crate) fn show_floating_link_menu(&mut self, vctx: &egui::Context) {
        let Some((url, pos)) = self.floating_link.clone() else {
            return;
        };
        let (act, close) = link_menu_area(vctx, self.lang, &url, pos);
        self.apply_link_action(vctx, act, &url);
        if close {
            self.floating_link = None;
        }
    }

    /// 링크 메뉴 액션을 실행한다(복사 or run_link_action).
    fn apply_link_action(&mut self, ctx: &egui::Context, act: Option<u8>, url: &str) {
        if let Some(a) = act {
            if a == 0 {
                ctx.copy_text(url.to_string());
                self.notify = Some((tr(self.lang, "link.copied").to_string(), std::time::Instant::now()));
            } else {
                self.run_link_action(a, url);
            }
        }
    }

    /// 링크 메뉴 액션: 1=파일 브라우저(탭), 2=탐색기/바깥 브라우저, 3=내장 에디터,
    /// 4=내장 브라우저 탭.
    fn run_link_action(&mut self, action: u8, url: &str) {
        match action {
            4 => {
                self.open_web_tab(url);
            }
            3 => {
                let (p, line, col) = nabi_render::parse_file_ref(url); // :줄:열 분리 → 해당 줄로 열기.
                self.open_editor_at_line(std::path::PathBuf::from(p), line, col);
            }
            1 => {
                let p = std::path::PathBuf::from(url);
                let dir = if p.is_dir() {
                    p
                } else {
                    p.parent().map(|d| d.to_path_buf()).unwrap_or(p)
                };
                self.open_browser_path(dir, 0); // 새 탭으로.
            }
            2 => {
                if is_local_path(url) {
                    let _ = std::process::Command::new("explorer")
                        .arg(format!("/select,{url}"))
                        .spawn();
                } else {
                    crate::paneurl::os_open(url);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_web_url;

    #[test]
    fn 웹_주소만_내장_브라우저로_연다() {
        assert!(is_web_url("https://example.com"));
        assert!(is_web_url("http://10.0.0.2:8080/a"));
        // 로컬 경로는 아니다 — 파일 브라우저·편집기가 맡는다.
        assert!(!is_web_url(r"C:\일감\메모.txt"));
        assert!(!is_web_url(r"\\서버\몫"));
        // 알 수 없는 스킴은 바깥에 맡긴다(메일·앱 링크).
        assert!(!is_web_url("mailto:a@b.c"));
        assert!(!is_web_url("ftp://example.com"));
    }
}
