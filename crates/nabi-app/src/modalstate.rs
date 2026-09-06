//! **지금 답을 기다리는 창이 있는가.**
//!
//! ## 왜 한곳에서 알아야 하나
//!
//! 확인 창(닫을까요? · 다시 연결할까요? · 이 명령 정말 보낼까요?)은 답을 받기 전에는 다른
//! 일을 막아야 하는 창이다. 그런데 그 사실을 **아무도 한곳에서 알지 못했다.** 그래서
//! 세 가지가 어긋났다.
//!
//! **하나, 웹 화면이 확인 창을 덮었다.** 웹은 운영체제가 자기 창에 그리므로 우리 그림보다
//! 늘 위에 온다. 그래서 우리가 직접 숨겨야 하는데, 숨기는 조건이 "마우스가 그 위에 있을
//! 때"뿐이었다. 확인 창이 떠도 포인터가 딴 데 있으면 웹이 다시 보이고, 그 아래 깔린 단추는
//! 누를 수 없다(사용자 보고 2026-09-06).
//!
//! **둘, 확인 창이 겹쳐 떴다.** 새 판 알림과 닫기 확인이 함께 떠서 서로를 가렸다(실측으로
//! 확인). 아래 것은 보이는데 눌리지 않는다 — 위 것이 뒤쪽 입력을 막기 때문이다. 보이는데
//! 안 눌리는 것이 가장 나쁘다.
//!
//! **셋, 창이 뒤에 있으면 손이 닿지 않았다.** 분리 창이나 다른 프로그램이 앞에 있으면
//! 메인 창의 확인 창은 그 뒤다. 물어 놓고 답할 길을 막은 셈이다.
//!
//! 그래서 "지금 답을 기다리는 창이 있는가"를 한 함수로 모은다. 새 확인 창을 만들면
//! **여기 한 줄만** 더하면 세 가지가 함께 지켜진다.

impl crate::app::NabiApp {
    /// 지금 **답을 기다리는 창**이 떠 있는가.
    ///
    /// 답을 받기 전에는 다른 일을 막는 창만 센다. 설정 창처럼 열어 두고 다른 일을 해도
    /// 되는 것은 세지 않는다 — 그것까지 세면 웹 화면이 공연히 사라진다.
    pub(crate) fn blocking_modal_open(&self) -> bool {
        self.confirm_close
            || self.reconnect_ask.is_some()
            || self.hostkey_prompt.is_some()
            || self.pending_send.is_some()
            || self.pending_paste.is_some()
            || self.onboarding_open
            // 아래는 검사(`확인_창을_새로_만들면…`)가 찾아 준 것들이다. 처음 목록을 손으로
            // 적었을 때 아홉 개를 빠뜨렸다 — 세어서 확인하지 않았으면 그 창들만 조용히
            // 가려졌을 것이다.
            || !self.pad_recover.is_empty()
            || self.rcmd_pending.is_some()
            || self.session_delete_ask.is_some()
            || self.trzsz.ask.is_some()
            || self.worktree_prompt.is_some()
    }
}

#[cfg(test)]
mod tests {
    /// **답을 기다리는 창을 새로 만들면 목록에도 넣어야 한다.**
    ///
    /// 안 넣으면 그 창만 웹 화면에 가려지고, 그 창만 겹쳐 뜨고, 그 창만 뒤에 뜬다 —
    /// 셋 다 조용히 어긋난다. 그래서 `foreground_modal` 을 쓰는 파일을 모아 놓고,
    /// 그것들이 `blocking_modal_open` 이 보는 상태와 이어져 있는지 눈으로 확인하게 한다.
    ///
    /// 자동으로 판정하지는 않는다 — 어떤 모달이 "답을 기다리는" 것인지는 뜻의 문제라
    /// 기계가 못 가린다(설정 창은 모달이지만 막지 않는다). 대신 **수가 늘면 걸리게** 해서,
    /// 새로 만든 사람이 이 자리를 한 번은 보게 한다.
    #[test]
    fn 확인_창을_새로_만들면_이_목록을_다시_보게_한다() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut users: Vec<String> = Vec::new();
        let rd = std::fs::read_dir(&dir).expect("src 를 읽지 못했다");
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "modal.rs" || name == "modalstate.rs" {
                continue; // 헬퍼 자신과 이 파일.
            }
            let Ok(t) = std::fs::read_to_string(e.path()) else { continue };
            if t.contains("modal::foreground_modal(") {
                users.push(name);
            }
        }
        users.sort();
        // 지금 아는 것들. 늘어나면 여기서 걸린다 — 그때 `blocking_modal_open` 에 넣을지
        // 정하고 이 목록도 함께 고친다.
        let known = [
            "browserrename.rs",
            "bulkconfirm.rs",
            "closeconfirm.rs",
            "copyidui.rs",
            "editorclose.rs",
            "editorconflict.rs",
            "guardui.rs",
            "hostkeyui.rs",
            "onboarding.rs",
            "padrecoverui.rs",
            "paste.rs",
            "pasteconfirm.rs",
            "reconnect.rs",
            "remotecmdui.rs",
            // 찾기·바꾸기와 세션 목록 고르기는 **답을 기다리는 창이 아니다.** 열어 둔 채
            // 다른 일을 해도 되므로, 이것들 때문에 웹 화면을 숨기면 공연히 사라진다.
            "replaceui.rs",
            "sessiondel.rs",
            "sidebarsel.rs",
            "snippetsend.rs",
            "trzszui.rs",
            "updatemodal.rs",
            "worksnapui.rs",
            "worktreeui.rs",
        ];
        let unknown: Vec<&String> =
            users.iter().filter(|u| !known.contains(&u.as_str())).collect();
        assert!(
            unknown.is_empty(),
            "확인 창을 쓰는 새 파일이 있다: {unknown:?}\n\
             답을 기다리는 창이라면 modalstate::blocking_modal_open 에 상태를 더하고,\n\
             아니라면 위 목록에만 이름을 더할 것."
        );
    }
}
