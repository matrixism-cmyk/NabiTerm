//! 세션 그룹(folder) 일괄 조작 — 이름 바꾸기·해산(순수 함수, 저장은 호출측).
//!
//! 그룹은 별도 엔티티가 아니라 세션들의 `folder` 라벨 집합이다. 그래서 그룹 조작은
//! "해당 라벨을 가진 모든 세션을 고쳐 쓰기"로 일반화된다(백로그 2026-06-22 사용자 요청).

use crate::model::SavedSession;

/// 그룹 이름을 바꾼다(그 그룹의 모든 세션 일괄). 빈/공백 새 이름은 무시. 바뀐 세션 수 반환.
pub fn rename_group(list: &mut [SavedSession], old: &str, new: &str) -> usize {
    let new = new.trim();
    if new.is_empty() || new == old {
        return 0;
    }
    let mut n = 0;
    for s in list.iter_mut().filter(|s| s.folder.as_deref() == Some(old)) {
        s.folder = Some(new.to_string());
        n += 1;
    }
    n
}

/// 그룹을 해산한다(세션은 그룹 없음으로 남긴다 — 삭제 아님). 바뀐 세션 수 반환.
pub fn disband_group(list: &mut [SavedSession], group: &str) -> usize {
    let mut n = 0;
    for s in list.iter_mut().filter(|s| s.folder.as_deref() == Some(group)) {
        s.folder = None;
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SavedSession, SessionKind};

    fn s(name: &str, folder: Option<&str>) -> SavedSession {
        SavedSession {
            name: name.into(), folder: folder.map(str::to_string),
            kind: SessionKind::Local { shell: "pwsh".into() },
            on_connect: None, cwd: None, is_ftp: false, open_sftp: false, tag: Default::default(),
        }
    }

    #[test]
    fn renames_all_in_group() {
        let mut v = vec![s("a", Some("운영")), s("b", Some("운영")), s("c", Some("개발")), s("d", None)];
        assert_eq!(rename_group(&mut v, "운영", "프로덕션"), 2);
        assert_eq!(v[0].folder.as_deref(), Some("프로덕션"));
        assert_eq!(v[2].folder.as_deref(), Some("개발"), "다른 그룹 불변");
        assert_eq!(rename_group(&mut v, "개발", "  "), 0, "빈 이름 무시");
        assert_eq!(rename_group(&mut v, "개발", "개발"), 0, "동일 이름 무시");
    }

    #[test]
    fn disband_keeps_sessions() {
        let mut v = vec![s("a", Some("운영")), s("b", Some("개발"))];
        assert_eq!(disband_group(&mut v, "운영"), 1);
        assert_eq!(v[0].folder, None, "세션은 남고 그룹만 해제");
        assert_eq!(v[1].folder.as_deref(), Some("개발"));
    }
}
