//! 설정 **검색** — 낱말로 항목을 찾아 그 페이지로 보낸다.
//!
//! 설정이 6페이지 60항목을 넘겼다. 눈으로 훑어 찾는 것은 이미 한계다(VS Code 설정 검색이
//! 그 시점에 나온 이유도 같다).
//!
//! ## 표를 손으로 관리하지 않는다
//!
//! 항목 목록을 사람이 적어 두면 **반드시 어긋난다.** 팔레트 단축키 표에서 이미 겪었고,
//! 그때는 만든 지 몇 분 만에 어긋났다. 그래서 이 표는 소스에서 뽑아 만들었고, 아래
//! `the_index_matches_the_screens` 시험이 **매번 소스를 다시 훑어 대조한다.** 설정 행을
//! 하나 더하고 표를 안 고치면 시험이 먼저 알려 준다.
//!
//! ## 무엇으로 찾는가
//!
//! 화면에 보이는 **번역된 글**로 찾는다. 한국어 사용자가 "글꼴"이라고 치면 찾아야지,
//! `settings.fontsize`를 알아야 할 이유가 없다. 영어 이름으로도 함께 찾는다 — 문서나
//! 다른 프로그램에서 본 낱말로 치는 사람이 있다.

use nabi_i18n::{tr, Lang};

/// (항목 i18n 키, 그 항목이 사는 페이지 번호). **소스에서 생성됨** — 손으로 고치지 말 것.
pub(crate) const INDEX: &[(&str, usize)] = &[
    ("settings.agentsound", 0),
    ("settings.autoreconnect", 0),
    ("settings.logkeep", 0),
    ("settings.builtineditor", 0),
    ("settings.clock", 0),
    ("settings.confirmclose", 0),
    ("settings.control", 0),
    ("settings.control.osc", 0),
    ("settings.copyonselect", 0),
    ("settings.language", 0),
    ("settings.quakehotkey", 0),
    ("settings.restoreai", 0),
    ("settings.restorecmd", 0),
    ("settings.restoreshaai", 0),
    ("settings.restorews", 0),
    ("settings.shellinteg", 0),
    ("settings.shellinteg.install", 0),
    ("settings.splash", 0),
    ("settings.statusbar", 0),
    ("settings.visualbell", 0),
    ("settings.warnpaste", 0),
    ("settings.warnpasteunicode", 0),
    ("settings.bgcolor", 1),
    ("settings.blinkms", 1),
    ("settings.colordefault", 1),
    ("settings.cursorblink", 1),
    ("settings.cursorcolor", 1),
    ("settings.cursorshape", 1),
    ("settings.export.alacritty", 1),
    ("settings.export.wt", 1),
    ("settings.fgcolor", 1),
    ("settings.fontfamily", 1),
    ("settings.fontsize", 1),
    ("settings.import.apply", 1),
    ("settings.matchcolor", 1),
    ("settings.preview", 1),
    ("settings.resetdefault", 1),
    ("settings.selectioncolor", 1),
    ("settings.defaultcwd", 2),
    ("settings.encoding", 2),
    ("settings.osc52", 2),
    ("settings.scrollback", 2),
    ("settings.searchlimit", 2),
    ("settings.shell", 2),
    ("settings.tipai", 2),
    ("settings.tipcachepath", 2),
    ("settings.tipoverlay", 2),
    ("settings.exteditor", 3),
    ("settings.downloadask", 3),
    ("settings.downloaddir", 3),
    ("settings.maxparallel", 3),
    ("settings.sftpcharset", 3),
    ("settings.sftpcharset.auto", 3),
    ("settings.speedlimit", 3),
    ("settings.palette", 6),
    ("settings.a11ymarks", 6),
    ("settings.contrast", 6),
    ("settings.contrastlow", 6),
    ("settings.offline", 3),
    ("settings.publicip", 3),
    ("settings.redacthist", 3),
    ("settings.redactlogs", 3),
    ("settings.autolog", 3),
    ("settings.sshtimeout", 3),
    ("settings.sshkeepalive", 3),
    ("settings.statsalert", 3),
    ("settings.slowcmd", 3),
    ("settings.connhist", 3),
    ("settings.uploadmode", 3),
    ("settings.verifyhash", 3),
    ("settings.addalert", 4),
    ("settings.autoreply", 4),
    ("settings.addhighlight", 4),
    ("settings.addsnippet", 4),
    ("settings.alertactions", 4),
];

/// 검색 결과 한 줄.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Hit {
    pub key: &'static str,
    pub page: usize,
}

