//! 미리보기 **본문 그리기** — 로컬 탐색기와 원격 SFTP 가 같은 것을 쓴다.
//!
//! ## 왜 모았나
//!
//! 2026-09-09 쌍둥이 비교에서 두 미리보기가 **완전히 다른 두 벌**인 것을 봤다.
//!
//! 원격(SFTP)은 [`crate::sftppreview::describe`] 로 내용을 갈라 보고, 이진이면 16진으로
//! 보여 주고, 어떤 인코딩으로 읽었는지 적고, 잘렸으면 그렇다고 말하고, 복사 단추와
//! "편집으로 이어가기"까지 있었다.
//!
//! 로컬은 64KB 를 읽어 200줄을 그냥 글자로 뿌렸다. **이진 파일을 미리보기로 열면
//! 깨진 글자가 화면을 채웠다** — 무엇인지 알 길이 없고, 인코딩이 틀려 깨진 것인지
//! 원래 글이 아닌 것인지도 구분되지 않았다.
//!
//! 없던 것은 기능이 아니라 **부르는 자리**였다. `describe` 는 바이트만 받는 순수 함수라
//! 로컬도 그대로 쓸 수 있었다. 그리는 쪽만 여기로 옮겨 둘이 같은 것을 부르게 한다.
//!
//! ## 키 이름이 `sftp.` 로 시작하는 까닭
//!
//! 이 문구들은 원격 미리보기에서 먼저 생겼다. 글 자체는 어느 쪽에서나 같은 뜻이라
//! (`비어 있습니다`·`이진 파일`·`일부만`) 이름만 남은 것이다. 이름을 바꾸면 쓰는 자리를
//! 전부 고쳐야 하는데, 그 값이 이름의 어색함보다 크지 않다고 봤다.

use crate::sftppreview::Preview;
use nabi_i18n::{tr, Lang};

/// 미리보기 본문. 복사 단추를 누르면 `copy` 에 담아 돌려준다(그리는 중에 클립보드를
/// 건드리지 않는다 — 부르는 쪽이 프레임 끝에서 한다).
pub(crate) fn body(ui: &mut egui::Ui, lang: Lang, p: &Preview, copy: &mut Option<String>) {
    match p {
        Preview::Empty => {
            ui.weak(tr(lang, "sftp.preview.empty"));
        }
        Preview::Text { body, encoding, truncated } => {
            ui.horizontal(|ui| {
                ui.weak(encoding); // 깨져 보이면 이게 첫 실마리다.
                if *truncated {
                    ui.weak(tr(lang, "sftp.preview.partial"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(tr(lang, "menu.copy")).clicked() {
                        *copy = Some(body.clone());
                    }
                });
            });
            ui.separator();
            scroll(ui, "prev_text", body);
        }
        Preview::Image { width, height, rgba } => {
            ui.horizontal(|ui| {
                ui.weak(format!("{width} \u{00d7} {height}"));
            });
            ui.separator();
            image(ui, *width, *height, rgba);
        }
        Preview::Binary { hex, shown } => {
            ui.horizontal(|ui| {
                ui.weak(tr(lang, "sftp.preview.binary"));
                ui.weak(format!("{shown} B"));
            });
            ui.separator();
            scroll(ui, "prev_hex", hex);
        }
    }
}

/// 가로세로로 구르는 등폭 본문. 줄바꿈하지 않는다 — 16진 덤프는 줄이 접히면 못 읽는다.
fn scroll(ui: &mut egui::Ui, salt: &str, text: &str) {
    egui::ScrollArea::both().id_salt(salt).auto_shrink([false, false]).show(ui, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap_mode(egui::TextWrapMode::Extend));
    });
}

