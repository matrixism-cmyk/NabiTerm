//! 터미널 설정을 **읽는 자리** — 저장된 값과 실제로 쓰는 값 사이의 규칙을 한곳에 둔다.
//!
//! 설정 파일의 값은 사람이 손으로 고칠 수 있으므로 범위를 벗어난 값이 들어올 수 있다.
//! 그 값을 다듬는 일을 쓰는 쪽마다 하면 어긋난다 — 실제로 동시 전송 수는 설정 화면의
//! 슬라이더(1~4)·큐의 클램프(1~4)·기본값(2) 세 곳에 따로 적혀 있었고, 그중 하나만
//! 고치면 나머지가 조용히 어긋나는 상태였다. 읽는 길을 하나로 만든다.

use crate::schema::{TerminalCfg, MAX_PARALLEL_TRANSFERS};

impl TerminalCfg {
    /// 실제로 동시에 돌릴 전송 수. 설정값이 어떻든 1..=[`MAX_PARALLEL_TRANSFERS`] 안이다.
    pub fn parallel_transfers(&self) -> usize {
        self.max_parallel_transfers.clamp(1, MAX_PARALLEL_TRANSFERS) as usize
    }

    /// 이 명령이 "기록을 자기 오버레이에만 두는 TUI"인가 — 휠을 페이지 키로 바꿔 보낼 대상.
    pub fn is_wheel_key_app(&self, cmd: &str) -> bool {
        is_wheel_key_app_in(&self.wheel_key_apps, cmd)
    }
}

/// 목록에 든 이름과 명령이 같은가.
///
/// 설정 전체가 아니라 **목록만** 받는 까닭: 탭 뷰어처럼 설정을 통째로 들고 있지 않은 곳도
/// 같은 규칙을 써야 한다. 규칙이 두 벌이 되면 탭과 분리 창이 다르게 굴기 시작한다.
///
/// 명령줄 전체가 아니라 **첫 낱말의 파일 이름**으로 본다. 경로와 확장자를 떼어 내야
/// `npm\codex.cmd --model x` 도 `codex` 로 읽힌다.
pub fn is_wheel_key_app_in(apps: &[String], cmd: &str) -> bool {
    let Some(base) = command_base(cmd) else {
        return false;
    };
    apps.iter().any(|a| a.trim().eq_ignore_ascii_case(&base))
}

/// 명령줄에서 실행 파일의 **이름만** 뽑는다(경로·확장자 제거, 소문자). 비었으면 None.
pub fn command_base(cmd: &str) -> Option<String> {
    let first = cmd.split_whitespace().next()?;
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first).to_ascii_lowercase();
    let base = base
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat");
    match base.is_empty() {
        true => None,
        false => Some(base.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 설정값이 범위를 벗어나도 쓰는 쪽은 늘 안전한 값을 받는다.
    #[test]
    fn 전송_수는_늘_범위_안이다() {
        let mut c = TerminalCfg::default();
        for (set, want) in [(0, 1), (1, 1), (4, 4), (10, 10), (999, 10)] {
            c.max_parallel_transfers = set;
            assert_eq!(c.parallel_transfers(), want, "설정 {set}");
        }
    }

    /// 기본값은 상한 안에 있어야 한다 — 기본이 상한을 넘으면 아무도 못 알아챈다.
    #[test]
    fn 기본값이_상한을_넘지_않는다() {
        let c = TerminalCfg::default();
        assert!(c.max_parallel_transfers <= MAX_PARALLEL_TRANSFERS);
        assert!(c.max_parallel_transfers >= 1);
    }

    /// 경로와 확장자를 떼어 내고 이름만 본다.
    #[test]
    fn 이름만_보고_고른다() {
        let c = TerminalCfg::default(); // 기본 목록 = ["codex"]
        assert!(c.is_wheel_key_app("codex"));
        assert!(c.is_wheel_key_app("codex resume --last"));
        assert!(c.is_wheel_key_app(r"C:\Users\u\AppData\Roaming\npm\codex.cmd --model x"));
        assert!(c.is_wheel_key_app("/usr/local/bin/codex"));
        // 클로드 코드는 스크롤백을 제대로 남긴다 — 휠은 그것을 봐야 한다.
        assert!(!c.is_wheel_key_app("claude --continue"));
        assert!(!c.is_wheel_key_app("cargo build"));
        // 이름이 다른 것을 앞에서부터 닮았다고 걸리면 안 된다.
        assert!(!c.is_wheel_key_app("codex-helper run"));
        assert!(!c.is_wheel_key_app(""));
    }

    /// 목록을 늘리면 그대로 늘어난다 — 코드에 박아 두지 않는 까닭이다.
    #[test]
    fn 목록을_늘리면_늘어난다() {
        let mut c = TerminalCfg::default();
        c.wheel_key_apps.push("  MyTui ".into()); // 앞뒤 공백·대소문자 무관.
        assert!(c.is_wheel_key_app(r"D:\tools\mytui.exe --flag"));
        assert!(!c.is_wheel_key_app("othertui"));
    }
}

/// 목록을 한 줄 글로(설정 화면의 입력칸에 보여 줄 모양).
pub fn apps_to_text(apps: &[String]) -> String {
    apps.join(", ")
}

/// 한 줄 글을 목록으로. 빈 토막과 앞뒤 공백은 버린다.
///
/// 사람이 치는 칸이므로 `codex,  , mytui ,` 같은 모양이 온다. 빈 이름을 그대로 담으면
/// 모든 명령이 그 빈 이름과 같다고 판정될 위험이 있어 반드시 걸러야 한다.
pub fn text_to_apps(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod textlist_tests {
    use super::*;

    /// 오갔다 와도 같아야 한다.
    #[test]
    fn 글과_목록을_오간다() {
        let apps = vec!["codex".to_string(), "mytui".to_string()];
        assert_eq!(apps_to_text(&apps), "codex, mytui");
        assert_eq!(text_to_apps("codex, mytui"), apps);
    }

    /// 빈 토막은 버린다 — 빈 이름이 들어가면 아무 명령이나 걸린다.
    #[test]
    fn 빈_토막은_버린다() {
        assert_eq!(text_to_apps("codex,  , mytui ,"), vec!["codex", "mytui"]);
        assert!(text_to_apps("   ").is_empty());
        assert!(text_to_apps(",,,").is_empty());
        // 그리고 실제로 아무것도 걸리지 않아야 한다.
        let mut c = crate::schema::TerminalCfg { wheel_key_apps: text_to_apps(",,"), ..Default::default() };
        assert!(!c.is_wheel_key_app("anything"));
        c.wheel_key_apps = text_to_apps("codex");
        assert!(c.is_wheel_key_app("codex"));
    }
}
