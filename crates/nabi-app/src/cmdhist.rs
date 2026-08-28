//! 명령 히스토리(E5, Atuin식 경량) — 명령+작업디렉터리+종료코드+시각을 기록해 팔레트에서 재실행.
//! SQLite(C 의존) 대신 config 벡터에 보관(상한). OSC133 명령 경계·run_cmd를 재사용. 순수 함수.

/// 한 항목: (명령, 작업디렉터리, 종료코드, unix초).
pub(crate) type CmdHist = Vec<(String, String, i32, i64)>;

/// 명령을 기록한다. 같은 cwd의 동일 명령은 중복 제거(최신만 유지), 상한 초과 시 오래된 것부터 제거.
pub(crate) fn record(h: &mut CmdHist, cmd: &str, cwd: &str, exit: i32, ts: i64, cap: usize) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }
    h.retain(|(c, d, _, _)| !(c == cmd && d == cwd));
    h.push((cmd.to_string(), cwd.to_string(), exit, ts));
    let n = h.len();
    if n > cap {
        h.drain(0..n - cap);
    }
}

/// 최근순 상위 n개 (명령, 작업디렉터리, 종료코드).
pub(crate) fn recent(h: &[(String, String, i32, i64)], n: usize) -> Vec<(String, String, i32)> {
    h.iter().rev().take(n).map(|(c, d, e, _)| (c.clone(), d.clone(), *e)).collect()
}

/// 특정 작업디렉터리에서 실행된 명령만 최근순으로(Atuin식 컨텍스트, F3). cwd 빈 문자열이면 전부.
pub(crate) fn recent_in_cwd(h: &[(String, String, i32, i64)], cwd: &str, n: usize) -> Vec<String> {
    h.iter().rev().filter(|(_, d, _, _)| cwd.is_empty() || d == cwd).take(n).map(|(c, _, _, _)| c.clone()).collect()
}