/// 질의에 걸리는 항목들. 빈 질의는 아무것도 내놓지 않는다(전체 목록은 페이지가 이미 보여 준다).
///
/// 앞에서부터 맞는 것을 먼저 준다 — "글꼴 크기"를 찾을 때 "글꼴"로 시작하는 항목이
/// 가운데에 그 낱말이 낀 항목보다 먼저 나와야 한다.
pub(crate) fn find(query: &str, lang: Lang) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut starts: Vec<Hit> = Vec::new();
    let mut inside: Vec<Hit> = Vec::new();
    for (key, page) in INDEX {
        let label = tr(lang, key).to_lowercase();
        let english = tr(Lang::En, key).to_lowercase();
        let hit = Hit { key, page: *page };
        if label.starts_with(&q) || english.starts_with(&q) {
            starts.push(hit);
        } else if label.contains(&q) || english.contains(&q) || key.contains(&q) {
            inside.push(hit);
        }
    }
    starts.extend(inside);
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 빈 질의에 60개를 쏟아 놓으면 검색이 아니라 방해다.
    #[test]
    fn an_empty_query_finds_nothing() {
        assert!(find("", Lang::Ko).is_empty());
        assert!(find("   ", Lang::En).is_empty());
    }

    /// **한국어로 친 낱말로 찾혀야 한다** — 키 이름을 아는 사용자는 없다.
    #[test]
    fn korean_words_find_korean_labels() {
        let hits = find("글꼴", Lang::Ko);
        assert!(!hits.is_empty(), "글꼴 관련 항목이 하나도 안 잡혔다");
        assert!(hits.iter().all(|h| h.page == 1), "글꼴은 모양 페이지에 있다: {hits:?}");
    }

    /// 영어 낱말로도 찾힌다 — 다른 프로그램에서 본 이름으로 치는 사람이 있다.
    #[test]
    fn english_words_work_even_in_korean_ui() {
        assert!(!find("font", Lang::Ko).is_empty());
    }

    /// 앞에서 맞는 것이 먼저다.
    #[test]
    fn a_prefix_match_outranks_a_middle_match() {
        let hits = find("scroll", Lang::En);
        if hits.len() >= 2 {
            let first = tr(Lang::En, hits[0].key).to_lowercase();
            assert!(first.starts_with("scroll"), "앞맞춤이 먼저여야 한다: {first}");
        }
    }

    #[test]
    fn nothing_matches_nonsense() {
        assert!(find("zzzznotathing", Lang::Ko).is_empty());
    }

    /// 페이지 번호가 범위를 벗어나면 클릭했을 때 엉뚱한 곳으로 간다.
    #[test]
    fn every_entry_points_at_a_real_page() {
        let pages = crate::settingsui::PAGE_KEYS.len();
        for (key, page) in INDEX {
            assert!(*page < pages, "{key}: 없는 페이지 {page}");
        }
    }

    /// 같은 키가 두 번 있으면 결과가 겹쳐 보인다.
    #[test]
    fn the_index_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (key, _) in INDEX {
            assert!(seen.insert(*key), "중복: {key}");
        }
    }

    /// **이 시험이 이 파일의 핵심이다.** 소스를 다시 훑어 화면에 실제로 그려지는 설정 행과
    /// 표를 대조한다. 행을 더하고 표를 안 고치면 여기서 걸린다(손으로 관리하는 표는
    /// 반드시 어긋난다 — 팔레트 단축키에서 이미 겪었다).
    #[test]
    fn the_index_matches_the_screens() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let want = crate::settingscan::scan(&dir);
        assert!(want.len() > 40, "훑기가 망가졌다(찾은 항목 {}개)", want.len());
        let have: std::collections::HashSet<&str> = INDEX.iter().map(|(k, _)| *k).collect();
        let missing: Vec<&String> = want.iter().filter(|(k, _)| !have.contains(k.as_str())).map(|(k, _)| k).collect();
        assert!(missing.is_empty(), "설정 화면에는 있는데 검색 표에 없다: {missing:?}");
        let want_keys: std::collections::HashSet<&str> = want.iter().map(|(k, _)| k.as_str()).collect();
        let extra: Vec<&&str> = have.iter().filter(|k| !want_keys.contains(**k)).collect();
        assert!(extra.is_empty(), "검색 표에만 있고 화면에는 없다: {extra:?}");
        for (k, page) in &want {
            let idx = INDEX.iter().find(|(ik, _)| ik == k).unwrap().1;
            assert_eq!(idx, *page, "{k}: 페이지가 어긋났다");
        }
    }
}
