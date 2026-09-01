//! **마크다운 강조** — 굵게·기울임·인라인 코드를 씌우고 벗긴다.
//!
//! ## 왜 따로 있는가
//!
//! nabiPad 의 변환 명령 마흔 남짓은 전부 `fn(&str) -> String` 이고, **선택이 없으면
//! 문서 전체**에 걸린다(정렬·중복 제거는 그게 맞다). 강조는 다르다 — 선택 없이 "굵게"를
//! 누르면 문서 전체가 굵어진다. 그래서 이 명령들만 다른 자리를 고른다.
//!
//! EmEditor 26 도 같은 자리를 손봤다: 굵게·기울임·코드가 **선택 없이도** 되게 바꿨다
//! (2026-09-01 조사). 우리는 그때 커서가 놓인 **낱말**을 대상으로 삼는다.
//!
//! ## 씌우기가 아니라 **토글**이다
//!
//! 이미 `**굵게**` 인 글에 다시 굵게를 걸면 `****굵게****` 가 되는 편집기는 쓰기 나쁘다.
//! 같은 명령을 다시 누르면 벗긴다 — VS Code·Obsidian 이 그렇게 한다.
//!
//! ## 양옆 공백은 밖에 남긴다
//!
//! 선택이 `" 굵게 "` 처럼 공백을 물고 있으면 `** 굵게 **` 가 되는데, 마크다운은 이것을
//! 강조로 읽지 않는다(표시 안쪽이 공백이면 안 된다). 그래서 공백은 밖에 남기고 알맹이만
//! 감싼다 — 사용자가 선택을 정확히 하도록 요구하는 대신 우리가 맞춘다.

use nabi_i18n::{tr, Lang};

/// 메뉴에 낼 강조 세 가지 — (i18n 키, 표시).
pub const EMPHASIS: [(&str, &str); 3] =
    [("editor.md.bold", "**"), ("editor.md.italic", "*"), ("editor.md.code", "`")];

/// `marker` 로 감싸거나, 이미 감싸여 있으면 벗긴다.
///
/// 알맹이가 비어 있으면(공백뿐이면) 그대로 돌려준다 — 감쌀 것이 없는데 표시만 넣으면
/// 화면에 `****` 같은 찌꺼기가 남는다.
pub fn toggle_wrap(text: &str, marker: &str) -> String {
    let core = text.trim();
    if core.is_empty() {
        return text.to_string();
    }
    let (lead, tail) = split_pads(text, core);
    let m = marker.len();
    // 벗기기: 앞뒤가 같은 표시로 감싸여 있고, 벗기고 나서도 알맹이가 남아야 한다.
    if core.len() > 2 * m && core.starts_with(marker) && core.ends_with(marker) {
        return format!("{lead}{}{tail}", &core[m..core.len() - m]);
    }
    format!("{lead}{marker}{core}{marker}{tail}")
}

/// 앞뒤 공백을 갈라낸다(알맹이는 이미 `trim` 한 것이라 반드시 안에 있다).
fn split_pads<'a>(text: &'a str, core: &str) -> (&'a str, &'a str) {
    let at = text.find(core).unwrap_or(0);
    (&text[..at], &text[at + core.len()..])
}

/// "마크다운" 서브메뉴 — 고른 표시를 돌려준다(없으면 None).
///
/// 메뉴를 두 벌로 두지 않는다. 작은 문서와 rope 문서가 같은 목록을 쓰므로, 항목이
/// 늘거나 이름이 바뀌어도 한쪽만 낡을 수가 없다.
pub fn emphasis_menu(ui: &mut egui::Ui, lang: Lang) -> Option<&'static str> {
    let mut got = None;
    for (key, marker) in EMPHASIS {
        if ui.button(tr(lang, key)).clicked() {
            got = Some(marker);
            ui.close();
        }
    }
    got
}

