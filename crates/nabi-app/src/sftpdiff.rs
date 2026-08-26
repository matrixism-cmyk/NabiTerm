//! **원격 파일 둘을 견준다** — 받아서 열고 비교하는 일을 사람이 하지 않게.
//!
//! 로컬 파일 비교는 이미 있다(`difflines::compare_selected`). 원격은 지금까지 **두 번
//! 내려받아 두 번 열어** 눈으로 봐야 했다. 서버에 diff가 있어도 두 파일이 서로 다른
//! 폴더에 있으면 명령을 짓는 것부터 일이다.
//!
//! ## 새 비교를 만들지 않는다
//!
//! 받아 온 뒤에는 **로컬 비교와 같은 길**로 들어간다(`compare_paths`). 이진이면 바이트
//! 비교, 글이면 줄 비교 — 그 판단도 한 곳에만 있다. 두 곳이 되면 "로컬에선 되는데
//! 원격에선 다르게 보이는" 일이 생긴다.
//!
//! ## 임시 파일
//!
//! 이름이 같은 두 파일(다른 폴더의 같은 이름)을 견주는 일이 흔하다. 그래서 임시 이름에
//! **a/b 표시를 넣는다** — 안 그러면 하나가 다른 하나를 덮어써 자기 자신과 비교하게 된다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_proto::Command;
use std::path::PathBuf;
use std::time::Instant;

/// 내려받기를 기다리는 비교 한 건.
pub(crate) struct PendingDiff {
    pub name_a: String,
    pub name_b: String,
    pub temp_a: PathBuf,
    pub temp_b: PathBuf,
    /// 아직 도착하지 않은 임시 경로들. 비면 비교를 연다.
    pub waiting: Vec<String>,
}

/// 임시 파일 이름을 짓는다. `slot`은 a/b — 같은 이름의 두 파일이 서로를 덮지 않게 한다.
///
/// 원격 이름에는 경로 구분자나 `..`가 들어올 수 있다. 그대로 이어 붙이면 임시 폴더 밖을
/// 가리키게 되므로 **글자를 걸러서** 쓴다.
pub(crate) fn temp_name(slot: char, remote_name: &str) -> String {
    let safe: String = remote_name
        .chars()
        .map(|c| match c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
            true => c,
            false => '_',
        })
        .collect();
    // 빗금이 사라졌으니 `..`만으로는 밖을 못 가리키지만, 남겨 둘 까닭도 없다.
    // 경로처럼 생긴 조각은 아예 지운다 — 나중에 이 이름을 다른 데 쓰게 될 때를 위해서다.
    let safe = safe.replace("..", "_");
    // 아주 긴 이름은 자른다(윈도우 경로 길이 제한). 앞쪽이 사람에게 더 뜻이 있다.
    let cut: String = safe.chars().take(60).collect();
    format!("nabi-diff-{slot}-{cut}")
}

impl NabiApp {
    /// 지금 고른 원격 파일 둘을 내려받아 비교한다.
    pub(crate) fn compare_remote_selected(&mut self) {
        let Some(id) = self.sftp.id else { return };
        let mut sel: Vec<String> = self.sftp.multi.iter().cloned().collect();
        sel.sort();
        if sel.len() != 2 {
            self.notify = Some((tr(self.lang, "diff.need2").to_string(), Instant::now()));
            return;
        }
        // 폴더는 줄로도 바이트로도 견줄 수 없다 — 폴더 비교는 이미 동기화 쪽에 있다.
        if sel.iter().any(|n| self.sftp.entries.iter().any(|e| e.name == *n && e.is_dir)) {
            self.notify = Some((tr(self.lang, "diff.nodirs").to_string(), Instant::now()));
            return;
        }
        let dir = std::env::temp_dir();
        let (ta, tb) = (dir.join(temp_name('a', &sel[0])), dir.join(temp_name('b', &sel[1])));
        let (la, lb) = (ta.to_string_lossy().into_owned(), tb.to_string_lossy().into_owned());
        for (name, local) in [(&sel[0], &la), (&sel[1], &lb)] {
            self.orch.send(Command::SftpDownload {
                id,
                xfer: crate::sftpxfer::XFER_NONE,
                remote: crate::sftppath::join_path(&self.sftp.path, name),
                local: local.clone(),
                resume: 0, // 비교는 늘 새로 받는다(반쯤 남은 옛 파일과 견주면 안 된다).
            });
        }
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        self.pending_diff = Some(PendingDiff {
            name_a: sel[0].clone(),
            name_b: sel[1].clone(),
            temp_a: ta,
            temp_b: tb,
            waiting: vec![la, lb],
        });
    }

    /// 내려받기 하나가 끝났다. 둘 다 왔으면 비교를 연다.
    pub(crate) fn on_diff_download(&mut self, local: &str) {
        let ready = match self.pending_diff.as_mut() {
            Some(p) => {
                p.waiting.retain(|w| !w.eq_ignore_ascii_case(local));
                p.waiting.is_empty()
            }
            None => false,
        };
        if !ready {
            return;
        }
        let Some(p) = self.pending_diff.take() else { return };
        self.compare_paths(&p.name_a, &p.temp_a, &p.name_b, &p.temp_b);
    }
}

#[cfg(test)]
mod tests {
    use super::temp_name;

    #[test]
    fn a_plain_name_survives() {
        assert_eq!(temp_name('a', "report.txt"), "nabi-diff-a-report.txt");
    }

    /// **같은 이름의 두 파일이 서로를 덮으면 안 된다** — 자기 자신과 비교하게 된다.
    #[test]
    fn the_two_slots_never_collide() {
        assert_ne!(temp_name('a', "app.conf"), temp_name('b', "app.conf"));
    }

    /// 이름에 든 경로 조각이 임시 폴더 밖을 가리키면 안 된다.
    #[test]
    fn a_name_cannot_escape_the_temp_folder() {
        for evil in ["../../etc/passwd", "a/b/c.txt", "..\\..\\win.ini"] {
            let t = temp_name('a', evil);
            assert!(!t.contains('/') && !t.contains('\\'), "{t}");
            assert!(!t.contains(".."), "{t}");
        }
    }

    /// 아주 긴 이름은 잘린다(윈도우 경로 길이 제한).
    #[test]
    fn a_very_long_name_is_cut() {
        let long = "x".repeat(300);
        assert!(temp_name('a', &long).len() < 100);
    }

    /// 한글 이름은 살린다 — 걸러야 하는 것은 경로 글자이지 우리말이 아니다.
    #[test]
    fn hangul_names_are_kept() {
        let t = temp_name('a', "보고서.txt");
        assert!(t.contains("보고서"), "{t}");
    }
}
