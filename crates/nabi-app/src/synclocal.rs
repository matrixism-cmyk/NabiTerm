//! **로컬 폴더끼리** 동기화 실행(WinSCP 6.6 이 2026-02 에 넣은 것).
//!
//! ## 왜 필요한가
//!
//! 지금까지 동기화는 늘 "여기와 저 서버"였다. 그런데 백업 디스크·USB·네트워크 드라이브
//! 처럼 **둘 다 로컬인** 자리가 흔하다. 그때마다 사용자는 로봇카피(robocopy)를 찾아
//! 명령줄을 짜야 했다 — 우리가 계획·미리보기·안전장치를 다 갖추고도 못 쓰게 하고 있었다.
//!
//! 비교하고 계획하는 일([`crate::syncplan`])은 처음부터 **어느 쪽이 원격인지 모른다** —
//! 상대경로와 (크기, 시각)만 본다. 그래서 새로 만들 것은 계획이 아니라 **실행기**뿐이다.
//!
//! ## 안전
//!
//! 미러 모드는 대상 쪽 파일을 지운다. 남의 폴더를 지우는 일이므로 세 겹으로 막는다.
//!
//! 1. 상대경로는 [`crate::syncplan::safe_rel`] 을 지난 것만 받는다(`..`·드라이브 탈출 차단).
//! 2. 여기서 한 번 더 **뿌리 안에 있는지** 확인한다 — 검사는 두 곳에 있어야 한다.
//!    첫 겹은 부르는 쪽이 잊을 수 있고, 그때 잃는 것이 남의 파일이다.
//! 3. 지우는 것은 **파일만**이다. 폴더는 건드리지 않는다(빈 폴더가 남는 편이
//!    엉뚱한 폴더가 통째로 사라지는 것보다 낫다).

use crate::syncplan::{safe_rel, SyncAction};
use std::path::{Path, PathBuf};

/// 실행 결과 — 몇 개를 했고 몇 개가 실패했나.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Done {
    pub copied: usize,
    pub moved: usize,
    pub deleted: usize,
    /// 실패한 것. **조용히 넘기지 않는다** — 안 된 것을 말하지 않으면 "다 됐다"로 읽힌다.
    pub failed: usize,
}

impl Done {
    pub fn total(&self) -> usize {
        self.copied + self.moved + self.deleted
    }
}

/// 상대경로를 뿌리 아래 실제 경로로. 뿌리를 벗어나면 `None`.
///
/// `safe_rel` 을 지난 뒤에도 다시 본다 — 검사가 한 겹뿐이면, 부르는 쪽이 잊는 날
/// 남의 파일이 지워진다.
fn under(root: &Path, rel: &str) -> Option<PathBuf> {
    if !safe_rel(rel) {
        return None;
    }
    let p = root.join(rel.replace('/', "\\"));
    // 아직 없는 파일도 있으므로(복사 대상) 정규화 대신 접두사로 본다.
    // 여기까지 온 상대경로에는 `..` 가 없으므로 이것으로 충분하다.
    p.starts_with(root).then_some(p)
}

/// 계획을 실제로 수행한다. `src`에서 `dst`로.
///
/// 폴더는 필요할 때 만든다. 실패한 항목은 세기만 하고 **멈추지 않는다** —
/// 한 파일이 잠겨 있다고 나머지를 포기하면 사용자가 같은 일을 또 해야 한다.
pub fn run(src: &Path, dst: &Path, actions: &[SyncAction]) -> Done {
    let mut d = Done::default();
    for a in actions {
        let ok = match a {
            SyncAction::Copy(rel) | SyncAction::Update(rel) => copy_one(src, dst, rel, &mut d),
            SyncAction::Move { from, to } => move_one(dst, from, to, &mut d),
            SyncAction::Delete(rel) => delete_one(dst, rel, &mut d),
        };
        if !ok {
            d.failed += 1;
        }
    }
    d
}

fn copy_one(src: &Path, dst: &Path, rel: &str, d: &mut Done) -> bool {
    let (Some(from), Some(to)) = (under(src, rel), under(dst, rel)) else { return false };
    if let Some(parent) = to.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    match std::fs::copy(&from, &to) {
        Ok(_) => {
            d.copied += 1;
            true
        }
        Err(_) => false,
    }
}

/// 자리만 옮긴 파일 — 바이트를 다시 쓰지 않는다(폴더 이름 하나에 5GB 를 다시 쓰던 자리).
///
/// 실패해도 잃는 것은 없다: 옛 자리에 그대로 남아 있고 다음 동기화가 다시 계획한다.
fn move_one(dst: &Path, from: &str, to: &str, d: &mut Done) -> bool {
    let (Some(old), Some(new)) = (under(dst, from), under(dst, to)) else { return false };
    if let Some(parent) = new.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    match std::fs::rename(&old, &new) {
        Ok(()) => {
            d.moved += 1;
            true
        }
        Err(_) => false,
    }
}

