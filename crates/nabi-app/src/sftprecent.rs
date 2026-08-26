//! **다녀온 원격 경로를 스스로 기억한다.**
//!
//! 북마크는 이미 있다. 다만 **손으로 찍어야** 한다 — 지금 필요한 것은 "아까 그 폴더"이고,
//! 그건 대개 찍어 둘 만큼 중요하지 않아서 안 찍어 둔 곳이다.
//!
//! 규칙은 흔한 최근 목록 그대로다: 최신이 앞, 같은 것은 하나만, 정해진 개수에서 끊는다.
//! 다만 **호스트가 다르면 다른 경로다** — `/var/log`는 서버마다 다른 곳이다. 그래서
//! `호스트:경로`로 기억한다.

/// 기억할 개수. 늘리면 목록이 길어져 오히려 못 찾는다.
pub(crate) const MAX: usize = 20;

/// 목록 앞에 넣는다(이미 있으면 앞으로 끌어올린다).
///
/// 순수 함수로 둔 까닭: 최근 목록은 규칙이 단순해 보이지만 "같은 것을 두 번 넣지 않는다"와
/// "넘치면 뒤를 버린다"가 함께 틀리기 쉽다. 화면 없이 시험할 수 있어야 한다.
pub(crate) fn push(list: &mut Vec<String>, entry: &str) {
    if entry.is_empty() {
        return;
    }
    list.retain(|e| e != entry);
    list.insert(0, entry.to_string());
    list.truncate(MAX);
}

/// `호스트:경로` 한 줄을 짓는다. 호스트를 모르면 경로만.
pub(crate) fn key(host: &str, path: &str) -> String {
    match host.is_empty() {
        true => path.to_string(),
        false => format!("{host}:{path}"),
    }
}

/// `호스트:경로`에서 경로만 떼어 낸다(목록에서 눌러 이동할 때 쓴다).
///
/// 경로에도 콜론이 들어갈 수 있으므로 **처음 하나**에서만 자른다. 윈도우 경로(`C:\`)가
/// 오면 콜론이 두 개가 되는데, 그때도 호스트가 앞에 있으므로 이 규칙이 맞다.
pub(crate) fn path_of(entry: &str) -> &str {
    match entry.split_once(':') {
        Some((_, p)) => p,
        None => entry,
    }
}

#[cfg(test)]
mod tests {
    use super::{key, path_of, push, MAX};

    #[test]
    fn the_newest_goes_first() {
        let mut v = Vec::new();
        push(&mut v, "a:/one");
        push(&mut v, "a:/two");
        assert_eq!(v, vec!["a:/two", "a:/one"]);
    }

    /// **같은 곳을 두 번 담지 않는다** — 오가면 목록이 그 둘로만 가득 찬다.
    #[test]
    fn revisiting_moves_it_up_instead_of_duplicating() {
        let mut v = Vec::new();
        for p in ["a:/one", "a:/two", "a:/one"] {
            push(&mut v, p);
        }
        assert_eq!(v, vec!["a:/one", "a:/two"], "같은 경로가 두 번 들어갔다");
    }

    /// 넘치면 **뒤에서** 버린다(오래된 것부터).
    #[test]
    fn the_list_stops_growing() {
        let mut v = Vec::new();
        for i in 0..(MAX + 5) {
            push(&mut v, &format!("a:/p{i}"));
        }
        assert_eq!(v.len(), MAX);
        assert_eq!(v[0], format!("a:/p{}", MAX + 4), "최신이 앞이 아니다");
        assert!(!v.contains(&"a:/p0".to_string()), "가장 오래된 것이 남았다");
    }

    #[test]
    fn an_empty_path_is_not_remembered() {
        let mut v = Vec::new();
        push(&mut v, "");
        assert!(v.is_empty());
    }

    /// **호스트가 다르면 다른 곳이다** — `/var/log`는 서버마다 다르다.
    #[test]
    fn the_same_path_on_two_hosts_is_two_entries() {
        let mut v = Vec::new();
        push(&mut v, &key("web", "/var/log"));
        push(&mut v, &key("db", "/var/log"));
        assert_eq!(v.len(), 2, "서로 다른 서버의 경로가 하나로 합쳐졌다");
    }

    #[test]
    fn the_path_comes_back_out_whole() {
        assert_eq!(path_of("web:/var/log"), "/var/log");
        assert_eq!(path_of("/var/log"), "/var/log");
        // 경로 안의 콜론은 살아남아야 한다.
        assert_eq!(path_of("web:/data/a:b"), "/data/a:b");
    }

    #[test]
    fn a_missing_host_leaves_the_path_alone() {
        assert_eq!(key("", "/srv"), "/srv");
    }
}
