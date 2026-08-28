//! **접속 이력** — 언제 어디에 붙었고 얼마나 있었나.
//!
//! 지금 남는 것은 세션별 `last_connected`(마지막 시각 하나)와 마지막 실패 이유뿐이다.
//! 그래서 답할 수 없는 질문들이 있다.
//!
//! * "어제 그 서버에 몇 시에 붙었더라" — 사고 조사·업무 기록에서 늘 필요하다.
//! * "이 세션은 붙으면 금방 끊긴다" — 지속 시간이 남지 않으면 느낌으로만 안다.
//!
//! ## 어디까지만 남기는가
//!
//! **호스트·사용자·시각·지속 시간·끝난 이유**까지다. 명령이나 화면은 남기지 않는다 —
//! 그건 세션 로그(`sessionlog`)의 몫이고, 이력에 섞으면 열어 보기 무서운 파일이 된다.
//! 비밀번호·키는 애초에 여기 오지 않는다.
//!
//! ## 크기를 정해 둔다
//!
//! 무한히 쌓이면 언젠가 열리지 않는다. 가장 최근 [`MAX`]건만 남기고 오래된 것부터 버린다.

use serde::{Deserialize, Serialize};

/// 남길 최대 건수.
pub(crate) const MAX: usize = 500;

/// 접속 한 건.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Entry {
    /// 세션 이름(빠른 연결이면 빈 문자열).
    pub name: String,
    pub host: String,
    pub user: String,
    /// 붙은 시각(unix 초).
    pub at: i64,
    /// 붙어 있던 시간(초). 아직 붙어 있으면 None.
    pub secs: Option<u64>,
    /// 끝난 이유. 정상 종료면 빈 문자열.
    pub why: String,
}

/// 새 접속을 앞에 넣고 상한을 지킨다(최신이 앞).
pub(crate) fn note_open(list: &mut Vec<Entry>, e: Entry) {
    list.insert(0, e);
    list.truncate(MAX);
}

/// 아직 안 끝난 그 호스트의 가장 최근 항목을 닫는다.
///
/// 같은 호스트에 여러 창으로 붙어 있을 수 있다. 그때는 **가장 최근에 연 것**을 닫는다 —
/// 창과 이력을 일대일로 묶으려면 pane 번호를 이력에 넣어야 하는데, 그 번호는 다시 열면
/// 달라지므로 파일에 남길 값이 아니다.
pub(crate) fn note_close(list: &mut [Entry], host: &str, user: &str, now: i64, why: &str) -> bool {
    let Some(e) = list.iter_mut().find(|e| e.host == host && e.user == user && e.secs.is_none()) else {
        return false;
    };
    e.secs = Some(now.saturating_sub(e.at).max(0) as u64);
    e.why = why.to_string();
    true
}

/// 화면에 낼 한 줄: "web-01 · 14:02 · 3시간 12분".
pub(crate) fn human_secs(secs: u64) -> String {
    // 여기만 자리를 안 채워서(1h 1m) 목록에서 숫자가 들쭉날쭉했다. 이제 공용 규칙을 쓴다.
    crate::statusfmt::human_secs(secs)
}

/// 이력 파일 경로. 설정과 같은 폴더에 **따로** 둔다 — 설정이 한 필드 때문에 통째로
/// 초기화되는 위험(`load`가 `unwrap_or_default`)에 이력까지 얽히면 안 된다.
pub(crate) fn path(base: &std::path::Path) -> std::path::PathBuf {
    base.join("connections.json")
}

/// 읽는다. 없거나 깨졌으면 빈 목록 — 이력 때문에 프로그램이 안 뜨면 안 된다.
pub(crate) fn load(base: &std::path::Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(path(base)) else { return Vec::new() };
    serde_json::from_str(&text).unwrap_or_default()
}