/// **파일만** 지운다. 폴더는 건드리지 않는다.
fn delete_one(dst: &Path, rel: &str, d: &mut Done) -> bool {
    let Some(p) = under(dst, rel) else { return false };
    if p.is_dir() {
        return false;
    }
    match std::fs::remove_file(&p) {
        Ok(()) => {
            d.deleted += 1;
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용 임시 폴더. 이름에 시각을 넣어 같은 판이 겹치지 않게 한다.
    fn tmp(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!("nabi_synclocal_{tag}_{n}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel.replace('/', "\\"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn read(root: &Path, rel: &str) -> Option<String> {
        std::fs::read_to_string(root.join(rel.replace('/', "\\"))).ok()
    }

    #[test]
    fn 복사는_내용까지_옮긴다() {
        let (s, dt) = (tmp("copy_s"), tmp("copy_d"));
        write(&s, "a/b.txt", "안녕");
        let d = run(&s, &dt, &[SyncAction::Copy("a/b.txt".into())]);
        assert_eq!(d.copied, 1);
        assert_eq!(d.failed, 0);
        assert_eq!(read(&dt, "a/b.txt").as_deref(), Some("안녕"), "폴더까지 만들어야 한다");
    }

    /// 갱신은 덮어쓴다 — 안 덮으면 "동기화했다"면서 옛 내용이 남는다.
    #[test]
    fn 갱신은_덮어쓴다() {
        let (s, dt) = (tmp("upd_s"), tmp("upd_d"));
        write(&s, "x.txt", "새것");
        write(&dt, "x.txt", "헌것");
        run(&s, &dt, &[SyncAction::Update("x.txt".into())]);
        assert_eq!(read(&dt, "x.txt").as_deref(), Some("새것"));
    }

    /// 이동은 **바이트를 다시 쓰지 않는다** — 대상 쪽에서 이름만 바꾼다.
    #[test]
    fn 이동은_대상_쪽에서_이름만_바꾼다() {
        let (s, dt) = (tmp("mv_s"), tmp("mv_d"));
        write(&dt, "old/f.bin", "내용");
        let d = run(&s, &dt, &[SyncAction::Move { from: "old/f.bin".into(), to: "new/f.bin".into() }]);
        assert_eq!(d.moved, 1);
        assert_eq!(read(&dt, "new/f.bin").as_deref(), Some("내용"));
        assert!(read(&dt, "old/f.bin").is_none(), "옛 자리가 남아 있다");
    }

    /// **뿌리를 벗어나는 경로는 아무것도 하지 않는다.** 이것이 이 파일에서 가장 중요한 시험이다 —
    /// 여기가 뚫리면 미러 모드가 남의 폴더를 지운다.
    #[test]
    fn 뿌리_밖은_건드리지_않는다() {
        let (s, dt) = (tmp("esc_s"), tmp("esc_d"));
        let outside = dt.parent().unwrap().join("nabi_synclocal_바깥.txt");
        std::fs::write(&outside, "남의 파일").unwrap();
        let d = run(&s, &dt, &[
            SyncAction::Delete("../nabi_synclocal_바깥.txt".into()),
            SyncAction::Delete("C:evil.txt".into()),
            SyncAction::Copy("..\\나쁜.txt".into()),
        ]);
        assert_eq!(d.total(), 0, "뿌리 밖에서 무언가를 했다");
        assert_eq!(d.failed, 3);
        assert!(outside.exists(), "뿌리 밖 파일이 지워졌다");
        let _ = std::fs::remove_file(&outside);
    }

    /// 폴더는 지우지 않는다 — 빈 폴더가 남는 편이 폴더가 통째로 사라지는 것보다 낫다.
    #[test]
    fn 폴더는_지우지_않는다() {
        let (s, dt) = (tmp("dir_s"), tmp("dir_d"));
        std::fs::create_dir_all(dt.join("어떤폴더")).unwrap();
        let d = run(&s, &dt, &[SyncAction::Delete("어떤폴더".into())]);
        assert_eq!(d.deleted, 0);
        assert_eq!(d.failed, 1);
        assert!(dt.join("어떤폴더").is_dir());
    }

    /// **하나가 실패해도 나머지는 한다.** 한 파일이 잠겼다고 전부 포기하면 사용자가 또 해야 한다.
    #[test]
    fn 하나가_실패해도_나머지는_한다() {
        let (s, dt) = (tmp("part_s"), tmp("part_d"));
        write(&s, "있다.txt", "본문");
        let d = run(&s, &dt, &[
            SyncAction::Copy("없다.txt".into()), // 원본이 없다 → 실패
            SyncAction::Copy("있다.txt".into()),
        ]);
        assert_eq!(d.copied, 1);
        assert_eq!(d.failed, 1);
        assert_eq!(read(&dt, "있다.txt").as_deref(), Some("본문"));
    }
}
