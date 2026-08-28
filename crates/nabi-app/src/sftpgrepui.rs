//! 원격 **내용 찾기**의 흐름 — 트리에서 후보를 뽑고, 앞부분을 하나씩 받아 훑는다(배치 AD).
//!
//! 창은 이름 찾기(`sftpfindui`)와 **같은 창**을 쓴다. 찾기가 두 군데면 사용자는 매번
//! "어느 찾기였더라"를 먼저 떠올려야 한다.
//!
//! ## 왜 한 번에 한 파일인가
//!
//! 400개를 동시에 요청하면 회선과 서버를 한꺼번에 물고, 취소도 어려워진다. 앞부분만 읽으므로
//! 한 번의 왕복이 짧고, 순서대로 받으면 **창을 닫는 순간 바로 멈출 수 있다.**
//!
//! ## 후보를 어떻게 줄이는가
//!
//! 트리가 돌려준 모든 파일을 다 받지는 않는다. 크기가 0인 것은 볼 것이 없고, 상한
//! (`MAX_FILES`)을 넘으면 거기서 멈춘다 — 그리고 **멈췄다는 사실을 결과에 적는다.**

use crate::app::NabiApp;
use crate::sftpgrep::{self, FileHits};
use nabi_proto::Command;

/// 내용 찾기의 진행 상태. 창을 닫으면 통째로 버린다.
#[derive(Default, Clone)]
pub(crate) struct GrepRun {
    /// 아직 받아 보지 않은 후보(상대경로).
    pub queue: Vec<String>,
    pub hits: Vec<FileHits>,
    /// 지금 회신을 기다리는 파일(원격 절대경로) — 남의 미리보기 회신을 먹지 않기 위해.
    pub waiting: Option<String>,
    pub scanned: usize,
    /// 후보가 상한에 걸려 잘렸다.
    pub stopped: bool,
}

impl NabiApp {
    /// 트리 회신을 내용 찾기의 후보 목록으로 바꾼다.
    ///
    /// 이름 찾기와 같은 `..` 탈출 방어를 쓴다 — 원격이 준 상대경로는 믿지 않는다.
    pub(crate) fn grep_start_from_tree(&mut self, files: Vec<(String, u64, u64)>) {
        let mut queue: Vec<String> = files
            .into_iter()
            .filter(|(r, size, _)| *size > 0 && crate::syncplan::safe_rel(r))
            .map(|(r, _, _)| r)
            .collect();
        let stopped = queue.len() > sftpgrep::MAX_FILES;
        queue.truncate(sftpgrep::MAX_FILES);
        queue.reverse(); // pop 이 앞에서부터 꺼내도록 — 사용자가 보는 순서와 맞춘다.
        self.sftp_grep = Some(GrepRun { queue, stopped, ..Default::default() });
        self.grep_next();
    }

    /// 다음 후보 하나의 앞부분을 요청한다. 없으면 결과를 연다.
    pub(crate) fn grep_next(&mut self) {
        let Some(id) = self.sftp.id else { return };
        let root = self.sftp_find.as_ref().map(|f| f.root.clone()).unwrap_or_default();
        let Some(g) = self.sftp_grep.as_mut() else { return };
        let Some(rel) = g.queue.pop() else {
            self.grep_finish();
            return;
        };
        // 경로 결합은  하나만 쓴다. 처음엔 여기서 손으로 이었는데,
        // 그것이 바로 이 배치가 고친 결함(같은 일을 두 곳에서 각각 하기)과 같은 잘못이었다.
        let path = crate::sftppath::join_path(&root, &rel);
        g.waiting = Some(path.clone());
        self.orch.send(Command::SftpPreview { id, path, max: sftpgrep::READ_CAP });
    }

    /// 미리보기 회신 하나를 내용 찾기가 먹었는가.
    ///
    /// 미리보기 창도 같은 명령을 쓰므로 **기다리던 경로와 같을 때만** 가져간다. 아니면
    /// `false`를 돌려주어 원래 주인이 처리하게 둔다.
    pub(crate) fn grep_on_preview(&mut self, path: &str, data: &[u8], more: bool) -> bool {
        let root = self.sftp_find.as_ref().map(|f| f.root.clone()).unwrap_or_default();
        let pat = self.sftp_find.as_ref().map(|f| f.query.clone()).unwrap_or_default();
        let Some(g) = self.sftp_grep.as_mut() else { return false };
        if g.waiting.as_deref() != Some(path) {
            return false;
        }
        g.waiting = None;
        g.scanned += 1;
        let rel = path.strip_prefix(&root).unwrap_or(path).trim_start_matches('/').to_string();
        let ci = sftpgrep::smart_case(&pat);
        if let Some(h) = sftpgrep::scan_file(&rel, data, &pat, ci, more) {
            g.hits.push(h);
        }
        let done = g.hits.iter().map(|f| f.lines.len()).sum::<usize>() >= sftpgrep::MAX_HITS;
        if done {
            g.queue.clear();
            g.stopped = true;
        }
        self.grep_next();
        true
    }

    /// 다 훑었다 — 결과를 nabiPad 문서로 연다(로컬 내용 찾기와 같은 방식).
    fn grep_finish(&mut self) {
        let Some(g) = self.sftp_grep.take() else { return };
        let pat = self.sftp_find.as_ref().map(|f| f.query.clone()).unwrap_or_default();
        let (body, total) = sftpgrep::report(&g.hits, &pat, g.stopped, g.scanned);
        let mut doc = crate::editor::EditorDoc::make(
            format!("\u{1f50d} {pat}"),
            std::path::PathBuf::new(),
            None,
            body,
            true,
            self.font_size,
            "UTF-8".into(),
            "\n",
        );
        doc.dirty = true;
        self.add_editor_tab(doc);
        self.notify = Some((
            format!("{} {total}", nabi_i18n::tr(self.lang, "sftp.grep.done")),
            std::time::Instant::now(),
        ));
    }
}
