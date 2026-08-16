//! 터미널 출력의 `파일:줄[:열]` 패턴 감지(컴파일러/테스트 에러 → 에디터로 점프). 순수 함수 + 테스트.
//! Windows 드라이브 콜론(C:\…)을 고려해 "끝에서부터" 줄/열 숫자를 떼어낸다.

use crate::app::NabiApp;
use std::path::{Path, PathBuf};

impl NabiApp {
    /// pending_pathline(터미널 더블클릭 결과)을 에디터로 연다(상대경로는 포커스 pane cwd 기준).
    pub(crate) fn open_terminal_pathline(&mut self) {
        let Some((path, line)) = self.pending_pathline.take() else { return };
        let mut pb = PathBuf::from(&path);
        if pb.is_relative() {
            if let Some(cwd) = self.focused_pane().and_then(|p| self.cwds.get(&p)) {
                pb = Path::new(&crate::workspace::strip_uri_slash(cwd)).join(&path);
            }
        }
        if pb.is_file() {
            self.open_editor_local(pb.clone());
            if let Some((_, d)) = self.editors.iter_mut().find(|(_, d)| d.path == pb) {
                d.find.scroll_to = Some(line); // find.scroll_to 재사용으로 그 줄 점프.
            }
        }
    }

    /// 명령 문자열을 (에디터/브라우저/SFTP 아닌) 첫 터미널 pane에 전송+Enter하고 그 탭을 활성화(에디터 ▸ 터미널에서 실행).
    /// 탭 순서(dock)대로 첫 터미널을 고른다 — HashMap 무작위 순서보다 예측 가능(왼쪽 우선).
    pub(crate) fn run_in_first_terminal(&mut self, cmd: String) {
        let term = self.dock.iter_all_tabs().map(|(_, p)| *p).find(|p| !self.editors.contains_key(p) && !self.browser_tabs.contains_key(p) && Some(*p) != self.sftp_pane && !self.sftp_bg.contains_key(p));
        if let Some(p) = term {
            let mut data = cmd.into_bytes();
            data.push(b'\r');
            self.orch.send(nabi_proto::Command::WriteInput { pane: p, data: bytes::Bytes::from(data) });
            if let Some(loc) = self.dock.find_tab(&p) {
                let _ = self.dock.set_active_tab(loc);
            }
        }
    }
}

/// 끝의 `:<숫자>`를 떼어 (앞부분, 숫자)를 돌려준다. 없으면 None.
fn split_trailing_num(s: &str) -> Option<(&str, usize)> {
    let i = s.rfind(':')?;
    let n = s[i + 1..].parse::<usize>().ok()?;
    Some((&s[..i], n))
}

/// 경로처럼 보이는가 — 경로 구분자(/ \)가 있거나, 마지막 확장자가 영문 포함 1~10 영숫자.
/// (IP "8.8.8.8" 같은 숫자만 확장자는 제외해 오검출을 줄인다.)
fn looks_like_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.contains('/') || s.contains('\\') {
        return true;
    }
    match s.rsplit_once('.') {
        Some((head, ext)) if !head.is_empty() => {
            (1..=10).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        }
        _ => false,
    }
}

/// 토큰이 `파일:줄` 또는 `파일:줄:열`이면 (경로, 0기반 줄)을 돌려준다. 아니면 None.
pub(crate) fn parse_path_line(token: &str) -> Option<(String, usize)> {
    let t = token
        .trim()
        .trim_start_matches(['(', '[', '{', '"', '\''])
        .trim_end_matches([')', ']', '}', ',', ';', '"', '\'']);
    let (head1, n1) = split_trailing_num(t)?; // 끝 숫자(줄 또는 열).
    // 앞이 또 `:<숫자>`고 그 앞이 경로면 head2:line:col 형태(col 무시).
    let (path, line1) = match split_trailing_num(head1) {
        Some((head2, n2)) if looks_like_path(head2) => (head2, n2),
        _ => (head1, n1),
    };
    if !looks_like_path(path) {
        return None;
    }
    Some((path.to_string(), line1.saturating_sub(1))) // 표시 1기반 → 내부 0기반.
}

#[cfg(test)]
mod tests {
    use super::parse_path_line;

    #[test]
    fn detects_common_forms() {
        assert_eq!(parse_path_line("src/foo.rs:42"), Some(("src/foo.rs".into(), 41)));
        assert_eq!(parse_path_line("a.py:10:5"), Some(("a.py".into(), 9))); // 열은 무시.
        assert_eq!(parse_path_line("C:\\dir\\main.rs:7"), Some(("C:\\dir\\main.rs".into(), 6))); // 드라이브 콜론.
        assert_eq!(parse_path_line("(./x.txt:3)"), Some(("./x.txt".into(), 2))); // 둘러싼 괄호 제거.
    }

    #[test]
    fn rejects_non_paths() {
        assert_eq!(parse_path_line("word:42"), None); // 확장자/구분자 없음.
        assert_eq!(parse_path_line("8.8.8.8:80"), None); // 숫자 확장자(IP)는 제외.
        assert_eq!(parse_path_line("foo.rs"), None); // 줄 번호 없음.
        assert_eq!(parse_path_line("https://a.com"), None); // URL(콜론 뒤 비숫자).
    }
}
