//! **선택 열** — 이름·유형·크기·수정일 말고 무엇을 더 보여 줄지.
//!
//! ## 왜 설정 스위치가 아니라 목록인가
//!
//! 처음에는 "열 하나 더 보기" 스위치 하나였다. 그런데 더 보여 줄 것은 하나가 아니다 —
//! 로컬에는 속성·만든 날짜·확장자가 있고, 원격에는 권한이 있다. 스위치를 하나씩 늘리면
//! 설정 화면이 열 개수만큼 길어지고, 정작 **열을 보고 있는 자리에서는 못 바꾼다.**
//!
//! 윈도우 탐색기는 스무 해 넘게 **머리글을 오른쪽 클릭**해서 고르게 했다. 보고 있는
//! 자리에서 바로 켜고 끄는 것이 맞다. 그래서 켠 열의 **이름 목록**을 설정에 담는다.
//!
//! ## 순서는 설정이 아니라 카탈로그가 정한다
//!
//! 설정 파일에 적힌 순서를 그대로 쓰면, 손으로 고친 파일이나 옛 판이 남긴 순서 때문에
//! 사람마다 열 차례가 달라진다. 켜고 끄는 것만 사용자가 정하고 **차례는 여기 적힌
//! 순서**를 따른다 — 그래야 화면을 설명할 때 "네 번째 열"이 모두에게 같은 뜻이 된다.
//!
//! ## 모르는 이름은 조용히 버린다
//!
//! 설정에 우리가 모르는 열 이름이 들어 있을 수 있다(옛 판·손편집·다음 판에서 없앤 열).
//! 그때 오류를 내면 설정 하나 때문에 목록이 안 뜬다. 카탈로그에 없는 이름은 그냥 지나간다.

/// 한 선택 열: `(설정에 적히는 이름, 머리글 i18n 키)`.
pub(crate) type Col = (&'static str, &'static str);

/// 로컬 탐색기가 더 보여 줄 수 있는 열(차례 고정).
pub(crate) const LOCAL: [Col; 3] = [
    ("attrs", "browser.col.attrs"),
    ("created", "browser.col.created"),
    ("ext", "browser.col.ext"),
];

/// 원격 SFTP 가 더 보여 줄 수 있는 열(차례 고정).
///
/// 차례는 `ls -l` 을 따른다 — 권한·소유자·그룹. 서버를 다루는 사람의 눈이 이미 그 순서에
/// 익어 있어서, 다르게 놓으면 매번 다시 읽게 된다.
pub(crate) const REMOTE: [Col; 3] = [
    ("perms", "browser.col.perms"),
    ("owner", "browser.col.owner"),
    ("group", "browser.col.group"),
];

// 번호를 글로 바꾸는 일은 [`crate::passwdmap::name_of`] 가 한다 — 이름을 알면 이름을
// 쓰기 때문에 번호만 아는 함수를 따로 두면 두 벌이 된다.

/// 이 열이 켜져 있나.
pub(crate) fn on(list: &[String], key: &str) -> bool {
    list.iter().any(|s| s == key)
}

/// 켜고 끄기. 켤 때는 뒤에 붙이지만, 보여 줄 때는 카탈로그 차례를 따르므로 상관없다.
pub(crate) fn toggle(list: &mut Vec<String>, key: &str) {
    match list.iter().position(|s| s == key) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(key.to_string()),
    }
}

/// 켜진 열을 **카탈로그 차례대로**. 모르는 이름은 여기서 사라진다.
pub(crate) fn enabled(cat: &[Col], list: &[String]) -> Vec<Col> {
    cat.iter().filter(|(k, _)| on(list, k)).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn toggling_turns_it_on_then_off() {
        let mut l = Vec::new();
        toggle(&mut l, "attrs");
        assert!(on(&l, "attrs"));
        toggle(&mut l, "attrs");
        assert!(!on(&l, "attrs"), "두 번 누르면 꺼져야 한다");
        assert!(l.is_empty(), "끈 이름을 남겨 두면 설정 파일이 계속 자란다");
    }

    /// **차례는 설정이 아니라 카탈로그가 정한다.** 설정에 거꾸로 적혀 있어도 같은 차례다.
    #[test]
    fn the_catalogue_decides_the_order_not_the_config() {
        let backwards = v(&["ext", "attrs", "created"]);
        let got: Vec<&str> = enabled(&LOCAL, &backwards).iter().map(|(k, _)| *k).collect();
        assert_eq!(got, ["attrs", "created", "ext"]);
    }

    /// 모르는 이름은 조용히 버린다 — 설정 하나 때문에 목록이 안 뜨면 안 된다.
    #[test]
    fn an_unknown_name_is_ignored_not_an_error() {
        let got = enabled(&LOCAL, &v(&["attrs", "이런열은없다", "perms"]));
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].0, "attrs", "원격 전용 열이 로컬에 끼어들면 안 된다");
    }

    /// 아무것도 안 켜면 기본 네 열만 나온다.
    #[test]
    fn nothing_enabled_means_no_extra_columns() {
        assert!(enabled(&LOCAL, &[]).is_empty());
        assert!(enabled(&REMOTE, &[]).is_empty());
    }

    /// **두 카탈로그의 이름이 겹치면 안 된다** — 겹치면 한쪽을 켰을 때 다른 쪽도 켜진다.
    #[test]
    fn the_two_catalogues_do_not_share_names() {
        for (k, _) in LOCAL {
            assert!(!REMOTE.iter().any(|(r, _)| *r == k), "{k} 가 양쪽에 있다");
        }
    }

    /// 머리글 키는 전부 `browser.col.` 로 시작한다 — i18n 검사기가 짝을 찾는 규칙이다.
    #[test]
    fn every_column_has_a_header_key() {
        for (_, key) in LOCAL.iter().chain(REMOTE.iter()) {
            assert!(key.starts_with("browser.col."), "{key}");
        }
    }
}
