//! nabiPad **미저장 문서 보호** — 강제 종료·정전에도 새 문서를 잃지 않게.
//!
//! ## 왜 자동 저장으로는 부족한가
//!
//! 기존 자동 저장(`extwatch::autosave_tick`)은 **경로가 있는 파일만** 그 자리에 다시 쓴다.
//! 아직 저장한 적 없는 문서(제목 없음)는 쓸 곳이 없어 그냥 건너뛴다. 그래서 새 문서를
//! 한참 쓰다가 강제 종료되면 **통째로 사라진다.** 게다가 자동 저장은 기본이 꺼져 있다.
//!
//! ## 어떻게 하는가
//!
//! 설정 폴더 밑 `recover/`에 미저장 문서를 주기적으로 떨궈 둔다. 정상 종료할 때는 지운다.
//! 다음 실행에서 그 폴더에 뭐가 남아 있으면 **비정상 종료였다는 뜻**이므로 되살릴지 묻는다.
//! (워드·VS Code가 하는 것과 같은 방식이다. 우리 워크스페이스 복원은 터미널·SFTP 탭만
//! 다루고 편집 문서는 아예 담지 않는다 — 감사 2026-08-25.)
//!
//! 원본 파일에는 절대 쓰지 않는다. 복구본은 어디까지나 사본이고, 되살릴지는 사용자가 정한다.

use std::path::{Path, PathBuf};

/// 복구본 하나 — 파일 이름과 내용.
pub(crate) struct Recovered {
    pub name: String,
    pub text: String,
}

/// 복구본을 두는 폴더.
pub(crate) fn dir(cfg_dir: &Path) -> PathBuf {
    cfg_dir.join("recover")
}

/// 문서 하나를 복구 폴더에 떨군다. `key`는 pane별로 고유해야 한다(같은 문서를 덮어쓰게).
///
/// 첫 줄에 원래 제목을 적어 두고 그 다음부터 본문이다 — 제목 없는 문서를 되살릴 때
/// "무제-3" 같은 이름이라도 돌려주려면 어딘가에 적어 둬야 한다.
pub(crate) fn stash(cfg_dir: &Path, key: u64, title: &str, text: &str) -> std::io::Result<()> {
    let d = dir(cfg_dir);
    std::fs::create_dir_all(&d)?;
    let body = format!("{}\n{text}", title.replace('\n', " "));
    std::fs::write(d.join(format!("{key}.nabipad")), body)
}

/// 그 문서의 복구본을 지운다(저장했거나 닫았을 때).
pub(crate) fn drop_one(cfg_dir: &Path, key: u64) {
    let _ = std::fs::remove_file(dir(cfg_dir).join(format!("{key}.nabipad")));
}

/// 복구 폴더를 통째로 비운다(정상 종료).
pub(crate) fn clear(cfg_dir: &Path) {
    let _ = std::fs::remove_dir_all(dir(cfg_dir));
}

/// 남아 있는 복구본을 모두 읽는다(시작 시 1회). 비었으면 정상 종료였다는 뜻이다.
pub(crate) fn take_all(cfg_dir: &Path) -> Vec<Recovered> {
    let d = dir(cfg_dir);
    let Ok(rd) = std::fs::read_dir(&d) else { return Vec::new() };
    let mut out = Vec::new();
    for f in rd.flatten() {
        if f.path().extension().and_then(|e| e.to_str()) != Some("nabipad") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(f.path()) else { continue };
        out.push(split(&raw));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 저장해 둔 덩어리를 제목과 본문으로 가른다.
fn split(raw: &str) -> Recovered {
    match raw.split_once('\n') {
        Some((title, body)) => Recovered { name: title.to_string(), text: body.to_string() },
        None => Recovered { name: String::new(), text: raw.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nabi-recover-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn a_stashed_document_comes_back_with_its_title() {
        let c = tmp("basic");
        stash(&c, 7, "무제-3", "안녕\n둘째 줄").unwrap();
        let got = take_all(&c);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "무제-3");
        assert_eq!(got[0].text, "안녕\n둘째 줄");
        let _ = std::fs::remove_dir_all(&c);
    }

    /// 정상 종료면 남는 것이 없어야 한다 — 남아 있음 자체가 "비정상이었다"는 신호다.
    #[test]
    fn a_clean_shutdown_leaves_nothing_to_recover() {
        let c = tmp("clean");
        stash(&c, 1, "a", "x").unwrap();
        clear(&c);
        assert!(take_all(&c).is_empty());
    }

    #[test]
    fn saving_a_document_removes_only_its_own_stash() {
        let c = tmp("one");
        stash(&c, 1, "a", "1").unwrap();
        stash(&c, 2, "b", "2").unwrap();
        drop_one(&c, 1);
        let got = take_all(&c);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "b");
        let _ = std::fs::remove_dir_all(&c);
    }

    /// 같은 문서를 여러 번 떨궈도 하나만 남는다(덮어쓰기).
    #[test]
    fn restashing_the_same_document_overwrites_it() {
        let c = tmp("over");
        stash(&c, 5, "t", "처음").unwrap();
        stash(&c, 5, "t", "나중").unwrap();
        let got = take_all(&c);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "나중");
        let _ = std::fs::remove_dir_all(&c);
    }

    /// 제목에 개행이 섞여 들어와도 본문 경계가 밀리지 않는다.
    #[test]
    fn a_newline_in_the_title_cannot_shift_the_body() {
        let c = tmp("nl");
        stash(&c, 9, "제목\n가짜", "진짜 본문").unwrap();
        let got = take_all(&c);
        assert_eq!(got[0].name, "제목 가짜");
        assert_eq!(got[0].text, "진짜 본문");
        let _ = std::fs::remove_dir_all(&c);
    }

    /// 복구 폴더가 아예 없으면(첫 실행) 조용히 빈 목록.
    #[test]
    fn a_missing_folder_is_not_an_error() {
        assert!(take_all(&tmp("none")).is_empty());
    }
}
