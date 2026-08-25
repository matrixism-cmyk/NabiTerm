//! 명령 기록 **고르기** — 창이 쓸 순수 함수들.
//!
//! 기록 자체는 `cmdhist`가 이미 모으고 있었다(명령·작업폴더·종료코드·시각). 그런데 그것을
//! 볼 수 있는 곳은 팔레트의 몇 줄뿐이라, **모아 놓고도 되찾을 수가 없었다.** 여기서
//! 고르고 추리는 규칙을 순수 함수로 두고, 창은 그리기만 한다.
//!
//! 목록은 화면 폭이 정해져 있으니 상한이 필요하다. 상한을 두면 "다 보여 준다"는 착각이
//! 생기므로, 창이 **잘린 개수를 함께 말한다**(silent truncation 금지).

/// 걸러진 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub cmd: String,
    pub cwd: String,
    pub exit: i32,
    pub ts: i64,
}

/// 무엇을 볼 것인가.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Filter {
    /// 실패한 것만(종료 코드가 0이 아닌 것).
    pub failed_only: bool,
    /// 지금 pane의 작업 폴더에서 실행한 것만.
    pub this_dir_only: bool,
}

/// 기록에서 조건에 맞는 것을 **최신순**으로 고른다.
///
/// `limit`는 돌려줄 최대 개수, 두 번째 반환값은 조건에 맞았지만 잘려 나간 개수다.
pub(crate) fn select(
    hist: &[(String, String, i32, i64)],
    query: &str,
    f: Filter,
    cwd: &str,
    limit: usize,
) -> (Vec<Row>, usize) {
    let q = query.trim().to_lowercase();
    let mut hits: Vec<Row> = Vec::new();
    let mut total = 0usize;
    // 뒤에서부터 = 최신부터.
    for (cmd, dir, exit, ts) in hist.iter().rev() {
        if f.failed_only && *exit == 0 {
            continue;
        }
        if f.this_dir_only && !cwd.is_empty() && dir != cwd {
            continue;
        }
        if !q.is_empty() && !cmd.to_lowercase().contains(&q) && !dir.to_lowercase().contains(&q) {
            continue;
        }
        total += 1;
        if hits.len() < limit {
            hits.push(Row { cmd: cmd.clone(), cwd: dir.clone(), exit: *exit, ts: *ts });
        }
    }
    let cut = total.saturating_sub(hits.len());
    (hits, cut)
}

/// 화면에 넣을 짧은 명령 글. 너무 길면 줄이되 **앞쪽을 남긴다**(명령은 앞이 정보다).
pub(crate) fn short(cmd: &str, max: usize) -> String {
    if cmd.chars().count() <= max {
        return cmd.to_string();
    }
    let head: String = cmd.chars().take(max.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hist() -> Vec<(String, String, i32, i64)> {
        vec![
            ("cargo build".into(), "C:/a".into(), 0, 100),
            ("cargo test".into(), "C:/a".into(), 1, 200),
            ("git status".into(), "C:/b".into(), 0, 300),
            ("git push".into(), "C:/b".into(), 128, 400),
        ]
    }

    /// 최신이 위다 — 기록은 아래로 쌓이지만 사람은 방금 것을 먼저 찾는다.
    #[test]
    fn the_newest_comes_first() {
        let (rows, _) = select(&hist(), "", Filter::default(), "", 10);
        assert_eq!(rows[0].cmd, "git push");
        assert_eq!(rows[3].cmd, "cargo build");
    }

    /// 실패만 보기 — 무엇이 안 됐는지 되찾는 것이 이 창의 가장 흔한 쓰임이다.
    #[test]
    fn failures_can_be_isolated() {
        let f = Filter { failed_only: true, ..Default::default() };
        let (rows, _) = select(&hist(), "", f, "", 10);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.exit != 0));
    }

    #[test]
    fn a_folder_filter_narrows_to_that_folder() {
        let f = Filter { this_dir_only: true, ..Default::default() };
        let (rows, _) = select(&hist(), "", f, "C:/b", 10);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.cwd == "C:/b"));
    }

    /// 작업 폴더를 모를 때 이 조건을 켜 두면 아무것도 안 보이면 안 된다 — 조건을 무시한다.
    #[test]
    fn an_unknown_folder_does_not_empty_the_list() {
        let f = Filter { this_dir_only: true, ..Default::default() };
        let (rows, _) = select(&hist(), "", f, "", 10);
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn searching_matches_command_and_folder() {
        let (by_cmd, _) = select(&hist(), "git", Filter::default(), "", 10);
        assert_eq!(by_cmd.len(), 2);
        let (by_dir, _) = select(&hist(), "C:/a", Filter::default(), "", 10);
        assert_eq!(by_dir.len(), 2);
    }

    /// **잘렸으면 몇 개가 잘렸는지 말해야 한다.** 조용히 자르면 "이게 전부"로 읽힌다.
    #[test]
    fn truncation_is_reported_not_hidden() {
        let (rows, cut) = select(&hist(), "", Filter::default(), "", 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(cut, 2, "잘린 개수를 말하지 않았다");
    }

    #[test]
    fn nothing_matching_is_empty_and_says_zero_cut() {
        let (rows, cut) = select(&hist(), "zzz", Filter::default(), "", 10);
        assert!(rows.is_empty());
        assert_eq!(cut, 0);
    }

    /// 긴 명령은 앞을 남긴다 — 뒤를 남기면 무슨 명령인지 알 수 없다.
    #[test]
    fn a_long_command_keeps_its_head() {
        let got = short("cargo test --workspace --all-features -- --nocapture", 12);
        assert!(got.starts_with("cargo test"), "{got}");
        assert_eq!(got.chars().count(), 12);
        assert!(got.ends_with('\u{2026}'));
    }

    #[test]
    fn a_short_command_is_left_alone() {
        assert_eq!(short("ls", 12), "ls");
    }

    /// 한글 명령도 글자 수로 잘라야 한다 — 바이트로 자르면 글자가 깨진다.
    #[test]
    fn multibyte_commands_are_cut_by_characters() {
        let got = short("가나다라마바사아자차", 5);
        assert_eq!(got.chars().count(), 5);
    }
}