impl crate::app::NabiApp {
    /// 명령 완료 시 히스토리에 기록한다(run_cmd+cwd+exit, 20s 디바운스 저장). events에서 호출.
    pub(crate) fn record_cmd_history(&mut self, pane: nabi_types::PaneId, exit: i32) {
        let Some(cmd) = self.run_cmd.get(&pane).cloned() else { return };
        let cwd = self.cwds.get(&pane).map(|c| crate::workspace::strip_uri_slash(c)).unwrap_or_default();
        let ts = chrono::Local::now().timestamp();
        // **저장 직전에** 가린다. 화면에서만 가리면 설정 파일에는 그대로 남는다 —
        // 그 파일은 백업·동기화·지원 문의로 밖에 나가기 쉽다.
        let cmd = match self.config.terminal.redact_history {
            true => crate::redact::line_full(&cmd),
            false => cmd,
        };
        record(&mut self.config.terminal.cmd_history, &cmd, &cwd, exit, ts, 500);
        // 얼마나 걸렸는지도 남긴다("아까 그 빌드 얼마나 걸렸지"는 매일 나오는 질문이다).
        if let Some(start) = self.cmd_started.remove(&pane) {
            record_secs(&mut self.config.terminal.cmd_secs, ts, start.elapsed().as_secs() as u32, 500);
        }
        if self.dir_save_at.elapsed().as_secs() >= 20 {
            self.save_config();
            self.dir_save_at = std::time::Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_and_cap() {
        let mut h = CmdHist::new();
        record(&mut h, "ls", "/a", 0, 1, 100);
        record(&mut h, "cargo build", "/a", 0, 2, 100);
        record(&mut h, "ls", "/a", 0, 3, 100); // 같은 cwd 동일 명령 → 최신만(중복 제거)
        assert_eq!(h.len(), 2);
        assert_eq!(h.last().unwrap().0, "ls"); // 최신으로 이동
        record(&mut h, "ls", "/b", 0, 4, 100); // 다른 cwd → 별도 항목
        assert_eq!(h.len(), 3);
        record(&mut h, "  ", "/a", 0, 5, 100); // 빈 명령 무시
        assert_eq!(h.len(), 3);
        // 최근순.
        assert_eq!(recent(&h, 1), vec![("ls".to_string(), "/b".to_string(), 0)]);
        // cwd 필터(F3): /a에서 실행된 명령만.
        assert_eq!(recent_in_cwd(&h, "/a", 9), vec!["ls".to_string(), "cargo build".to_string()]);
        assert_eq!(recent_in_cwd(&h, "/b", 9), vec!["ls".to_string()]);
        assert_eq!(recent_in_cwd(&h, "", 9).len(), 3); // 빈 cwd=전부.
    }

    #[test]
    fn caps_oldest() {
        let mut h = CmdHist::new();
        for i in 0..5 {
            record(&mut h, &format!("cmd{i}"), "/x", 0, i, 3);
        }
        assert_eq!(h.len(), 3);
        assert_eq!(h.first().unwrap().0, "cmd2"); // 오래된 cmd0,cmd1 제거
    }
}

/// 명령 **소요 시간** — (끝난 시각 unix초, 걸린 초).
///
/// ## 왜 `cmd_history` 튜플에 넣지 않았는가
///
/// 설정 로드는 `.extract().unwrap_or_default()`다 — **한 필드라도 파싱에 실패하면 설정
/// 전체가 기본값으로 초기화된다.** `cmd_history`의 4-튜플을 5-튜플로 바꾸면 옛 config.toml은
/// 4칸 배열을 5-튜플에 넣지 못하고, 그 순간 사용자의 모든 설정이 사라진다. 소요 시간
/// 하나 보자고 치를 값이 아니다.
///
/// 반면 **새 필드를 더하는 것은 안전하다** — 옛 파일에 없으면 `#[serde(default)]`가 빈 값을
/// 준다. 그래서 곁에 따로 둔다. 완료 시각으로 맞춰 본다.
pub(crate) type CmdSecs = Vec<(i64, u32)>;

/// 소요 시간을 적는다. 같은 시각이 이미 있으면 갈아 끼운다.
pub(crate) fn record_secs(v: &mut CmdSecs, ts: i64, secs: u32, cap: usize) {
    v.retain(|(t, _)| *t != ts);
    v.push((ts, secs));
    let n = v.len();
    if n > cap {
        v.drain(0..n - cap);
    }
}

/// 그 시각에 끝난 명령이 얼마나 걸렸는가.
pub(crate) fn secs_for(v: &[(i64, u32)], ts: i64) -> Option<u32> {
    v.iter().rev().find(|(t, _)| *t == ts).map(|(_, s)| *s)
}

/// 사람이 읽는 소요 시간. 초 단위는 `12s`, 분이 넘으면 `3m 07s`, 시간이 넘으면 `1h 04m`.
pub(crate) fn human_secs(s: u32) -> String {
    // 모양은 `statusfmt::human_secs` 한 곳에만 있다. 예전에는 같은 일을 하는 함수가 다섯
    // 개였고 답이 서로 달랐다 — 3661초가 화면마다 1h01m · 1h 01m · 1h 1m 로 보였다.
    crate::statusfmt::human_secs(s as u64)
}

#[cfg(test)]
mod secs_tests {
    use super::*;

    #[test]
    fn a_duration_can_be_looked_up_by_its_finish_time() {
        let mut v = CmdSecs::new();
        record_secs(&mut v, 100, 7, 10);
        record_secs(&mut v, 200, 90, 10);
        assert_eq!(secs_for(&v, 100), Some(7));
        assert_eq!(secs_for(&v, 200), Some(90));
        assert_eq!(secs_for(&v, 300), None);
    }

    /// 같은 시각이 다시 오면 덮어쓴다 — 두 값이 쌓이면 어느 쪽이 맞는지 알 수 없다.
    #[test]
    fn the_same_finish_time_is_replaced_not_duplicated() {
        let mut v = CmdSecs::new();
        record_secs(&mut v, 100, 7, 10);
        record_secs(&mut v, 100, 9, 10);
        assert_eq!(v.len(), 1);
        assert_eq!(secs_for(&v, 100), Some(9));
    }

    /// 무한히 자라면 설정 파일이 부푼다.
    #[test]
    fn the_table_stays_bounded_dropping_the_oldest() {
        let mut v = CmdSecs::new();
        for i in 0..20 {
            record_secs(&mut v, i, i as u32, 5);
        }
        assert_eq!(v.len(), 5);
        assert_eq!(secs_for(&v, 0), None, "가장 오래된 것이 남았다");
        assert_eq!(secs_for(&v, 19), Some(19));
    }

    #[test]
    fn durations_read_the_way_people_say_them() {
        assert_eq!(human_secs(0), "0s");
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(60), "1m 00s");
        assert_eq!(human_secs(187), "3m 07s");
        assert_eq!(human_secs(3600), "1h 00m");
        assert_eq!(human_secs(3840), "1h 04m");
    }
}
