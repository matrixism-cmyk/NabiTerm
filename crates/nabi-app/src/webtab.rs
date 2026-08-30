//! 웹 화면을 **탭 안에** 둔다(배치 AZ).
//!
//! ## 왜 별도 창이 아니라 탭인가
//!
//! 별도 창은 임시 형태였다. 탭에 있어야 다른 것과 나란히 놓고, 분할해서 보고, 작업 공간에
//! 함께 저장된다(사용자 요청 2026-08-29).
//!
//! ## 자식 창이라 지켜야 하는 것
//!
//! WebView2 는 운영체제가 자기 창에 그린다. 그 창은 우리가 egui 로 그리는 것보다 **늘 위에**
//! 온다. 그래서 우리가 직접 숨기고 옮겨 줘야 한다.
//!
//! * **안 보이는 탭이면 숨긴다.** 안 숨기면 다른 탭 위에 웹 화면이 그대로 남는다.
//! * **자리는 그 탭이 그려질 때 받아 둔다.** 매 프레임 맞춘다 — 탭을 옮기거나 창 크기가
//!   바뀌면 따라와야 한다.
//!
//! ## 만드는 시점
//!
//! 탭을 열 때 바로 만들지 않는다. 그때는 **아직 자리를 모른다**(화면에 그려져 봐야 안다).
//! 처음 그려질 때 만든다.

use nabi_types::PaneId;

/// 탭 하나가 들고 있는 것.
pub(crate) struct WebTab {
    /// 아직 안 만들었으면 `None`. 처음 그려질 때 만든다.
    pub view: Option<nabi_web::embed::Embedded>,
    /// 열려고 하는 주소. 만들 때 쓰고, 주소 칸에도 보여 준다.
    pub url: String,
    /// 만들다 실패했으면 그 이유. 화면에 그대로 보여 준다 — 빈 탭만 두지 않는다.
    pub failed: Option<String>,
    /// 지금 쪽의 제목 — 탭 이름으로 쓴다. 그릴 때마다 웹 화면에 물어 갱신한다.
    ///
    /// 여기 담아 두는 까닭은, 탭 이름을 정하는 곳에서는 웹 화면에 물어볼 수 없어서다.
    /// 그 자리는 안 보이는 탭의 이름도 정해야 하는데, 안 보이는 탭은 화면이 숨겨져 있다.
    pub title: String,
    /// 이 쪽을 PDF 로 저장해 달라는 요청(도구 줄 메뉴에서 켠다). 중앙이 받아 처리한다.
    ///
    /// 그리는 자리에서 바로 저장하지 않는 까닭은, 파일 자리를 묻는 창을 띄우려면 앱을
    /// 만져야 하는데 그리는 함수는 앱을 모르기 때문이다.
    pub want_pdf: bool,
}

impl WebTab {
    pub(crate) fn new(url: &str) -> Self {
        Self {
            view: None,
            url: url.to_string(),
            failed: None,
            title: String::new(),
            want_pdf: false,
        }
    }
}

impl crate::app::NabiApp {
    /// 웹 탭을 하나 연다.
    pub(crate) fn open_web_tab(&mut self, url: &str) -> PaneId {
        let p = nabi_types::next_pane_id(); // 오케스트레이터 pane 없는 UI 전용 id.
        self.web_tabs.insert(p, WebTab::new(url));
        self.add_pane(p);
        p
    }

    /// 안 보이는 웹 탭을 전부 숨긴다.
    ///
    /// 매 프레임 부른다. 그려진 탭은 자기가 다시 보이게 하므로, 여기서 한 번 다 숨겨도
    /// 깜빡이지 않는다 — 같은 값이면 아무 일도 하지 않기 때문이다.
    pub(crate) fn hide_unseen_web_tabs(&mut self, seen: &std::collections::HashSet<PaneId>) {
        for (id, tab) in self.web_tabs.iter_mut() {
            if !seen.contains(id) {
                if let Some(v) = &mut tab.view {
                    v.show(false);
                }
            }
        }
    }