/// 그림을 창 크기에 맞춰 그린다.
///
/// 텍스처는 **egui 의 임시 저장소에 담아 재사용한다.** 매 프레임 새로 올리면 큰 그림에서
/// 곧바로 느려지고, GPU 메모리도 프레임마다 늘어난다. 열쇠는 크기와 바이트 수로 만든다 —
/// 같은 그림을 다시 열면 그대로 쓰고, 다른 그림이면 새로 올린다.
fn image(ui: &mut egui::Ui, w: u32, h: u32, rgba: &[u8]) {
    let id = egui::Id::new(("nabi_preview_img", w, h, rgba.len()));
    let tex = ui.data_mut(|d| d.get_temp::<egui::TextureHandle>(id)).unwrap_or_else(|| {
        let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba);
        let t = ui.ctx().load_texture("nabi_preview_img", ci, egui::TextureOptions::LINEAR);
        ui.data_mut(|d| d.insert_temp(id, t.clone()));
        t
    });
    egui::ScrollArea::both().id_salt("prev_img").auto_shrink([false, false]).show(ui, |ui| {
        // 창보다 크면 줄여서 다 보이게 한다(비율은 지킨다). 작은 그림은 키우지 않는다 —
        // 아이콘을 늘려 놓으면 흐릿해져서 무엇인지 더 알기 어렵다.
        let avail = ui.available_size();
        // 하한을 두는 까닭: 창을 아주 작게 줄이면 배율이 0 이나 음수가 되어 그림이
        // 사라진다. 0.01 이면 적어도 "여기 무언가 있다"는 보인다.
        let scale = (avail.x / w as f32).min(avail.y / h as f32).clamp(0.01, 1.0);
        ui.add(
            egui::Image::from_texture((tex.id(), tex.size_vec2()))
                .fit_to_exact_size(egui::vec2(w as f32 * scale, h as f32 * scale)),
        );
    });
}

/// 이름만 보고 "그림일 것 같은가" — **얼마나 읽을지**를 정하는 데만 쓴다.
///
/// 진짜 판정은 디코드가 한다(`nabi_image::decode_image_bytes`). 이름은 믿을 것이 못 되지만,
/// 받기 **전에** 알 수 있는 것은 이름뿐이다. 틀려도 손해는 조금 더 읽거나 덜 읽는 것뿐이고,
/// 덜 읽었으면 그림 대신 16진이 보인다(거짓말은 하지 않는다).
pub(crate) fn looks_like_image(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
    // `.png` 는 확장자가 아니라 **이름 전체**다(`.gitignore` 와 같은 꼴). 앞이 비어 있으면
    // 확장자로 치지 않는다 — 시험이 이 경우를 짚어 줬다.
    let Some((stem, ext)) = name.rsplit_once('.').filter(|(s, _)| !s.is_empty()) else {
        return false;
    };
    let _ = stem;
    // `nabi-image` 가 실제로 푸는 것만 적는다(png·jpeg·gif). 못 푸는 것을 적어 두면
    // 크게 받아 놓고 16진을 보여 주게 된다.
    matches!(ext, "png" | "jpg" | "jpeg" | "gif")
}

#[cfg(test)]
mod tests {
    use super::looks_like_image;

    /// 푸는 것만 그림으로 본다 — 못 푸는 것을 크게 받아 봐야 16진만 나온다.
    #[test]
    fn 푸는_확장자만_그림으로_본다() {
        for y in ["a.png", "/srv/b.JPG", r"C:\x\c.jpeg", "d.gif"] {
            assert!(looks_like_image(y), "{y}");
        }
        for n in ["a.bmp", "a.webp", "a.svg", "a.txt", "noext", "", "a.png.txt", ".png"] {
            assert!(!looks_like_image(n), "{n}");
        }
    }

    /// 경로에 점이 있어도 **파일 이름**의 확장자만 본다.
    #[test]
    fn 폴더_이름에_속지_않는다() {
        assert!(!looks_like_image("/srv/v1.2.3/README"));
        assert!(looks_like_image("/srv/v1.2.3/shot.png"));
    }
}
