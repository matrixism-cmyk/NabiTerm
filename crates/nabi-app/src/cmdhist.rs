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
        record(&mut self.config.terminal.cmd_history, &cmd, &cwd, exit, chrono::Local::now().timestamp(), 500);
        if self.dir_save_at.elapsed().as_secs() >= 20 {
            let _ = nabi_config::save(&self.config_path, &self.config);
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
