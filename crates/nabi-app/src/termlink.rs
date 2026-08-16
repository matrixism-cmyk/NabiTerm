//! 터미널 링크 Ctrl+클릭 처리 — 파일 참조(`경로:줄[:열]`)는 nabiPad에서 해당 줄로,
//! 그 외(경로·URL)는 OS 기본 앱으로 연다. 탐지는 nabi-render(fileref), cwd 해석·열기는 여기.

use crate::app::NabiApp;
use nabi_types::PaneId;
use std::path::{Path, PathBuf};

impl NabiApp {
    /// 터미널 링크를 연다: 로컬 파일 참조면 nabiPad(해당 줄), 아니면 OS 기본 앱(폴백).
    pub(crate) fn open_term_link(&mut self, pane: PaneId, url: &str) {
        // ssh://·sftp:// 링크는 Quick Connect로 — OS 기본앱이 아닌 nabiTerm에서 연다.
        if let Some(rest) = url.strip_prefix("ssh://") {
            self.connect_ssh_url(rest);
            return;
        }
        if let Some(rest) = url.strip_prefix("sftp://") {
            self.connect_sftp_url(rest);
            return;
        }
        // 그 외 scheme URL(http://, file:// 등)은 그대로 OS로 — 포트(`:8080`)를 줄로 오인하지 않게.
        if url.contains("://") {
            crate::paneurl::os_open(url);
            return;
        }
        if self.open_file_ref(pane, url) {
            return;
        }
        // 파일로 못 열면: 줄 접미(`:42`)가 있으면 떼고 OS로 연다(잘못된 경로로 실패 방지).
        let (path, line, _) = nabi_render::parse_file_ref(url);
        crate::paneurl::os_open(if line.is_some() { &path } else { url });
    }

    /// `경로:줄[:열]` 파일 참조면 로컬 경로를 해석해 nabiPad에서 연다. 처리했으면 true.
    /// 상대경로는 pane cwd(OSC7) 기준. 줄 번호가 없거나 파일이 없으면 false(호출측이 OS로 폴백).
    fn open_file_ref(&mut self, pane: PaneId, url: &str) -> bool {
        let (path, line, col) = nabi_render::parse_file_ref(url);
        let Some(line) = line else { return false };
        let pb = Path::new(&path);
        let resolved: PathBuf = if pb.is_absolute() {
            pb.to_path_buf()
        } else if let Some(cwd) = self.cwds.get(&pane).map(|c| crate::workspace::strip_uri_slash(c)) {
            Path::new(&cwd).join(pb)
        } else {
            return false; // 상대경로인데 cwd를 모르면 해석 불가.
        };
        if !resolved.is_file() {
            return false;
        }
        self.open_editor_at_line(resolved, Some(line), col);
        true
    }

    /// 로컬 파일을 열고(이미 열려 있으면 포커스) 지정 줄(1-based)로 스크롤한다. col(1-based) 있으면 그 열에 커서.
    pub(crate) fn open_editor_at_line(&mut self, path: PathBuf, line: Option<u32>, col: Option<u32>) {
        self.open_editor_local(path.clone());
        if let Some(l) = line {
            let line0 = l.saturating_sub(1) as usize; // 1-based → 0-based.
            if let Some((_, d)) = self.editors.iter_mut().find(|(_, d)| d.path == path) {
                d.find.scroll_to = Some(line0);
                d.cur_line = line0;
                // 커서를 그 줄로 이동 — 열이 주어지면 그 열, 없으면 줄 시작(1열). jump_to_line·북마크와 일관.
                d.find.pending_cursor = Some(line_col_to_offset(&d.text, line0, col.unwrap_or(1)));
            }
        }
    }
}

pub(crate) use nabi_editor::textpos::line_col_to_offset;

#[cfg(test)]
mod tests {
    use super::line_col_to_offset;

    #[test]
    fn offset_from_line_col() {
        let t = "abc\nde\nfghij"; // 줄0 "abc"(0-2), 줄1 "de"(4-5), 줄2 "fghij"(7-).
        assert_eq!(line_col_to_offset(t, 0, 1), 0); // 줄0 1열 = 시작.
        assert_eq!(line_col_to_offset(t, 1, 1), 4); // 줄1 1열 = 'd'.
        assert_eq!(line_col_to_offset(t, 2, 3), 9); // 줄2 3열 = 'h'.
        assert_eq!(line_col_to_offset(t, 1, 99), 6); // 열 초과 → 그 줄 끝.
        assert_eq!(line_col_to_offset(t, 99, 1), 12); // 줄 초과 → 문서 끝.
    }
}
