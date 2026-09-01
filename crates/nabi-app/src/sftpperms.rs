//! SFTP 권한 변경 메뉴(프리셋·재귀·커스텀 8진수) — sftptable에서 분리.

use crate::sftpentries::EClick;
use crate::sftpentryfmt::{mode_to_rwx, parse_octal_mode};
use nabi_i18n::{tr, Lang};

/// 권한 변경 서브메뉴: 프리셋 + (디렉터리) 하위 포함 재귀 + 커스텀 8진수. 선택 시 Chmod 동작을 돌려준다.
pub(crate) fn perms_menu(ui: &mut egui::Ui, name: &str, is_dir: bool, lang: Lang) -> Option<EClick> {
    let mut click = None;
    for (lbl, m) in [("755", 0o755u32), ("700", 0o700), ("644", 0o644), ("600", 0o600)] {
        if ui.button(lbl).clicked() {
            click = Some(EClick::Chmod(name.to_string(), m));
            ui.close();
        }
    }
    // 디렉터리: 하위 포함 재귀 적용(WinSCP식 "모든 하위에 적용").
    if is_dir {
        ui.menu_button(tr(lang, "sftp.chmodrec"), |ui| {
            for (lbl, m) in [("755", 0o755u32), ("700", 0o700), ("644", 0o644), ("600", 0o600)] {
                if ui.button(lbl).clicked() {
                    click = Some(EClick::ChmodRecursive(name.to_string(), m));
                    ui.close();
                }
            }
        });
    }
    ui.separator();
    // 커스텀 8진수: 프리셋 외 임의 권한(예: 640). 입력은 항목별 임시 메모리에 보관.
    let id = egui::Id::new(("chmod", name));
    let mut txt = ui.data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut txt)
                .hint_text(tr(lang, "sftp.chmodcustom"))
                .desired_width(56.0),
        );
        let parsed = parse_octal_mode(&txt);
        if let Some(m) = parsed {
            ui.weak(mode_to_rwx(m, is_dir, false)); // 유효하면 rwx 미리보기(권한 편집 — 링크 무관).
        }
        if ui.add_enabled(parsed.is_some(), egui::Button::new("\u{2713}")).clicked() {
            if let Some(m) = parsed {
                click = Some(EClick::Chmod(name.to_string(), m));
            }
            ui.close();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, txt));
    ui.separator();
    // **소유자·그룹 바꾸기**(WinSCP 에는 있고 우리에겐 없었다). 목록에 소유자를 보여 주기
    // 시작했으니(배치 M) 바꾸고 싶어지는 것이 다음 순서다.
    //
    // 번호로 받는다 — 이름을 받으려면 서버의 `/etc/passwd` 를 신뢰해야 하는데, 그 대조는
    // 보여 주기용으로만 쓰고 있다(없는 계정을 만들어 낼 수는 없다). 빈 칸은 **그대로 둔다**.
    if let Some(c) = owner_row(ui, name, lang) {
        click = Some(c);
    }
    click
}

/// uid/gid 입력 한 줄. 둘 다 비면 아무 일도 안 한다(빈 칸 = 건드리지 않음).
fn owner_row(ui: &mut egui::Ui, name: &str, lang: Lang) -> Option<EClick> {
    let id = egui::Id::new(("chown", name));
    let (mut u, mut g) = ui.data_mut(|d| d.get_temp::<(String, String)>(id).unwrap_or_default());
    let mut click = None;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut u)
                .hint_text(tr(lang, "sftp.chownuid"))
                .desired_width(52.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut g)
                .hint_text(tr(lang, "sftp.chowngid"))
                .desired_width(52.0),
        );
        let (uid, gid) = (parse_id(&u), parse_id(&g));
        // 적은 것이 하나도 없거나, 적었는데 숫자가 아니면 누를 수 없다.
        let typed_ok = (u.trim().is_empty() || uid.is_some()) && (g.trim().is_empty() || gid.is_some());
        let ok = typed_ok && (uid.is_some() || gid.is_some());
        let btn = ui.add_enabled(ok, egui::Button::new("\u{2713}"));
        if btn.on_hover_text(tr(lang, "sftp.chownhint")).clicked() {
            click = Some(EClick::Chown(name.to_string(), uid, gid));
            ui.close();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, (u, g)));
    click
}

/// uid/gid 입력을 번호로. 빈 칸이나 숫자가 아니면 `None`(= 건드리지 않음).
fn parse_id(s: &str) -> Option<u32> {
    let t = s.trim();
    match t.is_empty() {
        true => None,
        false => t.parse::<u32>().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_id;

    /// **빈 칸과 잘못 적은 것은 둘 다 `None` 이지만 뜻이 다르다.**
    ///
    /// 빈 칸은 "건드리지 마라"이고, 잘못 적은 것은 "누를 수 없다"로 막는다(위 `typed_ok`).
    /// 여기서는 번호로 읽히는지만 본다.
    #[test]
    fn only_numbers_become_an_id() {
        assert_eq!(parse_id("0"), Some(0), "0 은 root 다 — 모름이 아니다");
        assert_eq!(parse_id("1000"), Some(1000));
        assert_eq!(parse_id("  1000  "), Some(1000));
        assert_eq!(parse_id(""), None);
        assert_eq!(parse_id("   "), None);
        assert_eq!(parse_id("root"), None, "이름은 못 받는다(번호만)");
        assert_eq!(parse_id("-1"), None);
        assert_eq!(parse_id("1e3"), None);
    }
}