/// 쓴다. 실패는 조용히 넘긴다 — 이력을 못 남기는 것이 작업을 막을 이유는 아니다.
pub(crate) fn save(base: &std::path::Path, list: &[Entry]) {
    if let Ok(text) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(path(base), text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(host: &str, at: i64) -> Entry {
        Entry { name: "s".into(), host: host.into(), user: "u".into(), at, secs: None, why: String::new() }
    }

    #[test]
    fn the_newest_connection_is_first() {
        let mut v = Vec::new();
        note_open(&mut v, open("a", 100));
        note_open(&mut v, open("b", 200));
        assert_eq!(v[0].host, "b");
    }

    #[test]
    fn closing_records_how_long_it_lasted() {
        let mut v = Vec::new();
        note_open(&mut v, open("a", 100));
        assert!(note_close(&mut v, "a", "u", 400, ""));
        assert_eq!(v[0].secs, Some(300));
    }

    /// 같은 호스트에 두 번 붙어 있으면 **가장 최근 것**부터 닫는다.
    #[test]
    fn the_most_recent_open_entry_is_the_one_closed() {
        let mut v = Vec::new();
        note_open(&mut v, open("a", 100));
        note_open(&mut v, open("a", 200));
        note_close(&mut v, "a", "u", 250, "");
        assert_eq!(v[0].secs, Some(50), "나중에 연 것이 닫혀야 한다");
        assert_eq!(v[1].secs, None, "먼저 연 것은 아직 열려 있다");
    }

    /// 열린 적 없는 것을 닫으려 하면 조용히 아무 일도 하지 않는다(이력을 지어내지 않는다).
    #[test]
    fn closing_something_never_opened_does_nothing() {
        let mut v = Vec::new();
        assert!(!note_close(&mut v, "ghost", "u", 1, ""));
        assert!(v.is_empty());
    }

    /// 끝난 이유가 남아야 "왜 끊겼나"를 나중에 볼 수 있다.
    #[test]
    fn the_reason_it_ended_is_kept() {
        let mut v = Vec::new();
        note_open(&mut v, open("a", 0));
        note_close(&mut v, "a", "u", 10, "Connection reset");
        assert_eq!(v[0].why, "Connection reset");
    }

    /// **무한히 쌓이지 않는다.** 오래된 것부터 버린다.
    #[test]
    fn the_history_has_a_ceiling() {
        let mut v = Vec::new();
        for i in 0..MAX + 30 {
            note_open(&mut v, open("h", i as i64));
        }
        assert_eq!(v.len(), MAX);
        assert_eq!(v[0].at, (MAX + 29) as i64, "최신이 앞에 있어야 한다");
    }

    /// 시계가 거꾸로 가도(시각 보정) 음수 시간이 나오면 안 된다.
    #[test]
    fn a_backwards_clock_does_not_produce_negative_time() {
        let mut v = Vec::new();
        note_open(&mut v, open("a", 500));
        note_close(&mut v, "a", "u", 100, "");
        assert_eq!(v[0].secs, Some(0));
    }

    #[test]
    fn durations_read_in_the_largest_useful_unit() {
        // 모양은 이제 `statusfmt::human_secs` 하나가 정한다 — 여기만 자리를 안 채워서
        // ("2m 5s"·"2h 2m") 목록의 숫자가 들쭉날쭉했다(배치 AD).
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(125), "2m 05s");
        assert_eq!(human_secs(7_320), "2h 02m");
    }

    /// 저장했다 읽으면 그대로여야 한다 — 이력의 쓸모는 **다음에 열었을 때** 나온다.
    #[test]
    fn the_history_survives_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("nabi-ch-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut v = Vec::new();
        note_open(&mut v, open("a", 100));
        note_close(&mut v, "a", "u", 160, "bye");
        save(&dir, &v);
        assert_eq!(load(&dir), v);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 파일이 깨져 있어도 빈 목록으로 시작한다 — 이력 때문에 프로그램이 안 뜨면 안 된다.
    #[test]
    fn a_corrupt_file_does_not_stop_startup() {
        let dir = std::env::temp_dir().join(format!("nabi-ch-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path(&dir), "{{{ 이건 JSON이 아니다");
        assert!(load(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