    /// 열려 있는 웹 탭의 주소를 도크 순서대로 저장한다.
    ///
    /// 파일 브라우저 탭과 같은 방식이다(`.btabs` 옆에 `.wtabs`). 주소만 적는다 —
    /// 웹 화면 자체는 다시 만들면 되고, 로그인 상태는 엣지가 알아서 들고 있다.
    pub(crate) fn save_web_tabs(&self) {
        let urls: Vec<String> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, p)| self.web_tabs.get(p))
            .map(|w| w.url.clone())
            .collect();
        let path = self.workspace_path.with_extension("wtabs");
        match urls.is_empty() {
            true => {
                let _ = std::fs::remove_file(path);
            }
            false => {
                if let Ok(s) = ron::to_string(&urls) {
                    // 삼킴: 웹 탭 주소다. 못 남기면 다시 켤 때 안 되살아난다.
                    let _ = std::fs::write(path, s);
                }
            }
        }
    }

    /// 저장된 웹 탭들을 다시 연다.
    ///
    /// 하나가 깨져도 나머지는 연다 — 한 줄 때문에 열두 탭을 잃을 이유가 없다.
    pub(crate) fn restore_web_tabs(&mut self) {
        let Ok(s) = std::fs::read_to_string(self.workspace_path.with_extension("wtabs")) else {
            return;
        };
        let (urls, _dropped) = crate::ronsalvage::parse_vec::<String>(&s);
        for u in urls {
            self.open_web_tab(&u);
        }
    }

    /// 도구 줄에서 켠 "PDF 로 저장"을 처리한다. 매 프레임 부른다.
    ///
    /// 그리는 자리가 아니라 여기서 하는 까닭은, 파일 자리를 묻는 창이 앱을 만져야 하기
    /// 때문이다. 그리는 함수는 앱을 모른다.
    pub(crate) fn tick_web_pdf(&mut self) {
        let Some(pane) = self.web_tabs.iter().find(|(_, w)| w.want_pdf).map(|(p, _)| *p) else {
            return;
        };
        // 요청은 한 번만 지운다 — 창을 띄우는 동안 프레임이 여러 번 돈다.
        if let Some(w) = self.web_tabs.get_mut(&pane) {
            w.want_pdf = false;
        }
        let name = self
            .web_tabs
            .get(&pane)
            .map(|w| pdf_name(&w.title, &w.url))
            .unwrap_or_else(|| "page.pdf".into());
        let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file() else {
            return;
        };
        let out = path.display().to_string();
        if let Some(v) = self.web_tabs.get(&pane).and_then(|w| w.view.as_ref()) {
            let note = out.clone();
            v.print_pdf(&out, move |r| match r {
                // 콜백은 UI 실에서 돌지만 앱을 만질 수는 없다(빌림) — 로그로 남긴다.
                Ok(()) => tracing::info!(target: "web", %note, "PDF 저장"),
                Err(e) => tracing::warn!(target: "web", %e, "PDF 저장 실패"),
            });
        }
        self.notify = Some((format!("\u{1f4c4} {out}"), std::time::Instant::now()));
    }

    /// 닫힌 웹 탭을 치운다.
    ///
    /// 다른 UI 전용 탭(파일 브라우저·편집기)과 같은 길이다. 치우면서 `Embedded` 가
    /// 떨어지고, 그때 엣지 프로세스도 함께 닫힌다 — 안 치우면 프로세스가 남는다.
    pub(crate) fn close_web_tab(&mut self, pane: PaneId) {
        self.web_tabs.remove(&pane);
    }
}

/// 긴 주소를 탭 이름에 쓸 만큼 줄인다 — 호스트 이름만 남긴다.
///
/// `https://github.com/matrixism-cmyk/NabiTerm` 를 그대로 쓰면 탭이 통째로 주소가 된다.
/// 쪽 제목을 아직 못 읽었을 때만 쓰는 임시 이름이다.
pub(crate) fn short_url(url: &str) -> String {
    let no_scheme = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = no_scheme.split(['/', '?', '#']).next().unwrap_or(no_scheme);
    match host.is_empty() {
        true => url.to_string(),
        false => host.to_string(),
    }
}

/// 탭에 붙일 이름 — 쪽 제목이 있으면 그것을, 없으면 주소를 줄여 쓴다.
///
/// 길이를 자르는 까닭은 화면으로 확인했기 때문이다. 깃허브 쪽 제목을 그대로 달았더니
/// 탭 하나가 창을 가로질러 다른 탭들을 전부 밀어냈다. 다른 탭들(파일·편집기)은 파일
/// 이름만 달고 있어서 짧다 — 웹만 길면 나란히 놓을 수 없다.
pub(crate) fn tab_name(title: &str, url: &str) -> String {
    const MAX: usize = 24;
    let full = match title.is_empty() {
        false => title.to_string(),
        true => short_url(url),
    };
    match full.chars().count() > MAX {
        false => full,
        true => full.chars().take(MAX - 1).collect::<String>() + "\u{2026}",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn 긴_제목은_줄인다() {
        // 짧으면 그대로 둔다.
        assert_eq!(super::tab_name("GitHub", "https://github.com"), "GitHub");
        // 길면 잘리고 말줄임표가 붙는다 — 세는 단위는 글자다(한글도 한 글자).
        let long = super::tab_name("가나다라마바사아자차카타파하가나다라마바사아자차카타", "");
        assert_eq!(long.chars().count(), 24);
        assert!(long.ends_with('\u{2026}'));
        // 제목이 없으면 주소에서 호스트만.
        assert_eq!(super::tab_name("", "https://example.com/a/b"), "example.com");
    }

    #[test]
    fn 주소에서_호스트만_남긴다() {
        assert_eq!(super::short_url("https://github.com/a/b?c=1"), "github.com");
        assert_eq!(super::short_url("example.com"), "example.com");
        // 로컬 파일은 자를 곳이 없다 — 그대로 둔다.
        assert_eq!(super::short_url("about:blank"), "about:blank");
    }
}

/// PDF 파일 이름을 짓는다 — 쪽 제목에서 파일에 못 쓰는 글자를 뺀다.
///
/// 제목을 그대로 쓰면 `?` 나 `:` 때문에 저장이 실패한다. 실패는 저장 창을 닫은 뒤에야
/// 드러나서, 사용자는 눌렀는데 아무 일도 안 일어난 것으로 본다.
pub(crate) fn pdf_name(title: &str, url: &str) -> String {
    const BAD: &str = r#"\/:*?"<>|"#;
    let base = match title.trim().is_empty() {
        false => title.trim(),
        true => url,
    };
    let safe: String = base
        .chars()
        .map(|c| match BAD.contains(c) {
            true => '_',
            false => c,
        })
        .take(60)
        .collect();
    format!("{}.pdf", safe.trim())
}

#[cfg(test)]
mod pdfname_tests {
    #[test]
    fn 파일에_못_쓰는_글자를_바꾼다() {
        assert_eq!(super::pdf_name("a/b:c?d", ""), "a_b_c_d.pdf");
        // 제목이 없으면 주소를 쓴다.
        assert_eq!(super::pdf_name("  ", "https://x.com/a"), "https___x.com_a.pdf");
    }
}