impl crate::editbuf::EditBuf {
    /// 선택(없으면 커서 밑 낱말)에 강조를 씌우거나 벗긴다. 바꿨으면 true.
    ///
    /// **문서 전체로 번지지 않는다.** 다른 변환 명령(`apply_transform`)은 선택이 없으면
    /// 문서 전체에 걸리는데, 강조에서 그러면 파일 하나가 통째로 굵어진다.
    #[must_use = "걸 곳이 없으면(false) 사용자에게 말해 줘야 한다"]
    pub fn toggle_emphasis(&mut self, marker: &str) -> bool {
        let (a, b) = match self.selection() {
            Some((a, b)) if a < b => (a, b),
            _ => match self.word_range_at(self.cursor()) {
                Some(r) => (r.start(), r.end()),
                None => return false, // 낱말 위가 아니면 감쌀 것이 없다.
            },
        };
        let src: String = self.rope.slice(a..b).to_string();
        let out = toggle_wrap(&src, marker);
        if out == src {
            return false;
        }
        let end = a + out.chars().count();
        self.replace_chars(a, b, &out);
        // 바꾼 자리를 그대로 선택해 둔다 — 굵게 뒤에 기울임을 이어 걸기 쉽다.
        self.set_cursor(a);
        self.move_head(end);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_and_unwrapping_are_the_same_command() {
        assert_eq!(toggle_wrap("굵게", "**"), "**굵게**");
        assert_eq!(toggle_wrap("**굵게**", "**"), "굵게", "다시 누르면 벗겨야 한다");
    }

    /// 두 번 걸면 원래대로 — 이것이 토글의 정의다.
    #[test]
    fn twice_gets_you_back_where_you_started() {
        for t in ["단어", "여러 낱말", "한글과 english", "a"] {
            for m in ["**", "*", "`"] {
                assert_eq!(toggle_wrap(&toggle_wrap(t, m), m), t, "{t:?} / {m}");
            }
        }
    }

    /// **양옆 공백은 밖에 남는다** — 마크다운은 표시 안쪽 공백을 강조로 읽지 않는다.
    #[test]
    fn padding_stays_outside_the_markers() {
        assert_eq!(toggle_wrap(" 굵게 ", "**"), " **굵게** ");
        assert_eq!(toggle_wrap("\n코드\n", "`"), "\n`코드`\n");
    }

    /// 감쌀 알맹이가 없으면 아무것도 하지 않는다 — `****` 찌꺼기를 남기지 않는다.
    #[test]
    fn nothing_to_emphasise_means_no_change() {
        assert_eq!(toggle_wrap("", "**"), "");
        assert_eq!(toggle_wrap("   ", "*"), "   ");
    }

    /// 표시만 있고 알맹이가 없는 글은 **벗기지 않는다**(벗기면 빈 글이 된다).
    #[test]
    fn a_string_of_only_markers_is_not_unwrapped() {
        assert_eq!(toggle_wrap("**", "**"), "******", "벗기면 빈 글이 되므로 감싸는 쪽으로 간다");
        assert_eq!(toggle_wrap("``", "`"), "````");
    }

    /// **표시가 겹치는 것은 한 겹씩 다룬다.** 굵게(`**`)와 기울임(`*`)은 글자가 같아서
    /// 겹쳐 쓰면 헷갈리기 쉬운데, 규칙은 단순하다 — 누른 표시만큼만 씌우고 벗긴다.
    #[test]
    fn overlapping_markers_are_handled_one_layer_at_a_time() {
        assert_eq!(toggle_wrap("*기울임*", "**"), "***기울임***", "굵게는 기울임 위에 덧씌운다");
        assert_eq!(toggle_wrap("**굵게**", "*"), "*굵게*", "기울임을 누르면 한 겹만 벗겨진다");
        assert_eq!(toggle_wrap("***둘다***", "**"), "*둘다*", "굵게를 누르면 굵게만 벗겨진다");
    }
}
