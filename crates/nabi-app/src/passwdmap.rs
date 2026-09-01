//! **번호를 이름으로** — 원격의 `/etc/passwd`·`/etc/group` 을 읽어 uid/gid 를 사람 이름으로.
//!
//! ## 왜 필요한가
//!
//! 소유자 열에 `1000` 이라고만 적히면 그게 나인지 남인지 알 수 없다. 서버를 다루는 사람이
//! 알고 싶은 것은 번호가 아니라 **누구인가**다.
//!
//! ## 왜 `longname` 을 안 쓰는가
//!
//! SFTP v3 응답에는 `ls -l` 을 흉내 낸 `longname` 줄이 함께 오고 거기에는 이름이 들어 있다.
//! 그런데 못 믿는다 — OpenSSH 자신도 상황에 따라 이름 대신 숫자를 내보내고, 줄 모양은
//! 서버 구현마다 다르다(2026-09-01 조사). 반면 `/etc/passwd` 의 모양은 스무 해 넘게 같다.
//!
//! ## 언제 읽는가
//!
//! **소유자·그룹 열을 켰을 때만, 접속당 한 번씩.** 켜지도 않은 사람의 서버에서 남의 계정
//! 목록을 읽어 올 이유가 없다. 상한을 걸어 받으므로(미리보기 명령을 그대로 쓴다) 아주 큰
//! 파일이 와도 앞부분만 본다.
//!
//! ## 못 읽어도 그만이다
//!
//! 권한이 없거나(읽기 금지), 파일이 없거나(윈도우 서버), LDAP 라서 로컬 파일에 없을 수
//! 있다. 그때는 **번호를 그대로 보여 준다** — 이름을 모른다고 열이 비면 더 나쁘다.

use std::collections::BTreeMap;

/// uid/gid → 이름.
pub(crate) type IdMap = BTreeMap<u32, String>;

/// 원격에서 읽어 올 파일. `(경로, 번호가 몇 번째 칸인가)`.
///
/// `/etc/passwd` 는 `이름:x:uid:gid:...` 라 uid 가 셋째 칸이고,
/// `/etc/group` 은 `이름:x:gid:...` 라 gid 가 셋째 칸이다 — 마침 자리가 같다.
pub(crate) const PASSWD: &str = "/etc/passwd";
pub(crate) const GROUP: &str = "/etc/group";

/// 한 번에 받아 올 최대 바이트. 계정 만 개짜리 서버도 이 안에 든다(한 줄 ~60바이트).
pub(crate) const MAX: usize = 512 * 1024;

/// `이름:x:번호:...` 꼴 줄들을 번호→이름 지도로.
///
/// 두 파일이 같은 함수를 쓴다 — `passwd` 도 `group` 도 셋째 칸이 번호이기 때문이다.
/// 모양이 다른 줄은 **조용히 건너뛴다.** 남의 서버 파일이라 무엇이 들어 있을지 모르고,
/// 한 줄이 이상하다고 나머지 이름을 다 버리면 손해만 크다.
pub(crate) fn parse_ids(text: &str) -> IdMap {
    let mut out = IdMap::new();
    for line in text.lines() {
        let line = line.trim();
        // 주석과 빈 줄. `+`·`-` 로 시작하는 NIS 줄도 이름이 아니다.
        if line.is_empty() || line.starts_with('#') || line.starts_with(['+', '-']) {
            continue;
        }
        let mut f = line.split(':');
        let (Some(name), Some(_), Some(id)) = (f.next(), f.next(), f.next()) else { continue };
        let (Ok(id), false) = (id.parse::<u32>(), name.is_empty()) else { continue };
        // 먼저 나온 이름을 남긴다 — 같은 번호에 여러 이름이 있으면 앞의 것이 주 계정이다.
        out.entry(id).or_insert_with(|| name.to_string());
    }
    out
}

/// 화면에 적을 글 — 이름을 알면 이름, 모르면 번호, 번호도 모르면 빈칸.
///
/// **번호를 괄호로 덧붙이지 않는다.** 열은 좁고, 이름을 아는 순간 번호는 알 필요가 없다.
/// 번호가 필요하면 열을 끄면 된다(그러면 권한 열 옆에 그대로 나온다).
pub(crate) fn name_of(map: &IdMap, id: Option<u32>) -> String {
    match id {
        None => String::new(),
        Some(n) => map.get(&n).cloned().unwrap_or_else(|| n.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_usual_passwd_shape() {
        let m = parse_ids("root:x:0:0:root:/root:/bin/bash\nkim:x:1000:1000::/home/kim:/bin/sh\n");
        assert_eq!(m.get(&0).map(String::as_str), Some("root"));
        assert_eq!(m.get(&1000).map(String::as_str), Some("kim"));
    }

    /// `/etc/group` 도 셋째 칸이 번호라 같은 함수로 읽는다.
    #[test]
    fn the_group_file_uses_the_same_shape() {
        let m = parse_ids("wheel:x:10:kim,lee\ndocker:x:999:\n");
        assert_eq!(m.get(&10).map(String::as_str), Some("wheel"));
        assert_eq!(m.get(&999).map(String::as_str), Some("docker"));
    }

    /// **한 줄이 이상하다고 나머지를 버리면 안 된다.** 남의 서버 파일이다.
    #[test]
    fn a_broken_line_does_not_lose_the_rest() {
        let m = parse_ids("# 주석\n\nrubbish\nbad:x:notanumber:1\n:x:5:5\nok:x:7:7\n");
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!(m.get(&7).map(String::as_str), Some("ok"));
    }

    /// NIS 줄(`+`·`-`)은 계정 이름이 아니다.
    #[test]
    fn nis_lines_are_skipped() {
        assert!(parse_ids("+::::::\n+@admins::::::\n-baduser:::::: \n").is_empty());
    }

    /// 같은 번호가 여러 번 나오면 **먼저 나온 이름**을 쓴다(주 계정이 위에 있다).
    #[test]
    fn the_first_name_wins_for_a_shared_id() {
        let m = parse_ids("root:x:0:0\ntoor:x:0:0\n");
        assert_eq!(m.get(&0).map(String::as_str), Some("root"));
    }

    /// 이름을 모르면 번호를, 번호도 모르면 빈칸을 보여 준다.
    #[test]
    fn unknown_ids_fall_back_to_the_number() {
        let m = parse_ids("kim:x:1000:1000\n");
        assert_eq!(name_of(&m, Some(1000)), "kim");
        assert_eq!(name_of(&m, Some(4242)), "4242", "모르는 번호는 번호 그대로");
        assert_eq!(name_of(&m, None), "", "모르는 것은 빈칸");
        assert_eq!(name_of(&IdMap::new(), Some(0)), "0", "지도가 비어도 0 은 0 이다");
    }
}
